//! Figure geometry shared by the diagram types and their exports: the actor, the component
//! figures, the cloud outline, and the physical diagram's figures. A port of the prototype's
//! `actor-figure.ts`, `component-figure.ts`, `cloud-shape.ts`, and `physical-figures.ts`.

use crate::geometry::{Point, Rect, Size, fmt_num, round2};
use crate::routing::Side;

/// The actor's figure, in a 48 × 46 box: a head above a separate body. A person has a round
/// head and half-circle shoulders; a bot a box head with an antenna above a box body.
pub mod actor {
    use crate::geometry::{Point, Rect};
    pub const WIDTH: f64 = 48.0;
    pub const HEIGHT: f64 = 46.0;
    pub const HEAD_CENTER: Point = Point::new(24.0, 12.0);
    pub const HEAD_RADIUS: f64 = 10.0;
    pub const BOT_HEAD: Rect = Rect::new(14.0, 6.0, 20.0, 16.0);
    pub const BOT_CORNER: f64 = 3.0;
    pub const ANTENNA_X: f64 = 24.0;
    pub const ANTENNA_Y1: f64 = 6.0;
    pub const ANTENNA_Y2: f64 = 3.5;
    pub const ANTENNA_TIP_RADIUS: f64 = 1.5;
    pub const ANTENNA_TIP_Y: f64 = 2.0;
    pub const SHOULDERS: &str = "M4 45 A20 20 0 0 1 44 45 Z";
    pub const BOT_BODY: Rect = Rect::new(8.0, 25.0, 32.0, 20.0);
}

/// The component figures, in the same 48 × 46 box: a database cylinder and a storage stack.
pub mod component {
    use crate::geometry::{Point, Rect, Size};
    pub const CYLINDER_SIDE: &str = "M6 7 V39 A18 6 0 0 0 42 39 V7";
    pub const CYLINDER_TOP_CENTER: Point = Point::new(24.0, 7.0);
    pub const CYLINDER_TOP_RX: f64 = 18.0;
    pub const CYLINDER_TOP_RY: f64 = 6.0;
    pub const DRIVES: [Rect; 4] = [
        Rect::new(4.0, 1.0, 40.0, 8.0),
        Rect::new(4.0, 13.0, 40.0, 8.0),
        Rect::new(4.0, 25.0, 40.0, 8.0),
        Rect::new(4.0, 37.0, 40.0, 8.0),
    ];
    pub const DRIVE_CORNER: f64 = 2.0;
    /// A file: an upright page with its upper-left corner folded down, as a triangle over the
    /// cut corner, and five lines spread evenly down it, like lines of text.
    pub const FILE_PAGE: &str = "M17 1 H41 V45 H7 V11 Z";
    pub const FILE_FOLD: &str = "M17 1 V11 H7 Z";
    /// Each line's start, end, and height. A line level with the fold, which reaches down to
    /// 11, starts clear of it.
    pub const FILE_LINES: [(f64, f64, f64); 5] = [
        (20.0, 34.0, 9.0),
        (14.0, 34.0, 16.0),
        (14.0, 34.0, 23.0),
        (14.0, 34.0, 30.0),
        (14.0, 34.0, 37.0),
    ];
    /// The size of a component drawn as a figure, with its name underneath.
    pub const FIGURE_SIZE: Size = Size::new(120.0, 80.0);
}

/// How far the cloud outline is moved down, as a fraction of its box's height.
pub const CLOUD_DROP: f64 = 0.05;
/// Space kept between a cloud's text and the sides of its box, as a fraction of its width.
pub const CLOUD_TEXT_INSET: f64 = 0.16;

const CLOUD_START: (f64, f64) = (0.25, 0.25);
const CLOUD_CURVES: [[f64; 6]; 6] = [
    [0.05, 0.25, 0.0, 0.5, 0.16, 0.55],
    [0.0, 0.66, 0.18, 0.9, 0.31, 0.8],
    [0.4, 1.0, 0.7, 1.0, 0.8, 0.8],
    [1.0, 0.8, 1.0, 0.6, 0.875, 0.5],
    [1.0, 0.3, 0.8, 0.1, 0.625, 0.2],
    [0.5, 0.05, 0.3, 0.05, 0.25, 0.25],
];

