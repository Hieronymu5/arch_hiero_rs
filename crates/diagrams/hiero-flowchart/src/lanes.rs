//! Swimlanes: a pool's lanes fill it, stacked one after the other, and the steps in a lane stay
//! within it. Lanes run horizontally (rows, one under the other) or vertically (columns, side
//! by side), as their pool says.
//!
//! This began as a copy of the physical diagram's zones filling a network. It's kept separate
//! on purpose, so the two can work differently.

use std::collections::HashMap;

use hiero_core::engine::{Dir, El, Engine, Laid, Placement, ResizeLimits, descendant_ids};
use hiero_core::geometry::{Point, Rect, Size, max_of};
use hiero_core::layout::{
    ContainerRect, MIN_CONTAINER_SIZE, clamp_into, distribute, fit_container, snap_to_canvas,
};
use hiero_core::model::ElementId;

use crate::model::{Flowchart, FlowchartKind, LANE_SIZE, Orientation, POOL_SIZE};

type FEngine = Engine<Flowchart>;
type FLaid = Laid<Flowchart>;

fn info(kind: FlowchartKind) -> &'static hiero_core::model::KindInfo<FlowchartKind> {
    <Flowchart as hiero_core::engine::DiagramType>::info(kind)
}

/// Whether a pool's lanes run vertically, as columns.
pub fn is_vertical(pool: &El<Flowchart>) -> bool {
    pool.props.orientation == Some(Orientation::Vertical)
}

/// Where a rect starts, and how long it is, along the way a pool's lanes are stacked.
fn start(r: Rect, vertical: bool) -> f64 {
    if vertical { r.x } else { r.y }
}

fn length(r: Rect, vertical: bool) -> f64 {
    if vertical { r.width } else { r.height }
}

/// An element's area as stored: its position, and its size or its kind's.
fn sized(e: &El<Flowchart>) -> Rect {
    let size = info(e.kind).size();
    Rect::new(
        e.x,
        e.y,
        e.width.unwrap_or(size.width),
        e.height.unwrap_or(size.height),
    )
}

/// The pool a lane is in, which it stays in.
pub fn pool_of<'a>(eng: &'a FEngine, lane: &FLaid) -> Option<&'a FLaid> {
    if lane.el.kind != FlowchartKind::Lane {
        return None;
    }
    let parent = lane.el.parent_id.as_ref()?;
    eng.layout()
        .iter()
        .find(|e| e.id() == parent && e.el.kind == FlowchartKind::Pool)
}

