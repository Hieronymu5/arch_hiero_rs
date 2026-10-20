//! Where arrows run and where their text and boxes go. A port of the prototype's
//! `utils/diagram-connections.ts`.

use std::collections::HashMap;

use crate::geometry::{Point, Rect, round2};
use crate::model::{ConnectionId, ElementId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    pub const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Whether the side runs up and down, so arrows meeting it run across.
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// The ends of an arrow, by element id, and where it was moved to by hand, if it was.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionEnds {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub manual_offset: Option<f64>,
}

/// Where an arrow is drawn: from a point on the source's side to a point on the target's side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    pub from: Point,
    pub to: Point,
    pub from_side: Side,
    pub to_side: Side,
    /// Whether the arrow runs straight at a position set by hand.
    pub placed_by_hand: bool,
}

/// How far a straight arrow stays from the corners of the sides it joins.
pub const STRAIGHT_MARGIN: f64 = 10.0;

/// Where a straight arrow can run between two elements, along their facing sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StraightRange {
    pub side: Side,
    pub start: f64,
    pub end: f64,
    pub source_start: f64,
}

/// The stretch, along two elements' facing sides, where the sides overlap, less a margin at
/// each corner. Coordinates are y for arrows between left and right sides, and x for arrows
/// between top and bottom sides.
pub fn straight_range(source: &Rect, target: &Rect) -> Option<StraightRange> {
    let side = facing_side(source, target);
    let (a, a_size, b, b_size) = if side.is_vertical() {
        (source.y, source.height, target.y, target.height)
    } else {
        (source.x, source.width, target.x, target.width)
    };
    let start = a.max(b) + STRAIGHT_MARGIN;
    let end = (a + a_size).min(b + b_size) - STRAIGHT_MARGIN;
    (end >= start).then_some(StraightRange {
        side,
        start,
        end,
        source_start: a,
    })
}

