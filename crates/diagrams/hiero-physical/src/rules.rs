//! The physical diagram's rules: zones filling their networks, figures kept within the room a
//! zone has, firewalls standing on their own, and arrows that start with the right protocol.

use std::collections::HashMap;

use hiero_core::engine::{
    Created, DiagramType, Dir, El, Engine, Laid, Placement, ResizeLimits, descendant_ids,
    validate_label, validate_length,
};
use hiero_core::figures::figure_rect;
use hiero_core::geometry::{GRID, Point, Rect, Size, max_of, min_of};
use hiero_core::layout::{
    CONTAINER_HEADER, ContainerRect, MIN_CONTAINER_SIZE, clamp_into, distribute, fit_container,
    snap_to_canvas, stacked_placement,
};
use hiero_core::model::{DiagramTypeId, Element, ElementId, KindInfo};

use crate::model::{
    DATABASE_PROTOCOL, DEFAULT_NETWORK_ZONES, GENERIC_DATABASE, MAX_INSTANCE_COUNT, PALETTE,
    PORT_MAX, PROTOCOL_MAX, Physical, PhysicalArrow, PhysicalColors, PhysicalKind, PhysicalProps,
    ZONE_SIZE,
};

type PEngine = Engine<Physical>;
type PLaid = Laid<Physical>;

/// Where the starting zones and network go: two grid steps in from the top-left corner.
const MARGIN: f64 = GRID * 2.0;

fn info(kind: PhysicalKind) -> &'static KindInfo<PhysicalKind> {
    Physical::info(kind)
}

/// The figure an element is drawn as: its kind's, or for a server, the one for what it is.
pub fn figure_of(element: &El<Physical>) -> Option<hiero_core::figures::PhysicalFigure> {
    if element.kind.has_server_type()
        && let Some(server) = element.props.server_type
    {
        return Some(server.figure());
    }
    element.kind.figure()
}

/// How many of an element run: one unless its kind counts them.
pub fn instance_count_of(element: &El<Physical>) -> u32 {
    if element.kind.instances().is_some() {
        element.props.instance_count.unwrap_or(1)
    } else {
        1
    }
}

/// Which database an element is, when it's a database with a type set.
pub fn database_of(element: &El<Physical>) -> Option<crate::model::DatabaseInfo> {
    element
        .kind
        .has_database_type()
        .then_some(element.props.database_type)
        .flatten()
        .map(crate::model::DatabaseType::info)
}

/// Whether `element` is stacked in a container that holds a stack, as a zone in a network.
pub fn is_stacked(eng: &PEngine, element: &El<Physical>) -> bool {
    element.kind.stacks()
        && element
            .parent_id
            .as_ref()
            .and_then(|p| eng.element(p))
            .is_some_and(|parent| parent.kind.holds_stack())
}

/// Whether `element` holds any stacked elements, such as a network with zones.
pub fn holds_stack(eng: &PEngine, element: &El<Physical>) -> bool {
    element.kind.holds_stack()
        && eng
            .elements()
            .iter()
            .any(|e| e.parent_id.as_ref() == Some(&element.id) && e.kind.stacks())
}

/// The network a zone is stacked in, which it stays in.
pub fn stack_holder_of<'a>(eng: &'a PEngine, element: &PLaid) -> Option<&'a PLaid> {
    if !element.el.kind.stacks() {
        return None;
    }
    let parent = element.el.parent_id.as_ref()?;
    eng.layout()
        .iter()
        .find(|e| e.id() == parent && holds_stack(eng, &e.el))
}

/// The stacked elements a container holds, such as a network's zones, from top to bottom.
pub fn stacked_in<'a>(eng: &'a PEngine, container: &ElementId) -> Vec<&'a PLaid> {
    let mut stack: Vec<&PLaid> = eng
        .layout()
        .iter()
        .filter(|e| e.el.parent_id.as_ref() == Some(container) && e.el.kind.stacks())
        .collect();
    stack.sort_by(|a, b| a.rect.y.partial_cmp(&b.rect.y).unwrap_or(std::cmp::Ordering::Equal));
    stack
}

/// The smallest a zone can be: around what it holds, and no smaller than any container.
fn minimum_of(eng: &PEngine, zone: &PLaid) -> Size {
    let children: Vec<Rect> = eng
        .layout()
        .iter()
        .filter(|c| c.el.parent_id.as_ref() == Some(zone.id()))
        .map(|c| c.rect)
        .collect();
    fit_container(
        &ContainerRect {
            rect: zone.rect.with_size(MIN_CONTAINER_SIZE),
            ..zone.container_rect()
        },
        &children,
    )
}