/// A pool's lanes, in order: top to bottom, or left to right.
pub fn lanes_in<'a>(eng: &'a FEngine, pool: &ElementId) -> Vec<&'a FLaid> {
    let vertical = eng.element(pool).is_some_and(is_vertical);
    let mut lanes: Vec<&FLaid> = eng
        .layout()
        .iter()
        .filter(|e| e.el.parent_id.as_ref() == Some(pool) && e.el.kind == FlowchartKind::Lane)
        .collect();
    lanes.sort_by(|a, b| {
        start(a.rect, vertical)
            .partial_cmp(&start(b.rect, vertical))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    lanes
}

/// The smallest a lane can be: around the steps it holds.
pub fn minimum_of(eng: &FEngine, lane: &FLaid) -> Size {
    let children: Vec<Rect> = eng
        .layout()
        .iter()
        .filter(|c| c.el.parent_id.as_ref() == Some(lane.id()))
        .map(|c| c.rect)
        .collect();
    fit_container(
        &ContainerRect {
            rect: lane.rect.with_size(MIN_CONTAINER_SIZE),
            ..lane.container_rect()
        },
        &children,
    )
}

/// `position` moved so an element of `size` fits in `container` without it growing: inside
/// its padding and under its header. A container too small to hold it leaves it where it is.
pub fn keep_inside(position: Point, size: Size, container: &FLaid) -> Point {
    let r = container.rect;
    let area = Rect::new(
        r.x + container.padding,
        r.y + container.header,
        r.width - container.padding * 2.0,
        r.height - container.header - container.padding - container.footer,
    );
    if area.width < size.width || area.height < size.height {
        position
    } else {
        clamp_into(position, size, area)
    }
}

/// Stacks the lanes in `pool_id` to fill it: across its full width (or height), one after the
/// other, the last taking any room left.
pub fn fill_pool(elements: Vec<El<Flowchart>>, pool_id: &ElementId) -> Vec<El<Flowchart>> {
    let Some(pool) = elements.iter().find(|e| e.id == *pool_id).cloned() else {
        return elements;
    };
    let vertical = is_vertical(&pool);
    let kind = info(pool.kind);
    let (header, padding, footer) = (kind.header(), kind.padding(), kind.footer());
    let mut lanes: Vec<(usize, &El<Flowchart>)> = elements
        .iter()
        .filter(|e| e.parent_id.as_ref() == Some(&pool.id) && e.kind == FlowchartKind::Lane)
        .enumerate()
        .collect();
    lanes.sort_by(|(ai, a), (bi, b)| {
        start(sized(a), vertical)
            .partial_cmp(&start(sized(b), vertical))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ai.cmp(bi))
    });
    let lanes: Vec<El<Flowchart>> = lanes.into_iter().map(|(_, l)| l.clone()).collect();
    if lanes.is_empty() {
        return elements;
    }
    let minimums: Vec<Size> = lanes
        .iter()
        .map(|lane| {
            let lk = info(lane.kind);
            fit_container(
                &ContainerRect {
                    rect: Rect::from_parts(Point::new(lane.x, lane.y), MIN_CONTAINER_SIZE),
                    header: lk.header(),
                    padding: lk.padding(),
                    footer: lk.footer(),
                },
                &elements
                    .iter()
                    .filter(|e| e.parent_id.as_ref() == Some(&lane.id))
                    .map(sized)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let size = sized(&pool);
    // How far each lane reaches across the pool, and how long each is along it.
    let across = if vertical {
        max_of(
            size.height - header - padding - footer,
            minimums.iter().map(|m| m.height),
        )
    } else {
        max_of(size.width - padding * 2.0, minimums.iter().map(|m| m.width))
    };
    let mut lengths: Vec<f64> = lanes
        .iter()
        .zip(&minimums)
        .map(|(lane, min)| {
            let own = length(sized(lane), vertical);
            own.max(if vertical { min.width } else { min.height })
        })
        .collect();
    let room = if vertical {
        size.width - padding * 2.0
    } else {
        size.height - header - padding - footer
    };
    let total: f64 = lengths.iter().sum();
    if room > total
        && let Some(last) = lengths.last_mut()
    {
        *last += room - total;
    }
    let mut placed: HashMap<ElementId, Rect> = HashMap::new();
    let mut along = if vertical {
        pool.x + padding
    } else {
        pool.y + header
    };
    for (lane, len) in lanes.iter().zip(&lengths) {
        let rect = if vertical {
            Rect::new(along, pool.y + header, *len, across)
        } else {
            Rect::new(pool.x + padding, along, across, *len)
        };
        placed.insert(lane.id.clone(), rect);
        along += len;
    }
    let mut shifts: HashMap<ElementId, Point> = HashMap::new();
    for lane in &lanes {
        let r = placed[&lane.id];
        if r.x != lane.x || r.y != lane.y {
            for id in descendant_ids(&elements, &lane.id) {
                shifts.insert(id, Point::new(r.x - lane.x, r.y - lane.y));
            }
        }
    }
    elements
        .into_iter()
        .map(|mut e| {
            if let Some(r) = placed.get(&e.id) {
                e.x = r.x;
                e.y = r.y;
                e.width = Some(r.width);
                e.height = Some(r.height);
            } else if let Some(s) = shifts.get(&e.id) {
                e.x += s.x;
                e.y += s.y;
            }
            e
        })
        .collect()
}

/// Moves `lane` to `index` in its pool, restacking the lanes in their new order.
pub fn move_in_pool(eng: &mut FEngine, lane_id: &ElementId, pool_id: &ElementId, index: usize) {
    let (Some(lane), Some(pool)) = (eng.laid(lane_id).cloned(), eng.laid(pool_id).cloned()) else {
        return;
    };
    let vertical = is_vertical(&pool.el);
    let lanes: Vec<FLaid> = lanes_in(eng, pool_id).into_iter().cloned().collect();
    if lanes.get(index).is_some_and(|s| s.id() == lane_id) {
        return;
    }
    let mut order: Vec<FLaid> = lanes
        .iter()
        .filter(|e| e.id() != lane_id)
        .cloned()
        .collect();
    order.insert(index.min(order.len()), lane.clone());
    let mut shifts: HashMap<ElementId, f64> = HashMap::new();
    let mut along = if vertical {
        pool.rect.x + pool.padding
    } else {
        pool.rect.y + pool.header
    };
    for each in &order {
        let at = start(each.rect, vertical);
        if at != along {
            shifts.insert(each.id().clone(), along - at);
            for id in descendant_ids(eng.elements(), each.id()) {
                shifts.insert(id, along - at);
            }
        }
        along += length(each.rect, vertical);
    }
    let elements = eng
        .elements()
        .iter()
        .cloned()
        .map(|mut e| {
            if let Some(d) = shifts.get(&e.id) {
                if vertical {
                    e.x += d;
                } else {
                    e.y += d;
                }
            }
            e
        })
        .collect();
    eng.set_elements(elements);
    eng.announcement = format!(
        "{} moved to {} of {} in {}.",
        lane.el.label,
        index + 1,
        order.len(),
        pool.el.label
    );
}

/// An arrow key on a step already against that edge of its lane moves it across the edge into
/// whatever is there, such as the next lane, or onto the canvas.
pub fn step_out_of_lane(eng: &mut FEngine, element: &FLaid, dir: Dir) -> bool {
    let Some(parent) = element
        .el
        .parent_id
        .as_ref()
        .and_then(|p| eng.laid(p))
        .cloned()
    else {
        return false;
    };
    let (dx, dy) = match dir {
        Dir::Left => (-1.0, 0.0),
        Dir::Right => (1.0, 0.0),
        Dir::Up => (0.0, -1.0),
        Dir::Down => (0.0, 1.0),
    };
    let r = element.rect;
    let nudged = Point::new(r.x + dx, r.y + dy);
    let kept = keep_inside(nudged, r.size(), &parent);
    if kept.x != r.x || kept.y != r.y {
        return false;
    }
    let p = parent.rect;
    let out = Point::new(
        if dx > 0.0 {
            p.right()
        } else if dx < 0.0 {
            p.x - r.width
        } else {
            r.x
        },
        if dy > 0.0 {
            p.bottom()
        } else if dy < 0.0 {
            p.y - r.height
        } else {
            r.y
        },
    );
    let middle = Rect::from_parts(out, r.size()).center();
    let next = eng
        .containers_for(element.el.kind, eng.layout())
        .into_iter()
        .rev()
        .find(|c| c.id() != element.id() && c.rect.contains_point(middle))
        .cloned();
    let snapped = snap_to_canvas(out, r.size(), eng.canvas());
    let placement = Placement {
        position: next
            .as_ref()
            .map_or(snapped, |n| keep_inside(snapped, r.size(), n)),
        parent_id: next.map(|n| n.id().clone()),
    };
    eng.commit_move(element.id(), placement);
    true
}

/// The elements with `id` resized to `rect`: a pool shares its new length among its lanes in
/// proportion, and a lane resized makes its pool match. `None` when neither is involved.
pub fn resized(
    eng: &FEngine,
    elements: &[El<Flowchart>],
    id: &ElementId,
    rect: Rect,
) -> Option<Vec<El<Flowchart>>> {
    let target = eng.laid(id)?;
    let pool = if target.el.kind == FlowchartKind::Pool {
        target.clone()
    } else {
        pool_of(eng, target)?.clone()
    };
    let lanes: Vec<FLaid> = lanes_in(eng, pool.id()).into_iter().cloned().collect();
    if lanes.is_empty() {
        return None;
    }
    let vertical = is_vertical(&pool.el);
    let (header, padding, footer) = (pool.header, pool.padding, pool.footer);
    let (pool_rect, lengths) = if pool.id() == target.id() {
        let room = if vertical {
            rect.width - padding * 2.0
        } else {
            rect.height - header - padding - footer
        };
        let lengths = distribute(
            &lanes
                .iter()
                .map(|l| length(l.rect, vertical))
                .collect::<Vec<_>>(),
            &lanes
                .iter()
                .map(|l| {
                    let m = minimum_of(eng, l);
                    if vertical { m.width } else { m.height }
                })
                .collect::<Vec<_>>(),
            room,
        );
        (rect, lengths)
    } else {
        let lengths: Vec<f64> = lanes
            .iter()
            .map(|l| {
                if l.id() == id {
                    length(rect, vertical)
                } else {
                    length(l.rect, vertical)
                }
            })
            .collect();
        let total: f64 = lengths.iter().sum();
        let pool_rect = if vertical {
            Rect::new(
                pool.rect.x,
                rect.y - header,
                total + padding * 2.0,
                rect.height + header + padding + footer,
            )
        } else {
            Rect::new(
                rect.x - padding,
                pool.rect.y,
                rect.width + padding * 2.0,
                total + header + padding + footer,
            )
        };
        (pool_rect, lengths)
    };
    let new_lengths: HashMap<&ElementId, f64> = lanes
        .iter()
        .zip(lengths)
        .map(|(l, h)| (l.id(), h))
        .collect();
    Some(
        elements
            .iter()
            .cloned()
            .map(|mut e| {
                if e.id == *pool.id() {
                    e.x = pool_rect.x;
                    e.y = pool_rect.y;
                    e.width = Some(pool_rect.width);
                    e.height = Some(pool_rect.height);
                } else if let Some(len) = new_lengths.get(&e.id) {
                    if vertical {
                        e.width = Some(*len);
                    } else {
                        e.height = Some(*len);
                    }
                }
                e
            })
            .collect(),
    )
}

/// The smallest a pool can be: what its lanes need to hold their steps.
pub fn resize_limits(eng: &FEngine, pool: &FLaid) -> Option<ResizeLimits> {
    let minimums: Vec<Size> = lanes_in(eng, pool.id())
        .into_iter()
        .map(|l| minimum_of(eng, l))
        .collect();
    if minimums.is_empty() {
        return None;
    }
    let extra_across = pool.padding * 2.0;
    let extra_along = pool.header + pool.padding + pool.footer;
    let min_size = if is_vertical(&pool.el) {
        Size::new(
            minimums.iter().map(|m| m.width).sum::<f64>() + extra_across,
            max_of(0.0, minimums.iter().map(|m| m.height)) + extra_along,
        )
    } else {
        Size::new(
            max_of(0.0, minimums.iter().map(|m| m.width)) + extra_across,
            minimums.iter().map(|m| m.height).sum::<f64>() + extra_along,
        )
    };
    Some(ResizeLimits {
        min_size: Size::new(
            min_size.width.max(MIN_CONTAINER_SIZE.width),
            min_size.height.max(MIN_CONTAINER_SIZE.height),
        ),
        children: Vec::new(),
    })
}

/// Where a lane dragged in its pool lands: still in it, across its full width (or height),
/// moving only along the stack.
pub fn constrain_lane(pool: &FLaid, lane: &FLaid, placement: &Placement) -> Placement {
    let position = if is_vertical(&pool.el) {
        let left = pool.rect.x + pool.padding;
        let right = pool.rect.right() - pool.padding - lane.rect.width;
        Point::new(
            placement.position.x.max(left).min(left.max(right)),
            lane.rect.y,
        )
    } else {
        let top = pool.rect.y + pool.header;
        let bottom = pool.rect.bottom() - pool.padding - pool.footer - lane.rect.height;
        Point::new(
            lane.rect.x,
            placement.position.y.max(top).min(top.max(bottom)),
        )
    };
    Placement {
        position,
        parent_id: Some(pool.id().clone()),
    }
}

/// The place in its pool a lane dragged to `position` takes: where its middle lands.
pub fn index_at(eng: &FEngine, pool: &FLaid, lane: &FLaid, position: Point) -> usize {
    let vertical = is_vertical(&pool.el);
    let middle = if vertical {
        position.x + lane.rect.width / 2.0
    } else {
        position.y + lane.rect.height / 2.0
    };
    lanes_in(eng, pool.id())
        .into_iter()
        .filter(|other| {
            other.id() != lane.id()
                && start(other.rect, vertical) + length(other.rect, vertical) / 2.0 < middle
        })
        .count()
}

/// The keys that move a lane earlier or later in its pool: up and down for rows, left and
/// right for columns. Zero for the others.
pub fn lane_step(pool: &FLaid, dir: Dir) -> i64 {
    match (is_vertical(&pool.el), dir) {
        (false, Dir::Up) | (true, Dir::Left) => -1,
        (false, Dir::Down) | (true, Dir::Right) => 1,
        _ => 0,
    }
}

/// A new lane for `pool`, as `(rect)`: after its last lane, across its full width (or height).
pub fn new_lane_rect(eng: &FEngine, pool: &FLaid) -> Rect {
    let lanes = lanes_in(eng, pool.id());
    if is_vertical(&pool.el) {
        let x = max_of(
            pool.rect.x + pool.padding,
            lanes.iter().map(|l| l.rect.right()),
        );
        Rect::new(
            x,
            pool.rect.y + pool.header,
            crate::model::COLUMN_WIDTH,
            pool.rect.height - pool.header - pool.padding - pool.footer,
        )
    } else {
        let y = max_of(
            pool.rect.y + pool.header,
            lanes.iter().map(|l| l.rect.bottom()),
        );
        Rect::new(
            pool.rect.x + pool.padding,
            y,
            pool.rect.width - pool.padding * 2.0,
            LANE_SIZE.height,
        )
    }
}

/// The elements with `pool`'s lanes turned to run the other way: each lane's steps keep their
/// place along the flow, so what went right now goes down, and the other way round. The pool
/// is resized round its turned lanes.
pub fn turned(eng: &FEngine, pool_id: &ElementId) -> Vec<El<Flowchart>> {
    let Some(pool) = eng.laid(pool_id).cloned() else {
        return eng.elements().to_vec();
    };
    // The lanes in their old order, as laid out the old way.
    let was_vertical = is_vertical(&pool.el);
    let mut lanes: Vec<FLaid> = eng
        .layout()
        .iter()
        .filter(|e| e.el.parent_id.as_ref() == Some(pool_id) && e.el.kind == FlowchartKind::Lane)
        .cloned()
        .collect();
    lanes.sort_by(|a, b| {
        start(a.rect, was_vertical)
            .partial_cmp(&start(b.rect, was_vertical))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let vertical = !was_vertical;
    let mut new_rects: HashMap<ElementId, Rect> = HashMap::new();
    let mut moves: HashMap<ElementId, Point> = HashMap::new();
    let mut along = if vertical {
        pool.rect.x + pool.padding
    } else {
        pool.rect.y + pool.header
    };
    let mut reach: f64 = 0.0;
    for lane in &lanes {
        let (h, p) = (lane.header, lane.padding);
        let r = lane.rect;
        // The lane's content turned: its width becomes its height, less its header.
        let width = r.height - h + p;
        let height = r.width + h - p;
        let rect = if vertical {
            Rect::new(along, pool.rect.y + pool.header, width, height)
        } else {
            Rect::new(pool.rect.x + pool.padding, along, width, height)
        };
        along += length(rect, vertical);
        reach = reach.max(if vertical { height } else { width });
        for child in eng
            .layout()
            .iter()
            .filter(|c| c.el.parent_id.as_ref() == Some(lane.id()))
        {
            let u = child.rect.x - (r.x + p);
            let v = child.rect.y - (r.y + h);
            moves.insert(
                child.id().clone(),
                Point::new(rect.x + p + v, rect.y + h + u),
            );
        }
        new_rects.insert(lane.id().clone(), rect);
    }
    let pool_rect = if vertical {
        Rect::new(
            pool.rect.x,
            pool.rect.y,
            along - pool.rect.x + pool.padding,
            reach + pool.header + pool.padding + pool.footer,
        )
    } else {
        Rect::new(
            pool.rect.x,
            pool.rect.y,
            reach + pool.padding * 2.0,
            along - pool.rect.y + pool.padding + pool.footer,
        )
    };
    eng.elements()
        .iter()
        .cloned()
        .map(|mut e| {
            if e.id == *pool_id {
                e.width = Some(pool_rect.width);
                e.height = Some(pool_rect.height);
            } else if let Some(r) = new_rects.get(&e.id) {
                e.x = r.x;
                e.y = r.y;
                e.width = Some(r.width);
                e.height = Some(r.height);
            } else if let Some(at) = moves.get(&e.id) {
                e.x = at.x;
                e.y = at.y;
            }
            e
        })
        .collect()
}

/// The lanes a new pool at `at` starts with, as `(label, rect)`: two, each half its height
/// under its header.
pub fn starting_lanes(at: Point) -> Vec<(&'static str, Rect)> {
    crate::model::STARTING_LANES
        .iter()
        .enumerate()
        .map(|(i, label)| {
            (
                *label,
                Rect::new(
                    at.x,
                    at.y + crate::model::POOL_HEADER + i as f64 * LANE_SIZE.height,
                    POOL_SIZE.width,
                    LANE_SIZE.height,
                ),
            )
        })
        .collect()
}