/// The side of `source` that faces `target`. Offsets are compared relative to the two boxes'
/// combined size, so wide boxes side by side still connect left to right.
pub fn facing_side(source: &Rect, target: &Rect) -> Side {
    let a = source.center();
    let b = target.center();
    let dx = (b.x - a.x) / (source.width + target.width);
    let dy = (b.y - a.y) / (source.height + target.height);
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { Side::Right } else { Side::Left }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

fn point_on_side(rect: &Rect, side: Side, fraction: f64) -> Point {
    match side {
        Side::Top => Point::new(rect.x + rect.width * fraction, rect.y),
        Side::Bottom => Point::new(rect.x + rect.width * fraction, rect.bottom()),
        Side::Left => Point::new(rect.x, rect.y + rect.height * fraction),
        Side::Right => Point::new(rect.right(), rect.y + rect.height * fraction),
    }
}

/// Routes every arrow between the facing sides of its two elements. An arrow moved by hand
/// runs straight at its chosen position, kept within `straight_range`. The other arrows meeting
/// one side of an element are spread evenly along it at 1/(n+1), 2/(n+1), …, ordered by where
/// their other end is, so they don't cross. Arrows whose elements are missing, or that connect
/// an element to itself, aren't routed.
pub fn route_connections(
    rects: &HashMap<ElementId, Rect>,
    connections: &[ConnectionEnds],
) -> HashMap<ConnectionId, Route> {
    #[derive(Clone)]
    struct Attachment {
        connection: ConnectionId,
        from: bool,
        element: ElementId,
        side: Side,
        sort_key: f64,
    }

    let mut attachments: Vec<Attachment> = Vec::new();
    let mut manual: HashMap<ConnectionId, Route> = HashMap::new();
    for c in connections {
        let (Some(source), Some(target)) = (rects.get(&c.source_id), rects.get(&c.target_id))
        else {
            continue;
        };
        if c.source_id == c.target_id {
            continue;
        }
        let range = straight_range(source, target);
        if let (Some(offset), Some(range)) = (c.manual_offset, range) {
            let at = (range.source_start + offset)
                .max(range.start)
                .min(range.end);
            let to_side = range.side.opposite();
            let point = |rect: &Rect, side: Side| match side {
                Side::Right => Point::new(rect.right(), at),
                Side::Left => Point::new(rect.x, at),
                Side::Bottom => Point::new(at, rect.bottom()),
                Side::Top => Point::new(at, rect.y),
            };
            manual.insert(
                c.id.clone(),
                Route {
                    from: point(source, range.side),
                    to: point(target, to_side),
                    from_side: range.side,
                    to_side,
                    placed_by_hand: true,
                },
            );
            continue;
        }
        let from_side = facing_side(source, target);
        let to_side = from_side.opposite();
        let horizontal = !from_side.is_vertical();
        let along = |rect: &Rect| {
            if horizontal {
                rect.center().x
            } else {
                rect.center().y
            }
        };
        attachments.push(Attachment {
            connection: c.id.clone(),
            from: true,
            element: c.source_id.clone(),
            side: from_side,
            sort_key: along(target),
        });
        attachments.push(Attachment {
            connection: c.id.clone(),
            from: false,
            element: c.target_id.clone(),
            side: to_side,
            sort_key: along(source),
        });
    }

    // Spread the attachments on each side of each element evenly along it, keeping the order
    // the groups were first met in.
    let mut groups: Vec<((ElementId, Side), Vec<Attachment>)> = Vec::new();
    for attachment in attachments {
        let key = (attachment.element.clone(), attachment.side);
        if let Some((_, group)) = groups.iter_mut().find(|(k, _)| *k == key) {
            group.push(attachment);
        } else {
            groups.push((key, vec![attachment]));
        }
    }
    let mut points: HashMap<(ConnectionId, bool), Point> = HashMap::new();
    for (_, mut group) in groups {
        group.sort_by(|a, b| {
            a.sort_key
                .partial_cmp(&b.sort_key)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.connection.cmp(&b.connection))
        });
        let count = group.len() as f64;
        for (index, attachment) in group.iter().enumerate() {
            let rect = &rects[&attachment.element];
            let fraction = (index as f64 + 1.0) / (count + 1.0);
            points.insert(
                (attachment.connection.clone(), attachment.from),
                point_on_side(rect, attachment.side, fraction),
            );
        }
    }

    let mut routes = HashMap::new();
    for c in connections {
        if let Some(route) = manual.get(&c.id) {
            routes.insert(c.id.clone(), *route);
            continue;
        }
        let from = points.get(&(c.id.clone(), true));
        let to = points.get(&(c.id.clone(), false));
        let (Some(from), Some(to), Some(source), Some(target)) =
            (from, to, rects.get(&c.source_id), rects.get(&c.target_id))
        else {
            continue;
        };
        let from_side = facing_side(source, target);
        routes.insert(
            c.id.clone(),
            Route {
                from: *from,
                to: *to,
                from_side,
                to_side: from_side.opposite(),
                placed_by_hand: false,
            },
        );
    }
    routes
}

