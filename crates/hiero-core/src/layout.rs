//! Placement and sizing rules shared by every diagram type. A port of the prototype's
//! `utils/diagram-layout.ts`.

use crate::geometry::{CANVAS_HEIGHT, CANVAS_WIDTH, GRID, Point, Rect, Size, max_of, min_of, snap};

/// Where elements can be placed automatically: the canvas less a one-grid margin.
pub const CANVAS_AREA: Rect = Rect::new(GRID, GRID, CANVAS_WIDTH - GRID, CANVAS_HEIGHT - GRID);

/// The whole canvas.
pub const CANVAS: Rect = Rect::new(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT);

/// Size of a placed element when its kind doesn't set one. Both are multiples of the grid.
pub const DEFAULT_ELEMENT_SIZE: Size = Size::new(160.0, 60.0);

/// The smallest a container can be resized to when it's empty.
pub const MIN_CONTAINER_SIZE: Size = Size::new(200.0, 80.0);

/// The smallest any other resizable element can be.
pub const MIN_ELEMENT_SIZE: Size = Size::new(120.0, 60.0);

/// Height of a container's name header unless its kind sets one.
pub const CONTAINER_HEADER: f64 = 40.0;
/// The gap kept around a container's children unless its kind sets one.
pub const CONTAINER_PADDING: f64 = 20.0;

/// A container: an area, with the height of its header, the gap around its children, and the
/// space below them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainerRect {
    pub rect: Rect,
    pub header: f64,
    pub padding: f64,
    pub footer: f64,
}

impl ContainerRect {
    /// A container with the usual header and padding and no footer.
    pub const fn plain(rect: Rect) -> Self {
        Self {
            rect,
            header: CONTAINER_HEADER,
            padding: CONTAINER_PADDING,
            footer: 0.0,
        }
    }
}

/// Rounds a position to the nearest grid point and keeps an element of `size` fully on the
/// canvas.
pub fn snap_to_canvas(p: Point, size: Size) -> Point {
    Point::new(
        snap(p.x).max(0.0).min(CANVAS_WIDTH - size.width),
        snap(p.y).max(0.0).min(CANVAS_HEIGHT - size.height),
    )
}

/// The size an element can be resized to: snapped to the grid, no smaller than `min_size` or
/// than needed to hold its children (for a container), and kept on the canvas.
pub fn resize_element(
    element: &ContainerRect,
    desired: Size,
    min_size: Size,
    children: &[Rect],
) -> Size {
    let min = fit_container(
        &ContainerRect {
            rect: element.rect.with_size(min_size),
            ..*element
        },
        children,
    );
    Size::new(
        snap(desired.width)
            .max(min.width)
            .min(CANVAS_WIDTH - element.rect.x),
        snap(desired.height)
            .max(min.height)
            .min(CANVAS_HEIGHT - element.rect.y),
    )
}

/// Where an element goes when it's resized by moving its top-left corner to `desired`: its
/// bottom-right corner stays put. The corner snaps to the grid and stays inside `bounds`. It
/// can't come so close to the bottom-right corner that the element is smaller than `min_size`,
/// and a container's corner stays clear of its children.
pub fn resize_from_top_left(
    element: &ContainerRect,
    desired: Point,
    min_size: Size,
    children: &[Rect],
    bounds: Rect,
) -> Rect {
    let r = element.rect;
    let right = r.right();
    let bottom = r.bottom();
    let max_x = min_of(
        right - min_size.width,
        children.iter().map(|c| c.x - element.padding),
    );
    let max_y = min_of(
        bottom - min_size.height,
        children.iter().map(|c| c.y - element.header),
    );
    let x = snap(desired.x).max(bounds.x).min(max_x);
    let y = snap(desired.y).max(bounds.y).min(max_y);
    Rect::new(x, y, right - x, bottom - y)
}

/// Moves an element of `size` the least distance needed to sit fully inside `area`.
pub fn clamp_into(p: Point, size: Size, area: Rect) -> Point {
    Point::new(
        p.x.max(area.x).min(area.x + area.width - size.width),
        p.y.max(area.y).min(area.y + area.height - size.height),
    )
}

/// The part of a container where children sit: everything below its name header.
pub fn content_area(container: &ContainerRect) -> Rect {
    let r = container.rect;
    Rect::new(
        r.x,
        r.y + container.header,
        r.width,
        r.height - container.header,
    )
}

/// Where new children are placed automatically: inside the container's padding, under its
/// header, and extending to the bottom of the canvas so the container can grow downwards.
pub fn child_placement_area(container: &ContainerRect) -> Rect {
    let r = container.rect;
    let y = r.y + container.header;
    Rect::new(
        r.x + container.padding,
        y,
        r.width - container.padding * 2.0,
        CANVAS_HEIGHT - y,
    )
}

/// A container's size: at least its own, and big enough to hold its children plus padding,
/// and its footer below them.
pub fn fit_container(container: &ContainerRect, children: &[Rect]) -> Size {
    let r = container.rect;
    let below = container.padding + container.footer;
    Size::new(
        max_of(
            r.width,
            children
                .iter()
                .map(|c| c.x + c.width - r.x + container.padding),
        ),
        max_of(
            r.height,
            children.iter().map(|c| c.y + c.height - r.y + below),
        ),
    )
}

/// Where an element of a kind that stacks, such as a zone, goes in `container`: under the
/// lowest of `siblings`, or at the top of its content, spanning the width inside its padding.
pub fn stacked_placement(container: &ContainerRect, siblings: &[Rect], height: f64) -> Rect {
    let area = child_placement_area(container);
    let y = max_of(area.y, siblings.iter().map(Rect::bottom));
    Rect::new(area.x, y, area.width, height)
}