/// The SVG path of a cloud filling `rect`: draw.io's cloud, moved down by `CLOUD_DROP`.
pub fn cloud_path(rect: Rect) -> String {
    let point = |fx: f64, fy: f64| {
        format!(
            "{} {}",
            fmt_num(round2(rect.x + rect.width * fx)),
            fmt_num(round2(rect.y + rect.height * (fy + CLOUD_DROP)))
        )
    };
    let curves: Vec<String> = CLOUD_CURVES
        .iter()
        .map(|[x1, y1, x2, y2, x, y]| {
            format!("C{} {} {}", point(*x1, *y1), point(*x2, *y2), point(*x, *y))
        })
        .collect();
    format!(
        "M{} {} Z",
        point(CLOUD_START.0, CLOUD_START.1),
        curves.join(" ")
    )
}

/// How many straight steps each of the outline's curves is traced in, to find where it is.
const STEPS_PER_CURVE: u32 = 32;

/// The cloud's outline traced as points, as fractions of its box, moved down by `CLOUD_DROP`.
fn cloud_outline() -> Vec<Point> {
    let mut points = Vec::new();
    let (mut x0, mut y0) = CLOUD_START;
    for [x1, y1, x2, y2, x3, y3] in CLOUD_CURVES {
        for step in 0..=STEPS_PER_CURVE {
            let t = f64::from(step) / f64::from(STEPS_PER_CURVE);
            let u = 1.0 - t;
            let bezier = |a: f64, b: f64, c: f64, d: f64| {
                u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
            };
            points.push(Point::new(
                bezier(x0, x1, x2, x3),
                bezier(y0, y1, y2, y3) + CLOUD_DROP,
            ));
        }
        (x0, y0) = (x3, y3);
    }
    points
}

/// Where an arrow ending at `point`, on `side` of a cloud filling `rect`, meets the cloud: the
/// point moved straight into the box until it reaches the outline, so the arrow touches the
/// cloud rather than the box around it. A point beyond the cloud's ends stays where it is.
pub fn cloud_edge_point(rect: Rect, side: Side, point: Point) -> Point {
    let outline: Vec<Point> = cloud_outline()
        .into_iter()
        .map(|p| Point::new(rect.x + rect.width * p.x, rect.y + rect.height * p.y))
        .collect();
    // A top or bottom side moves the point down or up, along its x; a left or right side across.
    let vertical = !side.is_vertical();
    let across = if vertical { point.x } else { point.y };
    let crossings: Vec<f64> = outline
        .windows(2)
        .filter_map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let ((a1, b1), (a2, b2)) = if vertical {
                ((a.x, b.x), (a.y, b.y))
            } else {
                ((a.y, b.y), (a.x, b.x))
            };
            #[allow(
                clippy::float_cmp,
                reason = "a segment parallel to the line never crosses it"
            )]
            let parallel = a1 == b1;
            (!parallel && (a1 - across) * (b1 - across) <= 0.0)
                .then(|| a2 + (across - a1) / (b1 - a1) * (b2 - a2))
        })
        .collect();
    // The crossing nearest the side the arrow comes from.
    let edge = if matches!(side, Side::Top | Side::Left) {
        crossings.into_iter().reduce(f64::min)
    } else {
        crossings.into_iter().reduce(f64::max)
    };
    match edge {
        None => point,
        Some(edge) if vertical => Point::new(point.x, edge),
        Some(edge) => Point::new(edge, point.y),
    }
}

/// A figure drawn for an element on a physical diagram, with its name underneath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicalFigure {
    User,
    Server,
    DatabaseCylinder,
    Firewall,
    LoadBalancer,
    Gateway,
    StorageArray,
    VirtualMachine,
    ContainerStack,
}

/// The height of every physical diagram figure.
pub const FIGURE_HEIGHT: f64 = 40.0;
/// The gap between a figure and its name.
pub const FIGURE_NAME_GAP: f64 = 4.0;

/// A line segment in a figure's own coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seg {
    pub from: Point,
    pub to: Point,
}

const fn seg(x1: f64, y1: f64, x2: f64, y2: f64) -> Seg {
    Seg {
        from: Point::new(x1, y1),
        to: Point::new(x2, y2),
    }
}

pub mod user {
    use crate::geometry::Point;
    pub const WIDTH: f64 = 40.0;
    pub const HEAD_CENTER: Point = Point::new(20.0, 10.0);
    pub const HEAD_RADIUS: f64 = 8.5;
    pub const SHOULDERS: &str = "M3 39 A17 17 0 0 1 37 39 Z";
}

pub mod server {
    use super::{Seg, seg};
    use crate::geometry::Rect;
    pub const WIDTH: f64 = 28.0;
    pub const BODY: Rect = Rect::new(1.0, 1.0, 26.0, 38.0);
    pub const CORNER: f64 = 3.0;
    pub const LINES: [Seg; 3] = [
        seg(6.0, 24.0, 22.0, 24.0),
        seg(6.0, 28.0, 22.0, 28.0),
        seg(6.0, 32.0, 22.0, 32.0),
    ];
}