/// How far an arrow's text sits from where the arrow starts: along the line, and off its side.
pub const LABEL_ALONG: f64 = 3.0;
pub const LABEL_ACROSS: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vertical {
    /// The text hangs below the anchor.
    Top,
    /// The text sits above the anchor.
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelPlacement {
    pub anchor: Point,
    pub align: Align,
    pub vertical: Vertical,
}

/// Where an arrow's text goes: near the start of the arrow, on the left-hand side of its
/// direction of travel.
pub fn label_placement(from: Point, to: Point) -> LabelPlacement {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dy.abs() > dx.abs() {
        return if dy > 0.0 {
            LabelPlacement {
                anchor: Point::new(from.x + LABEL_ACROSS, from.y + LABEL_ALONG),
                align: Align::Start,
                vertical: Vertical::Top,
            }
        } else {
            LabelPlacement {
                anchor: Point::new(from.x - LABEL_ACROSS, from.y - LABEL_ALONG),
                align: Align::End,
                vertical: Vertical::Bottom,
            }
        };
    }
    if dx >= 0.0 {
        LabelPlacement {
            anchor: Point::new(from.x + LABEL_ALONG, from.y - LABEL_ACROSS),
            align: Align::Start,
            vertical: Vertical::Bottom,
        }
    } else {
        LabelPlacement {
            anchor: Point::new(from.x - LABEL_ALONG, from.y + LABEL_ACROSS),
            align: Align::End,
            vertical: Vertical::Top,
        }
    }
}

/// The x, baseline y, and alignment that draw text of `font_size` at a placement.
pub fn label_text_position(placement: LabelPlacement, font_size: f64) -> (Point, Align) {
    let y = match placement.vertical {
        Vertical::Top => placement.anchor.y + font_size * 0.8,
        Vertical::Bottom => placement.anchor.y - font_size * 0.2,
    };
    (Point::new(placement.anchor.x, y), placement.align)
}

/// The small return arrow of a bi-directional arrow: where it starts, how long, how far off.
pub const RETURN_ARROW_ALONG: f64 = 3.0;
pub const RETURN_ARROW_LENGTH: f64 = 12.0;
pub const RETURN_ARROW_ACROSS: f64 = 7.0;

/// Where a bi-directional arrow's small return arrow goes: beside the start of the line, on
/// the right-hand side of its direction of travel, pointing back towards the source. There's
/// none when the line is too short to fit it.
pub fn return_arrow_placement(from: Point, to: Point) -> Option<(Point, Point)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = dx.hypot(dy);
    if length < RETURN_ARROW_ALONG + RETURN_ARROW_LENGTH + 16.0 {
        return None;
    }
    let unit = Point::new(dx / length, dy / length);
    let right = Point::new(-unit.y, unit.x);
    let at = |along: f64| {
        Point::new(
            round2(from.x + unit.x * along + right.x * RETURN_ARROW_ACROSS),
            round2(from.y + unit.y * along + right.y * RETURN_ARROW_ACROSS),
        )
    };
    Some((
        at(RETURN_ARROW_ALONG + RETURN_ARROW_LENGTH),
        at(RETURN_ARROW_ALONG),
    ))
}

/// A connector box's height, text size, padding beside its text, and narrowest width.
pub const CONNECTOR_BOX_HEIGHT: f64 = 20.0;
pub const CONNECTOR_BOX_FONT_SIZE: f64 = 12.0;
pub const CONNECTOR_BOX_PADDING_X: f64 = 6.0;
pub const CONNECTOR_BOX_MIN_WIDTH: f64 = 32.0;

/// Where an arrow's connector box goes, such as "API": against the side of the target the
/// arrow meets, centred on where the arrow met it. The arrow then ends at the box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectorPlacement {
    pub rect: Rect,
    pub arrow_end: Point,
    pub text: Point,
}

pub fn connector_placement(to: Point, to_side: Side, text_width: f64) -> ConnectorPlacement {
    let height = CONNECTOR_BOX_HEIGHT;
    let width = CONNECTOR_BOX_MIN_WIDTH.max(text_width.ceil() + CONNECTOR_BOX_PADDING_X * 2.0);
    let rect = match to_side {
        Side::Left => Rect::new(to.x - width, to.y - height / 2.0, width, height),
        Side::Right => Rect::new(to.x, to.y - height / 2.0, width, height),
        Side::Top => Rect::new(to.x - width / 2.0, to.y - height, width, height),
        Side::Bottom => Rect::new(to.x - width / 2.0, to.y, width, height),
    };
    let arrow_end = match to_side {
        Side::Left => Point::new(rect.x, to.y),
        Side::Right => Point::new(rect.x + width, to.y),
        Side::Top => Point::new(to.x, rect.y),
        Side::Bottom => Point::new(to.x, rect.y + height),
    };
    let rounded = Rect {
        x: round2(rect.x),
        y: round2(rect.y),
        ..rect
    };
    ConnectorPlacement {
        rect: rounded,
        arrow_end,
        text: connector_text_position(&rounded),
    }
}

/// Where a connector box's text goes, centred in it: its x and its baseline's y.
pub fn connector_text_position(rect: &Rect) -> Point {
    Point::new(
        round2(rect.x + rect.width / 2.0),
        round2(rect.y + rect.height / 2.0 + CONNECTOR_BOX_FONT_SIZE * 0.35),
    )
}

/// The width of an arrow tag's border, as for connector and transformation boxes.
pub const ARROW_TAG_BORDER_WIDTH: f64 = 1.5;