/// `position` moved so an element of `size` fits in `container` without it growing: inside
/// its padding, under its header, and above its footer. A container too small to hold it
/// leaves it where it is.
fn keep_inside(position: Point, size: Size, container: &PLaid) -> Point {
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

/// Stacks the zones in `holder_id` to fill it.
fn fill_with_stack(elements: Vec<El<Physical>>, holder_id: &ElementId) -> Vec<El<Physical>> {
    let Some(holder) = elements.iter().find(|e| e.id == *holder_id).cloned() else {
        return elements;
    };
    let kind = info(holder.kind);
    let (header, padding, footer) = (kind.header(), kind.padding(), kind.footer());
    let children: Vec<&El<Physical>> = elements
        .iter()
        .filter(|e| e.parent_id.as_ref() == Some(&holder.id))
        .collect();
    let mut stack: Vec<(usize, &El<Physical>)> = children
        .iter()
        .filter(|e| e.kind.stacks())
        .copied()
        .enumerate()
        .collect();
    stack.sort_by(|(ai, a), (bi, b)| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ai.cmp(bi))
    });
    let stack: Vec<El<Physical>> = stack.into_iter().map(|(_, z)| z.clone()).collect();
    if stack.is_empty() {
        return elements;
    }
    let sized = |e: &El<Physical>| {
        let size = info(e.kind).size();
        Rect::new(e.x, e.y, e.width.unwrap_or(size.width), e.height.unwrap_or(size.height))
    };
    let minimums: Vec<Size> = stack
        .iter()
        .map(|zone| {
            let zk = info(zone.kind);
            fit_container(
                &ContainerRect {
                    rect: Rect::from_parts(Point::new(zone.x, zone.y), MIN_CONTAINER_SIZE),
                    header: zk.header(),
                    padding: zk.padding(),
                    footer: zk.footer(),
                },
                &elements
                    .iter()
                    .filter(|e| e.parent_id.as_ref() == Some(&zone.id))
                    .map(sized)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let size = fit_container(
        &ContainerRect {
            rect: sized(&holder),
            header,
            padding,
            footer,
        },
        &children
            .iter()
            .filter(|e| !e.kind.stacks())
            .map(|e| sized(e))
            .collect::<Vec<_>>(),
    );
    let width = max_of(size.width - padding * 2.0, minimums.iter().map(|m| m.width));
    let mut heights: Vec<f64> = stack
        .iter()
        .zip(&minimums)
        .map(|(zone, min)| sized(zone).height.max(min.height))
        .collect();
    let room = size.height - header - padding - footer;
    let total: f64 = heights.iter().sum();
    if room > total
        && let Some(last) = heights.last_mut()
    {
        *last += room - total;
    }
    let mut placed: HashMap<ElementId, Rect> = HashMap::new();
    let mut y = holder.y + header;
    for (zone, height) in stack.iter().zip(&heights) {
        placed.insert(zone.id.clone(), Rect::new(holder.x + padding, y, width, *height));
        y += height;
    }
    let mut shifts: HashMap<ElementId, Point> = HashMap::new();
    for zone in &stack {
        let r = placed[&zone.id];
        if r.x != zone.x || r.y != zone.y {
            for id in descendant_ids(&elements, &zone.id) {
                shifts.insert(id, Point::new(r.x - zone.x, r.y - zone.y));
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

/// Moves `zone` to `index` in its network's stack, restacking the zones in their new order.
fn move_in_stack(eng: &mut PEngine, zone_id: &ElementId, holder_id: &ElementId, index: usize) {
    let (Some(zone), Some(holder)) = (eng.laid(zone_id).cloned(), eng.laid(holder_id).cloned()) else {
        return;
    };
    let stack: Vec<PLaid> = stacked_in(eng, holder_id).into_iter().cloned().collect();
    if stack.get(index).is_some_and(|s| s.id() == zone_id) {
        return;
    }
    let mut order: Vec<PLaid> = stack.iter().filter(|e| e.id() != zone_id).cloned().collect();
    order.insert(index.min(order.len()), zone.clone());
    let mut shifts: HashMap<ElementId, f64> = HashMap::new();
    let mut y = holder.rect.y + holder.header;
    for each in &order {
        if each.rect.y != y {
            shifts.insert(each.id().clone(), y - each.rect.y);
            for id in descendant_ids(eng.elements(), each.id()) {
                shifts.insert(id, y - each.rect.y);
            }
        }
        y += each.rect.height;
    }
    let elements = eng
        .elements()
        .iter()
        .cloned()
        .map(|mut e| {
            if let Some(dy) = shifts.get(&e.id) {
                e.y += dy;
            }
            e
        })
        .collect();
    eng.set_elements(elements);
    eng.announcement = format!(
        "{} moved to {} of {} in {}.",
        zone.el.label,
        index + 1,
        order.len(),
        holder.el.label
    );
}

/// An arrow key on an element already against that edge of its zone moves it across the
/// edge into whatever is there, such as the next zone, or onto the canvas.
fn step_out_of_container(eng: &mut PEngine, element: &PLaid, dir: Dir) -> bool {
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
    let inside = descendant_ids(eng.elements(), element.id());
    let next = eng
        .containers_for(element.el.kind, eng.layout())
        .into_iter()
        .rev()
        .find(|c| c.id() != element.id() && !inside.contains(c.id()) && c.rect.contains_point(middle))
        .cloned();
    let snapped = snap_to_canvas(out, r.size());
    let placement = Placement {
        position: next
            .as_ref()
            .map_or(snapped, |n| keep_inside(snapped, r.size(), n)),
        parent_id: next.map(|n| n.id().clone()),
    };
    eng.commit_move(element.id(), placement);
    true
}

/// The first network with zones, which a new one copies.
fn stack_template(eng: &PEngine, kind: PhysicalKind) -> Option<PLaid> {
    eng.layout()
        .iter()
        .find(|e| e.el.kind == kind && !stacked_in(eng, e.id()).is_empty())
        .cloned()
}

/// The zones a new network at `rect` starts with: copies of the first network's, or the
/// usual ones, stacked from the top.
fn starting_stack(eng: &PEngine, kind: PhysicalKind, rect: Rect) -> Vec<(PhysicalKind, String, Rect)> {
    if let Some(template) = stack_template(eng, kind) {
        return stacked_in(eng, template.id())
            .into_iter()
            .map(|child| {
                (
                    child.el.kind,
                    child.el.label.clone(),
                    Rect::new(
                        child.rect.x - template.rect.x + rect.x,
                        child.rect.y - template.rect.y + rect.y,
                        child.rect.width,
                        child.rect.height,
                    ),
                )
            })
            .collect();
    }
    if !kind.holds_stack() {
        return Vec::new();
    }
    let k = info(kind);
    let container = ContainerRect {
        rect,
        header: k.header(),
        padding: k.padding(),
        footer: 0.0,
    };
    let height = info(PhysicalKind::Zone).size().height;
    let mut placed: Vec<Rect> = Vec::new();
    DEFAULT_NETWORK_ZONES
        .iter()
        .map(|label| {
            let child = stacked_placement(&container, &placed, height);
            placed.push(child);
            (PhysicalKind::Zone, (*label).to_owned(), child)
        })
        .collect()
}

/// A position, with the size too when it isn't the kind's default.
fn stored_rect(element: &mut El<Physical>, rect: Rect, default: Size) {
    element.x = rect.x;
    element.y = rect.y;
    element.width = (rect.width != default.width).then_some(rect.width);
    element.height = (rect.height != default.height).then_some(rect.height);
}

/// The elements, with each stacked element outside any container that sits above `rect`, such
/// as the End User zone, widened to span it as well.
fn span_across(eng: &PEngine, rect: Rect) -> Vec<El<Physical>> {
    eng.elements()
        .iter()
        .cloned()
        .map(|mut element| {
            let Some(shown) = eng.laid(&element.id) else {
                return element;
            };
            if element.parent_id.is_some() || !element.kind.stacks() || shown.rect.bottom() > rect.y {
                return element;
            }
            let left = shown.rect.x.min(rect.x);
            let right = shown.rect.right().max(rect.right());
            if left != shown.rect.x || right != shown.rect.right() {
                element.x = left;
                element.width = Some(right - left);
            }
            element
        })
        .collect()
}

fn new_element(eng: &mut PEngine, kind: PhysicalKind, label: &str, parent: Option<ElementId>) -> El<Physical> {
    Element {
        id: ElementId(eng.new_id()),
        kind,
        label: label.to_owned(),
        description: None,
        parent_id: parent,
        props: PhysicalProps::default(),
        width: None,
        height: None,
        x: 0.0,
        y: 0.0,
    }
}

impl DiagramType for Physical {
    const ID: DiagramTypeId = DiagramTypeId::Physical;
    type Kind = PhysicalKind;
    type Props = PhysicalProps;
    type Arrow = PhysicalArrow;
    type Colors = PhysicalColors;

    fn palette() -> &'static [KindInfo<PhysicalKind>] {
        &PALETTE
    }

    /// The End User zone across the top, and under it an Intranet network divided into
    /// Perimeter, Business, and Restricted zones.
    fn starting_elements(eng: &mut PEngine) -> Vec<El<Physical>> {
        let mut end_user = new_element(eng, PhysicalKind::Zone, "End User", None);
        end_user.x = MARGIN;
        end_user.y = MARGIN;
        let mut network = new_element(eng, PhysicalKind::Network, "Intranet", None);
        network.x = MARGIN;
        network.y = end_user.y + ZONE_SIZE.height + MARGIN;
        let mut elements = vec![end_user, network.clone()];
        for (index, label) in DEFAULT_NETWORK_ZONES.iter().enumerate() {
            let mut zone = new_element(eng, PhysicalKind::Zone, label, Some(network.id.clone()));
            zone.x = network.x;
            zone.y = network.y + CONTAINER_HEADER + index as f64 * ZONE_SIZE.height;
            elements.push(zone);
        }
        elements
    }

    /// Zones fill their network. An element in a container that can't hold its kind, such as
    /// a firewall saved in a network, is shown on its own.
    fn arrange(_eng: &PEngine, elements: Vec<El<Physical>>) -> Vec<El<Physical>> {
        let kinds: HashMap<ElementId, PhysicalKind> =
            elements.iter().map(|e| (e.id.clone(), e.kind)).collect();
        let held: Vec<El<Physical>> = elements
            .into_iter()
            .map(|e| {
                let can_hold = e
                    .parent_id
                    .as_ref()
                    .and_then(|p| kinds.get(p))
                    .is_none_or(|parent| info(*parent).can_contain.contains(&e.kind));
                if can_hold { e } else { e.with_parent(None) }
            })
            .collect();
        let holders: Vec<ElementId> = held
            .iter()
            .filter(|e| e.kind.holds_stack())
            .map(|e| e.id.clone())
            .collect();
        holders
            .iter()
            .fold(held, fill_with_stack)
    }

    /// Resizing a network resizes its zones too, in proportion. Resizing a zone in a network
    /// resizes the network to match.
    fn resized(eng: &PEngine, elements: Vec<El<Physical>>, id: &ElementId, rect: Rect) -> Vec<El<Physical>> {
        let Some(target) = eng.laid(id).cloned() else {
            return eng.base_resized(elements, id, rect);
        };
        let holder = if holds_stack(eng, &target.el) {
            Some(target.clone())
        } else {
            target
                .el
                .parent_id
                .as_ref()
                .and_then(|p| eng.laid(p))
                .filter(|p| holds_stack(eng, &p.el))
                .cloned()
        };
        let Some(holder) = holder else {
            return eng.base_resized(elements, id, rect);
        };
        let stack: Vec<PLaid> = stacked_in(eng, holder.id()).into_iter().cloned().collect();
        let room = |height: f64| height - holder.header - holder.padding - holder.footer;
        let (holder_rect, heights) = if holder.id() == target.id() {
            let heights = distribute(
                &stack.iter().map(|z| z.rect.height).collect::<Vec<_>>(),
                &stack.iter().map(|z| minimum_of(eng, z).height).collect::<Vec<_>>(),
                room(rect.height),
            );
            (rect, heights)
        } else {
            let heights: Vec<f64> = stack
                .iter()
                .map(|z| if z.id() == id { rect.height } else { z.rect.height })
                .collect();
            let height: f64 = heights.iter().sum();
            (
                Rect::new(
                    rect.x - holder.padding,
                    holder.rect.y,
                    rect.width + holder.padding * 2.0,
                    height + holder.header + holder.padding + holder.footer,
                ),
                heights,
            )
        };
        let new_heights: HashMap<&ElementId, f64> =
            stack.iter().zip(heights).map(|(z, h)| (z.id(), h)).collect();
        elements
            .into_iter()
            .map(|mut e| {
                if e.id == *holder.id() {
                    e.x = holder_rect.x;
                    e.y = holder_rect.y;
                    e.width = Some(holder_rect.width);
                    e.height = Some(holder_rect.height);
                } else if let Some(h) = new_heights.get(&e.id) {
                    e.height = Some(*h);
                }
                e
            })
            .collect()
    }

    /// A network can't shrink past what its zones need, nor past anything else it holds.
    fn resize_limits(eng: &PEngine, element: &PLaid) -> ResizeLimits {
        if !holds_stack(eng, &element.el) {
            return eng.base_resize_limits(element);
        }
        let minimums: Vec<Size> = stacked_in(eng, element.id())
            .into_iter()
            .map(|z| minimum_of(eng, z))
            .collect();
        ResizeLimits {
            min_size: Size::new(
                max_of(
                    MIN_CONTAINER_SIZE.width,
                    minimums.iter().map(|m| m.width + element.padding * 2.0),
                ),
                MIN_CONTAINER_SIZE.height.max(
                    minimums.iter().map(|m| m.height).sum::<f64>()
                        + element.header
                        + element.padding
                        + element.footer,
                ),
            ),
            children: eng
                .layout()
                .iter()
                .filter(|c| c.el.parent_id.as_ref() == Some(element.id()) && !c.el.kind.stacks())
                .map(|c| c.rect)
                .collect(),
        }
    }

    /// A zone in a network stays in it, across its width, moving only up and down. Anything
    /// else moved in a zone stays within the room the zone has.
    fn constrain_move(eng: &PEngine, element: &PLaid, placement: Placement) -> Placement {
        let Some(holder) = stack_holder_of(eng, element) else {
            let parent = placement.parent_id.as_ref().and_then(|p| eng.laid(p));
            return match parent {
                Some(parent) => Placement {
                    position: keep_inside(placement.position, element.rect.size(), parent),
                    ..placement
                },
                None => placement,
            };
        };
        let top = holder.rect.y + holder.header;
        let bottom = holder.rect.y + holder.rect.height - holder.padding - holder.footer - element.rect.height;
        let y = placement.position.y.max(top).min(top.max(bottom));
        Placement {
            position: Point::new(element.rect.x, y),
            parent_id: Some(holder.id().clone()),
        }
    }

    /// A zone dragged within its network takes the place in its stack where its middle lands.
    fn commit_move(eng: &mut PEngine, id: &ElementId, placement: Placement, with: Option<El<Physical>>) {
        let zone = eng.laid(id).cloned();
        let holder = zone.as_ref().and_then(|z| stack_holder_of(eng, z)).cloned();
        match (zone, holder) {
            (Some(zone), Some(holder)) if placement.parent_id.as_ref() == Some(holder.id()) && with.is_none() => {
                let middle = placement.position.y + zone.rect.height / 2.0;
                let index = stacked_in(eng, holder.id())
                    .into_iter()
                    .filter(|other| other.id() != id && other.rect.y + other.rect.height / 2.0 < middle)
                    .count();
                move_in_stack(eng, id, holder.id(), index);
            }
            _ => eng.base_commit_move(id, placement, with),
        }
    }

    /// Up and down swap a zone in a network with its neighbour; left and right leave it. An
    /// element at a zone's edge steps across it. Groups move as usual.
    fn element_key(eng: &mut PEngine, id: &ElementId, dir: Dir, _shift: bool) -> bool {
        if eng.in_group(id) {
            return false;
        }
        let Some(zone) = eng.laid(id).cloned() else {
            return false;
        };
        let Some(holder) = stack_holder_of(eng, &zone).cloned() else {
            return step_out_of_container(eng, &zone, dir);
        };
        let stack = stacked_in(eng, holder.id());
        let Some(position) = stack.iter().position(|e| e.id() == id) else {
            return true;
        };
        let step = match dir {
            Dir::Up => -1,
            Dir::Down => 1,
            Dir::Left | Dir::Right => 0,
        };
        let index = position as i64 + step;
        if step != 0 && index >= 0 && (index as usize) < stack.len() {
            move_in_stack(eng, id, holder.id(), index as usize);
        }
        true
    }

    /// Removing the last zone in a network removes the network too.
    fn remove(eng: &mut PEngine, id: &ElementId) {
        let Some(element) = eng.laid(id).cloned() else {
            return;
        };
        let holder = stack_holder_of(eng, &element).cloned();
        let last = holder
            .as_ref()
            .is_some_and(|h| stacked_in(eng, h.id()).len() == 1);
        eng.base_remove(id);
        if let (Some(holder), true) = (holder, last) {
            let elements = eng
                .elements()
                .iter()
                .filter(|o| o.id != *holder.id())
                .cloned()
                .map(|o| {
                    if o.parent_id.as_ref() == Some(holder.id()) {
                        o.with_parent(None)
                    } else {
                        o
                    }
                })
                .collect();
            eng.set_elements(elements);
            eng.remove_connections_of(holder.id());
            eng.announcement = format!(
                "{} and its network {} removed.",
                element.el.label, holder.el.label
            );
        }
    }

    /// A figure dropped in a zone sits within the room the zone has.
    fn drop_position(eng: &PEngine, kind: &'static KindInfo<PhysicalKind>, position: Point, parent: &PLaid) -> Point {
        let size = Self::new_size_of(eng, kind);
        keep_inside(
            snap_to_canvas(keep_inside(position, size, parent), size),
            size,
            parent,
        )
    }

    /// A zone goes under the others in its container, across its width. A network starts with
    /// copies of the first network's zones, or the usual ones, and widens anything stacked
    /// above it, such as the End User zone, to span it too.
    fn created(eng: &mut PEngine, kind: &'static KindInfo<PhysicalKind>, placement: Placement) -> Created<Physical> {
        let default_size = kind.size();
        let parent = placement.parent_id.as_ref().and_then(|p| eng.laid(p)).cloned();
        let template = stack_template(eng, kind.kind);
        let size = Self::new_size_of(eng, kind);
        let rect = match &parent {
            Some(parent) if kind.kind.stacks() => stacked_placement(
                &parent.container_rect(),
                &eng.layout()
                    .iter()
                    .filter(|e| e.el.parent_id.as_ref() == Some(parent.id()) && e.el.kind.stacks())
                    .map(|e| e.rect)
                    .collect::<Vec<_>>(),
                default_size.height,
            ),
            _ => Rect::from_parts(snap_to_canvas(placement.position, size), size),
        };
        let mut element = new_element(eng, kind.kind, kind.label, placement.parent_id.clone());
        let stored = if template.is_some() {
            rect
        } else {
            rect.with_size(default_size)
        };
        stored_rect(&mut element, stored, default_size);
        let stacked: Vec<El<Physical>> = starting_stack(eng, kind.kind, rect)
            .into_iter()
            .map(|(child_kind, label, child_rect)| {
                let mut child = new_element(eng, child_kind, &label, Some(element.id.clone()));
                stored_rect(&mut child, child_rect, info(child_kind).size());
                child
            })
            .collect();
        let existing = (!stacked.is_empty()).then(|| span_across(eng, rect));
        let mut added = vec![element];
        added.extend(stacked);
        Created { added, existing }
    }

    /// A network takes the size of the network it copies, or is grown to hold its zones.
    fn new_size_of(eng: &PEngine, kind: &'static KindInfo<PhysicalKind>) -> Size {
        if let Some(template) = stack_template(eng, kind.kind) {
            return template.rect.size();
        }
        let area = Rect::from_parts(Point::default(), kind.size());
        let children: Vec<Rect> = starting_stack(eng, kind.kind, area)
            .into_iter()
            .map(|(_, _, r)| r)
            .collect();
        fit_container(
            &ContainerRect {
                rect: area,
                header: kind.header(),
                padding: kind.padding(),
                footer: kind.footer(),
            },
            &children,
        )
    }

    /// A network goes to the right of the last one, level with the first; the first one goes
    /// under anything stacked across the top, such as the End User zone.
    fn preferred_position(eng: &PEngine, kind: &'static KindInfo<PhysicalKind>) -> Option<Point> {
        if let Some(template) = stack_template(eng, kind.kind) {
            let right = eng
                .layout()
                .iter()
                .filter(|e| e.el.kind == kind.kind)
                .map(|e| e.rect.right())
                .fold(f64::NEG_INFINITY, f64::max);
            return Some(Point::new(right + GRID * 2.0, template.rect.y));
        }
        if !kind.kind.holds_stack() {
            return None;
        }
        let across: Vec<&PLaid> = eng
            .layout()
            .iter()
            .filter(|e| e.el.parent_id.is_none() && e.el.kind.stacks())
            .collect();
        if across.is_empty() {
            return None;
        }
        Some(Point::new(
            min_of(f64::INFINITY, across.iter().map(|e| e.rect.x)),
            max_of(f64::NEG_INFINITY, across.iter().map(|e| e.rect.bottom())) + GRID * 2.0,
        ))
    }

    /// Which database it is, how many run, and its arrows.
    fn extra_details(eng: &PEngine, element: &PLaid) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some(database) = database_of(&element.el) {
            parts.push(database.label.to_owned());
        }
        let count = instance_count_of(&element.el);
        if count > 1
            && let Some((_, noun)) = element.el.kind.instances()
        {
            parts.push(format!("{count} {noun}s"));
        }
        for c in eng.connections() {
            let tag = c.props.tag();
            let over = if tag.is_empty() { String::new() } else { format!(" over {tag}") };
            if c.source_id == *element.id() {
                parts.push(format!("arrow to {}{over}", eng.label_of(&c.target_id).unwrap_or_default()));
            } else if c.target_id == *element.id() {
                parts.push(format!("arrow from {}{over}", eng.label_of(&c.source_id).unwrap_or_default()));
            }
        }
        parts
    }

    /// TCP on its database's port for a database, the kind's own for storage, or HTTPS 443.
    fn new_arrow(eng: &PEngine, _source: &ElementId, target: &ElementId) -> PhysicalArrow {
        let Some(target) = eng.element(target) else {
            return PhysicalArrow {
                protocol: crate::model::DEFAULT_PROTOCOL.into(),
                port: crate::model::DEFAULT_PORT.into(),
            };
        };
        if target.kind.has_database_type() {
            let database = database_of(target).unwrap_or(GENERIC_DATABASE);
            return PhysicalArrow {
                protocol: DATABASE_PROTOCOL.into(),
                port: database.port.into(),
            };
        }
        let (protocol, port) = target.kind.arrow_defaults();
        PhysicalArrow {
            protocol: protocol.into(),
            port: port.into(),
        }
    }

    fn arrow_added(eng: &PEngine, connection: &hiero_core::engine::Conn<Physical>) -> String {
        format!(
            "{} added, over {}:{}.",
            eng.arrow_name(&connection.source_id, &connection.target_id),
            connection.props.protocol,
            connection.props.port
        )
    }

    /// An arrow meets a figure, not its whole area, so it ends close to the figure.
    fn arrow_rect(_eng: &PEngine, element: &PLaid) -> Rect {
        figure_of(&element.el).map_or(element.rect, |f| figure_rect(element.rect, f))
    }

    /// Clears settings the element's kind doesn't have.
    fn apply_element(eng: &mut PEngine, id: &ElementId, mut with: El<Physical>, parent_id: Option<ElementId>) {
        let kind = with.kind;
        if !kind.has_server_type() {
            with.props.server_type = None;
        }
        if kind.instances().is_none() {
            with.props.instance_count = None;
        }
        if with.props.instance_count == Some(1) {
            with.props.instance_count = None;
        }
        if !kind.has_database_type() {
            with.props.database_type = None;
        }
        if !kind.has_zone_color() {
            with.props.zone_color = None;
        }
        with.description = None;
        eng.apply_changes(id, with, parent_id);
    }

    fn validate_element(element: &El<Physical>) -> Result<(), String> {
        validate_label(&element.label)?;
        match element.props.instance_count {
            Some(n) if n == 0 || n > MAX_INSTANCE_COUNT => Err(format!(
                "Enter a whole number from 1 to {MAX_INSTANCE_COUNT}."
            )),
            _ => Ok(()),
        }
    }

    fn validate_arrow(arrow: &PhysicalArrow) -> Result<(), String> {
        if arrow.protocol.trim().is_empty() {
            return Err("Enter a protocol.".into());
        }
        if arrow.port.trim().is_empty() {
            return Err("Enter a port.".into());
        }
        validate_length("Protocol", &arrow.protocol, PROTOCOL_MAX)?;
        validate_length("Port", &arrow.port, PORT_MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    fn engine() -> PEngine {
        let mut n = 0;
        let mut e = Engine::new_diagram(PhysicalColors::default())
            .with_measure(&ESTIMATE)
            .with_ids(move || {
                n += 1;
                format!("id{n}")
            });
        // Rebuild the starting elements with predictable ids.
        let starting = Physical::starting_elements(&mut e);
        e.replace_content(starting, Vec::new());
        e
    }

    fn id(s: &str) -> ElementId {
        s.into()
    }

    fn rect(e: &PEngine, s: &str) -> Rect {
        e.laid(&id(s)).unwrap().rect
    }

    #[test]
    fn starts_with_end_user_and_an_intranet_of_three_zones() {
        let e = engine();
        let labels: Vec<&str> = e.elements().iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["End User", "Intranet", "Perimeter", "Business", "Restricted"]);
        assert_eq!(rect(&e, "id1"), Rect::new(40.0, 40.0, 600.0, 200.0));
        // The network grows to fit its three zones under its header.
        assert_eq!(rect(&e, "id2"), Rect::new(40.0, 280.0, 600.0, 640.0));
        assert_eq!(rect(&e, "id5"), Rect::new(40.0, 720.0, 600.0, 200.0));
    }

    #[test]
    fn a_new_network_copies_the_first_and_goes_beside_it() {
        let mut e = engine();
        e.add_at_free_position(PhysicalKind::Network);
        let network = e.selected().unwrap().clone();
        assert_eq!(network.rect, Rect::new(680.0, 280.0, 600.0, 640.0));
        let zones: Vec<String> = stacked_in(&e, network.id()).iter().map(|z| z.el.label.clone()).collect();
        assert_eq!(zones, ["Perimeter", "Business", "Restricted"]);
        // The End User zone is widened to span both networks.
        assert_eq!(rect(&e, "id1"), Rect::new(40.0, 40.0, 1240.0, 200.0));
        assert!(e.announcement.contains("with zones Perimeter, Business, Restricted"));
    }

    #[test]
    fn zones_swap_with_the_arrow_keys() {
        let mut e = engine();
        e.select(&id("id3"));
        e.arrow_key(&id("id3"), Dir::Down, false);
        let order: Vec<String> = stacked_in(&e, &id("id2")).iter().map(|z| z.el.label.clone()).collect();
        assert_eq!(order, ["Business", "Perimeter", "Restricted"]);
        assert_eq!(e.announcement, "Perimeter moved to 2 of 3 in Intranet.");
    }

    #[test]
    fn resizing_a_network_shares_height_among_zones() {
        let mut e = engine();
        let network = e.laid(&id("id2")).unwrap().clone();
        let r = e.resize_bounds(&network, hiero_core::engine::Corner::BottomRight, Point::new(0.0, 300.0));
        e.commit_resize(&id("id2"), r);
        assert_eq!(rect(&e, "id2").height, 940.0);
        let heights: Vec<f64> = stacked_in(&e, &id("id2")).iter().map(|z| z.rect.height).collect();
        assert_eq!(heights, [300.0, 300.0, 300.0]);
    }

    #[test]
    fn removing_the_last_zone_removes_the_network() {
        let mut e = engine();
        for zone in ["id3", "id4"] {
            e.select(&id(zone));
            e.delete_selection();
        }
        assert!(e.element(&id("id2")).is_some());
        e.select(&id("id5"));
        e.delete_selection();
        assert!(e.element(&id("id2")).is_none());
        assert_eq!(e.announcement, "Restricted and its network Intranet removed.");
    }

    #[test]
    fn servers_stay_within_their_zone_and_step_into_the_next() {
        let mut e = engine();
        e.select(&id("id3"));
        e.add_at_free_position(PhysicalKind::Server);
        let server = e.selected().unwrap().clone();
        assert_eq!(server.el.parent_id, Some(id("id3")));
        // Perimeter is 600×200 at y=320 with a 20 header, footer, and padding: a server's top
        // can go from 340 to 380. Two steps reach the edge; the third crosses it.
        for _ in 0..3 {
            e.arrow_key(server.id(), Dir::Down, false);
        }
        let moved = e.element(server.id()).unwrap();
        assert_eq!(moved.parent_id, Some(id("id4")), "stepped into Business");
    }

    #[test]
    fn arrows_start_with_the_targets_protocol() {
        let mut e = engine();
        e.select(&id("id3"));
        e.add_at_free_position(PhysicalKind::Server);
        e.select(&id("id3"));
        e.add_at_free_position(PhysicalKind::Database);
        e.select(&id("id3"));
        e.add_at_free_position(PhysicalKind::Storage);
        let server = e.elements().iter().find(|x| x.kind == PhysicalKind::Server).unwrap().id.clone();
        let database = e.elements().iter().find(|x| x.kind == PhysicalKind::Database).unwrap().id.clone();
        let storage = e.elements().iter().find(|x| x.kind == PhysicalKind::Storage).unwrap().id.clone();
        e.add_connection(&server, &database);
        assert_eq!(e.announcement, "Arrow from Server to Database added, over TCP:5432.");
        e.add_connection(&server, &storage);
        assert_eq!(e.connections()[1].props.tag(), "TCP:445");
        e.add_connection(&database, &server);
        assert_eq!(e.connections()[2].props.tag(), "HTTPS:443");
    }

    #[test]
    fn firewalls_stand_on_their_own() {
        let mut e = engine();
        let mut elements = e.elements().to_vec();
        let mut firewall = elements[0].clone();
        firewall.id = id("fw");
        firewall.kind = PhysicalKind::Firewall;
        firewall.parent_id = Some(id("id2"));
        elements.push(firewall);
        e.set_elements(elements);
        assert_eq!(e.element(&id("fw")).unwrap().parent_id, None);
    }

    #[test]
    fn instance_counts_are_validated() {
        let e = engine();
        let mut server = e.elements()[0].clone();
        server.kind = PhysicalKind::Server;
        server.props.instance_count = Some(0);
        assert!(Physical::validate_element(&server).is_err());
        server.props.instance_count = Some(3);
        assert!(Physical::validate_element(&server).is_ok());
    }
}