pub mod firewall {
    use super::Seg;
    use crate::geometry::{Point, Rect};
    pub const WIDTH: f64 = 58.0;
    pub const BODY: Rect = Rect::new(1.0, 1.0, 56.0, 38.0);
    pub const MORTAR_WIDTH: f64 = 1.5;
    const ROWS: usize = 4;
    const PER_ROW: usize = 4;

    /// The mortar between the bricks: the lines between rows, then the joints in each row.
    /// Every other row starts with a half brick.
    pub fn mortar() -> Vec<Seg> {
        let row_height = BODY.height / ROWS as f64;
        let brick = BODY.width / PER_ROW as f64;
        let mut lines: Vec<Seg> = (0..ROWS - 1)
            .map(|i| {
                let y = BODY.y + row_height * (i as f64 + 1.0);
                Seg {
                    from: Point::new(BODY.x, y),
                    to: Point::new(BODY.x + BODY.width, y),
                }
            })
            .collect();
        for row in 0..ROWS {
            let y1 = BODY.y + row_height * row as f64;
            let (offset, joints) = if row % 2 == 1 {
                (brick / 2.0, PER_ROW)
            } else {
                (brick, PER_ROW - 1)
            };
            for j in 0..joints {
                let x = BODY.x + offset + brick * j as f64;
                lines.push(Seg {
                    from: Point::new(x, y1),
                    to: Point::new(x, y1 + row_height),
                });
            }
        }
        lines
    }
}

pub mod storage_array {
    use super::{Seg, seg};
    use crate::geometry::Rect;
    pub const WIDTH: f64 = 48.0;
    pub const DISKS: [Rect; 3] = [
        Rect::new(1.0, 1.0, 46.0, 8.0),
        Rect::new(1.0, 12.0, 46.0, 8.0),
        Rect::new(1.0, 23.0, 46.0, 8.0),
    ];
    pub const CORNER: f64 = 2.0;
    pub const CONNECTION: [Seg; 2] = [seg(24.0, 31.0, 24.0, 39.0), seg(14.0, 39.0, 34.0, 39.0)];
}

pub mod database {
    use crate::geometry::Point;
    pub const WIDTH: f64 = 48.0;
    pub const CYLINDER_SIDE: &str = "M6 7 V33 A18 6 0 0 0 42 33 V7";
    pub const CYLINDER_TOP_CENTER: Point = Point::new(24.0, 7.0);
    pub const CYLINDER_TOP_RX: f64 = 18.0;
    pub const CYLINDER_TOP_RY: f64 = 6.0;
    /// Where the abbreviation sits: centred, with this baseline.
    pub const TEXT_AT: Point = Point::new(24.0, 29.0);
    pub const TEXT_SIZE: f64 = 9.0;
}

pub mod virtual_machine {
    pub const WIDTH: f64 = 40.0;
    pub const FACES: [&str; 3] = [
        "M20 1L38 10L20 19L2 10Z",
        "M2 10L20 19V39L2 30Z",
        "M38 10L20 19V39L38 30Z",
    ];
}

pub mod container_stack {
    use crate::geometry::Rect;
    pub const WIDTH: f64 = 68.0;
    pub const HULL: &str = "M1 27H42V7H50V27H67Q66 39 56 39H9Q2 39 1 27Z";
    pub const SQUARES: [Rect; 8] = [
        Rect::new(18.0, 11.0, 8.0, 8.0),
        Rect::new(26.0, 11.0, 8.0, 8.0),
        Rect::new(34.0, 11.0, 8.0, 8.0),
        Rect::new(10.0, 19.0, 8.0, 8.0),
        Rect::new(18.0, 19.0, 8.0, 8.0),
        Rect::new(26.0, 19.0, 8.0, 8.0),
        Rect::new(34.0, 19.0, 8.0, 8.0),
        Rect::new(50.0, 19.0, 8.0, 8.0),
    ];
}

pub mod load_balancer {
    use super::{Seg, seg};
    use crate::geometry::Point;
    pub const WIDTH: f64 = 40.0;
    pub const BODY_CENTER: Point = Point::new(20.0, 20.0);
    pub const BODY_RADIUS: f64 = 19.0;
    pub const DOT_RADIUS: f64 = 3.25;
    pub const INPUT: Point = Point::new(9.0, 20.0);
    pub const OUTPUTS: [Point; 3] = [
        Point::new(27.0, 10.0),
        Point::new(27.0, 20.0),
        Point::new(27.0, 30.0),
    ];
    pub const LINKS: [Seg; 3] = [
        seg(9.0, 20.0, 27.0, 10.0),
        seg(9.0, 20.0, 27.0, 20.0),
        seg(9.0, 20.0, 27.0, 30.0),
    ];
    pub const DETAIL_WIDTH: f64 = 1.5;
}