/// Where an arrow's transformation box goes, centred on the middle of the line.
pub fn transformation_placement(from: Point, to: Point, text_width: f64) -> (Rect, Point) {
    let height = CONNECTOR_BOX_HEIGHT;
    let width = CONNECTOR_BOX_MIN_WIDTH.max(text_width.ceil() + CONNECTOR_BOX_PADDING_X * 2.0);
    let rect = Rect::new(
        round2((from.x + to.x) / 2.0 - width / 2.0),
        round2((from.y + to.y) / 2.0 - height / 2.0),
        width,
        height,
    );
    (rect, connector_text_position(&rect))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ends(id: &str, source: &str, target: &str) -> ConnectionEnds {
        ConnectionEnds {
            id: id.into(),
            source_id: source.into(),
            target_id: target.into(),
            manual_offset: None,
        }
    }

    #[test]
    fn facing_sides() {
        let a = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(
            facing_side(&a, &Rect::new(300.0, 0.0, 100.0, 50.0)),
            Side::Right
        );
        assert_eq!(
            facing_side(&a, &Rect::new(0.0, 300.0, 100.0, 50.0)),
            Side::Bottom
        );
        assert_eq!(
            facing_side(&a, &Rect::new(-300.0, 10.0, 100.0, 50.0)),
            Side::Left
        );
        assert_eq!(
            facing_side(&a, &Rect::new(10.0, -300.0, 100.0, 50.0)),
            Side::Top
        );
    }

    #[test]
    fn spreads_arrows_on_a_side() {
        let rects: HashMap<ElementId, Rect> = [
            ("a".into(), Rect::new(0.0, 0.0, 100.0, 90.0)),
            ("b".into(), Rect::new(300.0, -100.0, 100.0, 90.0)),
            ("c".into(), Rect::new(300.0, 100.0, 100.0, 90.0)),
        ]
        .into();
        let routes = route_connections(&rects, &[ends("1", "a", "b"), ends("2", "a", "c")]);
        assert_eq!(
            routes[&ConnectionId::from("1")].from,
            Point::new(100.0, 30.0)
        );
        assert_eq!(
            routes[&ConnectionId::from("2")].from,
            Point::new(100.0, 60.0)
        );
        assert_eq!(
            routes[&ConnectionId::from("1")].to,
            Point::new(300.0, -55.0)
        );
    }

    #[test]
    fn manual_offsets_run_straight() {
        let rects: HashMap<ElementId, Rect> = [
            ("a".into(), Rect::new(0.0, 0.0, 100.0, 100.0)),
            ("b".into(), Rect::new(300.0, 40.0, 100.0, 100.0)),
        ]
        .into();
        let mut c = ends("1", "a", "b");
        c.manual_offset = Some(70.0);
        let route = route_connections(&rects, &[c])[&ConnectionId::from("1")];
        assert!(route.placed_by_hand);
        assert_eq!(route.from, Point::new(100.0, 70.0));
        assert_eq!(route.to, Point::new(300.0, 70.0));
        let range = straight_range(&rects[&"a".into()], &rects[&"b".into()]).unwrap();
        assert_eq!((range.start, range.end), (50.0, 90.0));
    }

    #[test]
    fn self_arrows_are_not_routed() {
        let rects: HashMap<ElementId, Rect> =
            [("a".into(), Rect::new(0.0, 0.0, 10.0, 10.0))].into();
        assert!(route_connections(&rects, &[ends("1", "a", "a")]).is_empty());
    }

    #[test]
    fn connector_box_sits_before_the_target() {
        let p = connector_placement(Point::new(300.0, 50.0), Side::Left, 20.0);
        assert_eq!(p.rect, Rect::new(268.0, 40.0, 32.0, 20.0));
        assert_eq!(p.arrow_end, Point::new(268.0, 50.0));
        assert_eq!(p.text, Point::new(284.0, 54.2));
    }

    #[test]
    fn labels_sit_left_of_travel() {
        let p = label_placement(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        assert_eq!(p.anchor, Point::new(3.0, -6.0));
        assert_eq!(p.vertical, Vertical::Bottom);
        assert!(return_arrow_placement(Point::new(0.0, 0.0), Point::new(10.0, 0.0)).is_none());
        let (a, b) = return_arrow_placement(Point::new(0.0, 0.0), Point::new(100.0, 0.0)).unwrap();
        assert_eq!((a, b), (Point::new(15.0, 7.0), Point::new(3.0, 7.0)));
    }
}