/// The first position, scanning `area` row by row from its top left, where an element of
/// `size` doesn't overlap any `occupied` area and keeps a one-grid gap from it. Falls back to
/// the area's top-left corner when it's full.
pub fn find_free_position(occupied: &[Rect], size: Size, area: Rect) -> Point {
    let padded: Vec<Rect> = occupied
        .iter()
        .map(|r| {
            Rect::new(
                r.x - GRID,
                r.y - GRID,
                r.width + GRID * 2.0,
                r.height + GRID * 2.0,
            )
        })
        .collect();
    let mut y = area.y;
    while y + size.height <= area.y + area.height {
        let mut x = area.x;
        while x + size.width <= area.x + area.width {
            let candidate = Rect::new(x, y, size.width, size.height);
            if !padded.iter().any(|other| candidate.overlaps(other)) {
                return Point::new(x, y);
            }
            x += GRID;
        }
        y += GRID;
    }
    area.position()
}

/// Shares `total` among items of the given `sizes` in proportion. Where each meets the next is
/// rounded to the grid. Each stays no smaller than its minimum, with the last ones giving up
/// what that takes.
pub fn distribute(sizes: &[f64], minimums: &[f64], total: f64) -> Vec<f64> {
    let sum: f64 = sizes.iter().sum();
    let mut before = 0.0;
    let mut start = 0.0;
    let mut shared: Vec<f64> = sizes
        .iter()
        .enumerate()
        .map(|(i, size)| {
            before += size;
            let end = if i == sizes.len() - 1 {
                total
            } else {
                snap(before * total / sum)
            };
            let share = minimums[i].max(end - start);
            start = end;
            share
        })
        .collect();
    let mut excess = shared.iter().sum::<f64>() - total;
    for i in (0..shared.len()).rev() {
        if excess <= 0.0 {
            break;
        }
        let change = excess.min(shared[i] - minimums[i]);
        shared[i] -= change;
        excess -= change;
    }
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_to_canvas_keeps_elements_on_the_canvas() {
        let size = Size::new(160.0, 60.0);
        assert_eq!(
            snap_to_canvas(Point::new(-30.0, 9.0), size),
            Point::new(0.0, 0.0)
        );
        assert_eq!(
            snap_to_canvas(Point::new(3000.0, 3000.0), size),
            Point::new(2240.0, 1540.0)
        );
        assert_eq!(
            snap_to_canvas(Point::new(31.0, 49.0), size),
            Point::new(40.0, 40.0)
        );
    }

    #[test]
    fn find_free_position_keeps_a_grid_gap() {
        let occupied = [Rect::new(20.0, 20.0, 160.0, 60.0)];
        let p = find_free_position(&occupied, Size::new(160.0, 60.0), CANVAS_AREA);
        assert_eq!(p, Point::new(200.0, 20.0));
    }

    #[test]
    fn find_free_position_falls_back_to_the_corner() {
        let area = Rect::new(0.0, 0.0, 100.0, 100.0);
        let occupied = [Rect::new(0.0, 0.0, 100.0, 100.0)];
        assert_eq!(
            find_free_position(&occupied, Size::new(50.0, 50.0), area),
            Point::new(0.0, 0.0)
        );
    }

    #[test]
    fn fit_container_grows_around_children() {
        let container = ContainerRect::plain(Rect::new(0.0, 0.0, 200.0, 100.0));
        let size = fit_container(&container, &[Rect::new(100.0, 60.0, 200.0, 100.0)]);
        assert_eq!(size, Size::new(320.0, 180.0));
    }

    #[test]
    fn distribute_shares_in_proportion() {
        assert_eq!(
            distribute(&[200.0, 200.0, 200.0], &[80.0, 80.0, 80.0], 900.0),
            vec![300.0, 300.0, 300.0]
        );
        // The minimum of the first is taken from the last.
        assert_eq!(
            distribute(&[20.0, 380.0], &[100.0, 100.0], 400.0),
            vec![100.0, 300.0]
        );
    }

    #[test]
    fn resize_from_top_left_keeps_bottom_right() {
        let element = ContainerRect::plain(Rect::new(100.0, 100.0, 300.0, 200.0));
        let r = resize_from_top_left(
            &element,
            Point::new(60.0, 61.0),
            MIN_CONTAINER_SIZE,
            &[],
            CANVAS,
        );
        assert_eq!(r, Rect::new(60.0, 60.0, 340.0, 240.0));
    }

    #[test]
    fn resize_element_respects_minimum_and_children() {
        let element = ContainerRect::plain(Rect::new(0.0, 0.0, 300.0, 200.0));
        let s = resize_element(
            &element,
            Size::new(10.0, 10.0),
            MIN_CONTAINER_SIZE,
            &[Rect::new(20.0, 40.0, 240.0, 60.0)],
        );
        assert_eq!(s, Size::new(280.0, 120.0));
    }

    #[test]
    fn stacked_placement_goes_under_siblings() {
        let container = ContainerRect {
            rect: Rect::new(40.0, 40.0, 600.0, 240.0),
            header: 40.0,
            padding: 0.0,
            footer: 0.0,
        };
        let r = stacked_placement(&container, &[Rect::new(40.0, 80.0, 600.0, 200.0)], 200.0);
        assert_eq!(r, Rect::new(40.0, 280.0, 600.0, 200.0));
    }
}