pub mod gateway {
    use crate::geometry::{Rect, fmt_num};
    pub const WIDTH: f64 = 58.0;
    pub const BODY: Rect = Rect::new(1.0, 1.0, 56.0, 38.0);
    pub const CORNER: f64 = 3.0;
    pub const DETAIL_WIDTH: f64 = 1.5;

    /// Each port's outline: a square slightly wider than tall, with a tab on top.
    pub fn ports() -> Vec<String> {
        [9.0, 23.0, 37.0]
            .into_iter()
            .map(|x: f64| {
                let y = 26.0;
                let (width, height) = (8.0, 6.0);
                let (tab_w, tab_h) = (3.0, 2.5);
                let tab_left = x + (width - tab_w) / 2.0;
                format!(
                    "M{} {}H{}V{}H{}V{}H{}V{}H{}Z",
                    fmt_num(x),
                    fmt_num(y),
                    fmt_num(tab_left),
                    fmt_num(y - tab_h),
                    fmt_num(tab_left + tab_w),
                    fmt_num(y),
                    fmt_num(x + width),
                    fmt_num(y + height),
                    fmt_num(x)
                )
            })
            .collect()
    }
}

impl PhysicalFigure {
    /// The figure's size, such as for placing its name under it.
    pub const fn size(self) -> Size {
        let width = match self {
            Self::User | Self::VirtualMachine | Self::LoadBalancer => 40.0,
            Self::Server => server::WIDTH,
            Self::DatabaseCylinder | Self::StorageArray => 48.0,
            Self::Firewall | Self::Gateway => 58.0,
            Self::ContainerStack => container_stack::WIDTH,
        };
        Size::new(width, FIGURE_HEIGHT)
    }
}

/// Where a figure is drawn in its element's area: centred at the top, with the name under it.
pub fn figure_rect(area: Rect, figure: PhysicalFigure) -> Rect {
    let size = figure.size();
    Rect::new(
        area.x + (area.width - size.width) / 2.0,
        area.y,
        size.width,
        size.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_path_matches_the_prototype() {
        let d = cloud_path(Rect::new(1.0, 1.0, 198.0, 98.0));
        assert!(d.starts_with("M50.5 30.4 C10.9 30.4 1 54.9 32.68 59.8"));
        assert!(d.ends_with(" Z"));
    }

    #[test]
    fn cloud_edge_point_moves_top_and_bottom_ends_straight_to_the_cloud() {
        let rect = Rect::new(10.0, 20.0, 200.0, 100.0);
        // The cloud's top dips about 16% below the box's top in the middle.
        let top = cloud_edge_point(rect, Side::Top, Point::new(110.0, 20.0));
        assert_eq!(top.x, 110.0);
        assert!((top.y - 35.7).abs() < 0.5, "{top:?}");
        // Its bottom touches the box in the middle.
        let bottom = cloud_edge_point(rect, Side::Bottom, Point::new(110.0, 120.0));
        assert!((bottom.y - 119.6).abs() < 0.5, "{bottom:?}");
    }

    #[test]
    fn cloud_edge_point_moves_left_and_right_ends_across_to_the_cloud() {
        let rect = Rect::new(10.0, 20.0, 200.0, 100.0);
        let left = cloud_edge_point(rect, Side::Left, Point::new(10.0, 70.0));
        assert_eq!(left.y, 70.0);
        assert!((left.x - 23.4).abs() < 0.5, "{left:?}");
        let right = cloud_edge_point(rect, Side::Right, Point::new(210.0, 70.0));
        assert!((right.x - 189.9).abs() < 0.5, "{right:?}");
        // A point beyond the cloud's ends stays where it is.
        let beyond = Point::new(12.0, 20.0);
        assert_eq!(cloud_edge_point(rect, Side::Top, beyond), beyond);
    }

    #[test]
    fn firewall_mortar_has_staggered_joints() {
        // 3 row lines, then 3 + 4 + 3 + 4 joints.
        assert_eq!(firewall::mortar().len(), 3 + 14);
    }

    #[test]
    fn gateway_ports_have_tabs() {
        assert_eq!(gateway::ports()[0], "M9 26H11.5V23.5H14.5V26H17V32H9Z");
    }
}
