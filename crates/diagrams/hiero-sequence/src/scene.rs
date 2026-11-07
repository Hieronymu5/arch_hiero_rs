//! How a sequence diagram is drawn: each participant in its colour, its lifeline running down
//! from it in the same colour with its activations on it, and the messages across between
//! the lifelines with their text. Also its draw.io file.

use hiero_core::color::readable_text;
use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{
    HEADING_GAP, HEADING_MIN_WIDTH, Heading, ORIGIN, Style, cell, figure_style, heading_cell,
    rect_attributes, wrap,
};
use hiero_core::export::svg::escape_xml;
use hiero_core::geometry::{Point, Rect, Size, fmt_num, round2};
use hiero_core::look::{
    ARROWS, BACKGROUND, DiagramColors, Look, LookElement, Shape, TEXT, draw_element,
};
use hiero_core::model::{ConnectionId, LineWeight};
use hiero_core::scene::{Anchor, Node, Paint, Scene, Tone};
use hiero_core::text::MeasureText;

use crate::model::{MessageKind, Sequence};
use crate::timeline::{
    LABEL_SIZE, LABEL_WEIGHT, Placed, SELF_HEIGHT, SELF_WIDTH, TOP, Timeline, colors_of,
    head_bottom, head_shape, is_figure, ordered, timeline,
};

/// A lifeline's dashes and gaps.
pub const LIFELINE_DASH: (f64, f64) = (6.0, 4.0);
/// A return's dashes and gaps.
pub const RETURN_DASH: (f64, f64) = (8.0, 4.0);
/// A message's line width, as every type's arrows are.
pub const MESSAGE_WIDTH: f64 = 2.0;
/// A message's head: how long it is, and half how wide.
pub const HEAD_LENGTH: f64 = 12.0;
pub const HEAD_HALF_WIDTH: f64 = 5.0;
/// The outline under a message's text, in the paper's colour, so it reads over lifelines.
const HALO: f64 = 3.0;

/// How a participant looks.
pub fn look_of(element: &Laid<Sequence>) -> LookElement {
    let colors = colors_of(element);
    LookElement {
        id: element.el.id.clone(),
        parent_id: None,
        label: element.el.label.clone(),
        description: None,
        look: Look {
            agent: element.el.props.agent,
            fill: Some(colors.fill.into()),
            border_color: Some(colors.border.into()),
            text_color: Some(readable_text(colors.fill).into()),
            ..Look::new(head_shape(element.el.kind, element.el.props.component_type))
        },
        rect: element.rect,
    }
}

/// The area a participant is drawn in: a figure's reaches down to the end of its name.
pub fn drawn_rect(eng: &Engine<Sequence>, element: &Laid<Sequence>) -> Rect {
    Rect {
        height: head_bottom(eng, element) - element.rect.y,
        ..element.rect
    }
}

/// The diagram's lifelines, activations, and messages, with the message `moving` drawn where
/// it's being dragged to.
pub fn timeline_of(
    eng: &Engine<Sequence>,
    layout: &[Laid<Sequence>],
    moving: Option<(&ConnectionId, f64)>,
) -> Timeline {
    timeline(eng, layout, &ordered(eng, layout, moving))
}

fn p(point: Point) -> String {
    format!("{} {}", fmt_num(round2(point.x)), fmt_num(round2(point.y)))
}

/// A message's head at `tip`, pointing along `dir` (1 for right, -1 for left), and where its
/// line ends: at the back of a filled head, or at an open one's tip.
fn head(tip: Point, dir: f64, filled: bool) -> (Node, Point) {
    let back = Point::new(tip.x - dir * HEAD_LENGTH, tip.y);
    let (top, bottom) = (
        Point::new(back.x, back.y - HEAD_HALF_WIDTH),
        Point::new(back.x, back.y + HEAD_HALF_WIDTH),
    );
    if filled {
        (
            Node::path(
                format!("M{}L{}L{}Z", p(tip), p(top), p(bottom)),
                Paint::fill_stroke(ARROWS, ARROWS, 1.0),
            ),
            back,
        )
    } else {
        (
            Node::path(
                format!("M{}L{}L{}", p(top), p(tip), p(bottom)),
                Paint {
                    fill: Some("none".into()),
                    ..Paint::stroke(ARROWS, MESSAGE_WIDTH)
                },
            ),
            tip,
        )
    }
}

/// The nodes that draw a message: its line, its head, and its text, over a box in the
/// paper's colour so lifelines it crosses don't show through it.
fn message_nodes(m: &Placed, measure: &dyn MeasureText) -> Vec<Node> {
    let filled = m.kind.is_none();
    let dash = (m.kind == Some(MessageKind::Return)).then_some(RETURN_DASH);
    let line_paint = Paint {
        fill: Some("none".into()),
        ..Paint::stroke(ARROWS, MESSAGE_WIDTH).dashed(dash)
    };
    let mut nodes = Vec::new();
    if m.to_self {
        let (head, end) = head(m.to, -1.0, filled);
        let right = m.from.x + SELF_WIDTH;
        nodes.push(Node::path(
            format!(
                "M{}H{}V{}H{}",
                p(m.from),
                fmt_num(round2(right)),
                fmt_num(round2(m.to.y)),
                fmt_num(round2(end.x))
            ),
            line_paint,
        ));
        nodes.push(head);
    } else {
        let dir = if m.to.x >= m.from.x { 1.0 } else { -1.0 };
        let (head, end) = head(m.to, dir, filled);
        nodes.push(Node::line(m.from, end, line_paint));
        nodes.push(head);
    }
    if let Some(label) = &m.label {
        let (at, centred) = m.label_at();
        let width = measure.width(label, LABEL_SIZE, LABEL_WEIGHT);
        let left = if centred { at.x - width / 2.0 } else { at.x };
        nodes.push(Node::rect(
            Rect::new(
                round2(left - 2.0),
                round2(at.y - LABEL_SIZE),
                round2(width + 4.0),
                round2(LABEL_SIZE + 4.0),
            ),
            0.0,
            Paint::fill(BACKGROUND).toned(Tone::Paper),
        ));
        nodes.push(Node::Text {
            at: Point::new(round2(at.x), round2(at.y)),
            anchor: if centred {
                Anchor::Middle
            } else {
                Anchor::Start
            },
            size: LABEL_SIZE,
            weight: LABEL_WEIGHT,
            text: label.clone(),
            fill: None,
            halo: Some((BACKGROUND.into(), HALO)),
        });
    }
    nodes
}

/// The area a message covers, its text too.
fn message_rect(eng: &Engine<Sequence>, m: &Placed) -> Rect {
    let width = m
        .label
        .as_deref()
        .map_or(0.0, |l| eng.measure().width(l, LABEL_SIZE, LABEL_WEIGHT));
    let (at, centred) = m.label_at();
    let (left, right) = if centred {
        (at.x - width / 2.0, at.x + width / 2.0)
    } else {
        (at.x, at.x + width)
    };
    let left = left.min(m.from.x).min(m.to.x);
    let right = right.max(m.from.x).max(m.to.x).max(if m.to_self {
        m.from.x + SELF_WIDTH
    } else {
        0.0
    });
    let top = at.y - LABEL_SIZE;
    let bottom = if m.to_self {
        m.from.y + SELF_HEIGHT
    } else {
        m.from.y
    } + HEAD_HALF_WIDTH;
    Rect::new(left, top, right - left, bottom - top)
}

/// The area everything is drawn in.
fn bounds(eng: &Engine<Sequence>, layout: &[Laid<Sequence>], timeline: &Timeline) -> Rect {
    let rects: Vec<Rect> = layout
        .iter()
        .map(|e| drawn_rect(eng, e))
        .chain(timeline.messages.iter().map(|m| message_rect(eng, m)))
        .chain(timeline.bars.iter().map(|b| b.rect))
        .collect();
    if rects.is_empty() {
        return Rect::default();
    }
    let left = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
    let top = rects.iter().map(|r| r.y).fold(TOP, f64::min);
    let right = rects
        .iter()
        .map(Rect::right)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = rects
        .iter()
        .map(Rect::bottom)
        .fold(timeline.bottom(), f64::max);
    Rect::new(left, top, right - left, bottom - top)
}

/// The diagram drawn from `layout`, with the message `moving` where it's being dragged to.
pub fn scene_of(
    eng: &Engine<Sequence>,
    layout: &[Laid<Sequence>],
    moving: Option<(&ConnectionId, f64)>,
) -> Scene {
    let timeline = timeline_of(eng, layout, moving);
    let weight = eng.line_weight();
    let mut nodes = Vec::new();
    for l in &timeline.lifelines {
        nodes.push(Node::line(
            Point::new(round2(l.x), round2(l.top)),
            Point::new(round2(l.x), round2(l.bottom)),
            Paint::stroke(l.colors.border, weight.width()).dashed(Some(LIFELINE_DASH)),
        ));
    }
    for b in &timeline.bars {
        nodes.push(Node::rect(
            b.rect,
            0.0,
            Paint::fill_stroke(b.colors.fill, b.colors.border, weight.width()),
        ));
    }
    let colors = DiagramColors::default();
    for e in layout {
        nodes.extend(draw_element(&look_of(e), eng.measure(), &colors, weight));
    }
    for m in &timeline.messages {
        nodes.extend(message_nodes(m, eng.measure()));
    }
    Scene {
        nodes,
        bounds: bounds(eng, layout, &timeline),
        text_color: TEXT.into(),
        marker_color: ARROWS.into(),
        background: BACKGROUND.into(),
    }
}

/// The diagram as it's stored, drawn.
pub fn scene(eng: &Engine<Sequence>) -> Scene {
    scene_of(eng, eng.layout(), None)
}

/// An edge between two points, not attached to any cell, through `points`, with `value` (already
/// escaped) moved by `offset` from its usual place.
fn edge(
    id: &str,
    value: &str,
    style: &Style,
    ends: (Point, Point),
    points: &[Point],
    offset: Option<Point>,
) -> String {
    let point = |p: Point, role: &str| {
        format!(
            r#"<mxPoint x="{}" y="{}"{role} />"#,
            fmt_num(round2(p.x)),
            fmt_num(round2(p.y))
        )
    };
    let mut inner = vec![
        point(ends.0, r#" as="sourcePoint""#),
        point(ends.1, r#" as="targetPoint""#),
    ];
    if !points.is_empty() {
        inner.push(format!(
            r#"<Array as="points">{}</Array>"#,
            points.iter().map(|&p| point(p, "")).collect::<String>()
        ));
    }
    if let Some(o) = offset {
        inner.push(point(o, r#" as="offset""#));
    }
    format!(
        "        <mxCell id=\"{}\" value=\"{value}\" style=\"{}\" edge=\"1\" parent=\"1\">\n          <mxGeometry relative=\"1\" as=\"geometry\">{}</mxGeometry>\n        </mxCell>",
        escape_xml(id),
        style.build(),
        inner.concat()
    )
}

/// The diagram as a draw.io file, with `heading` above it: the participants, their lifelines
/// and activations, and the messages as lines between points on the lifelines.
pub fn to_drawio(eng: &Engine<Sequence>, title: &str, heading: Option<&Heading>) -> String {
    let layout = eng.layout();
    let timeline = timeline_of(eng, layout, None);
    let area = bounds(eng, layout, &timeline);
    let weight: LineWeight = eng.line_weight();
    let heading = heading.map(|h| heading_cell(h, area.width.max(HEADING_MIN_WIDTH)));
    let shift = Point::new(
        ORIGIN - area.x,
        ORIGIN + heading.as_ref().map_or(0.0, |(_, h)| h + HEADING_GAP) - area.y,
    );
    let at = |q: Point| Point::new(q.x + shift.x, q.y + shift.y);
    let moved = |r: Rect| Rect {
        x: r.x + shift.x,
        y: r.y + shift.y,
        ..r
    };
    let mut cells = Vec::new();
    if let Some((xml, _)) = &heading {
        cells.push(xml.clone());
    }
    for l in &timeline.lifelines {
        let style = Style::new()
            .set("html", 1)
            .set("endArrow", "none")
            .set("dashed", 1)
            .set(
                "dashPattern",
                format!("{} {}", fmt_num(LIFELINE_DASH.0), fmt_num(LIFELINE_DASH.1)),
            )
            .set("strokeColor", l.colors.border)
            .num("strokeWidth", weight.width());
        cells.push(edge(
            &format!("{}-lifeline", l.id),
            "",
            &style,
            (at(Point::new(l.x, l.top)), at(Point::new(l.x, l.bottom))),
            &[],
            None,
        ));
    }
    for (i, b) in timeline.bars.iter().enumerate() {
        let style = Style::new()
            .set("html", 1)
            .set("rounded", 0)
            .set("fillColor", b.colors.fill)
            .set("strokeColor", b.colors.border)
            .num("strokeWidth", weight.width());
        cells.push(cell(
            &format!("{}-activation-{i}", b.participant),
            "",
            &style.build(),
            "1",
            &rect_attributes(moved(b.rect)),
        ));
    }
    let colors = DiagramColors::default();
    for e in layout {
        let look = look_of(e);
        let geometry = rect_attributes(moved(e.rect));
        let (value, style) = if is_figure(look.look.shape) {
            (
                escape_xml(&look.label),
                figure_style(&look.look, &colors, weight),
            )
        } else {
            debug_assert_eq!(look.look.shape, Shape::Box);
            (
                escape_xml(&format!("<b>{}</b>", escape_xml(&look.label))),
                Style::new()
                    .set("rounded", 1)
                    .set("absoluteArcSize", 1)
                    .set("arcSize", 12)
                    .set("html", 1)
                    .set("whiteSpace", "wrap")
                    .set("fillColor", look.look.fill.clone().unwrap_or_default())
                    .set(
                        "strokeColor",
                        look.look.border_color.clone().unwrap_or_default(),
                    )
                    .num("strokeWidth", weight.width())
                    .set(
                        "fontColor",
                        look.look.text_color.clone().unwrap_or_default(),
                    )
                    .set("fontSize", 16)
                    .set("spacingLeft", 10)
                    .set("spacingRight", 10),
            )
        };
        cells.push(cell(
            e.id().as_str(),
            &value,
            &style.build(),
            "1",
            &geometry,
        ));
    }
    for m in &timeline.messages {
        let mut style = Style::new()
            .set("html", 1)
            .set("rounded", 0)
            .set("strokeColor", ARROWS)
            .num("strokeWidth", MESSAGE_WIDTH)
            .set("fontSize", 13)
            .set("fontColor", TEXT)
            .set("labelBackgroundColor", BACKGROUND);
        style = match m.kind {
            None => style.set("endArrow", "block").set("endFill", 1),
            Some(MessageKind::Async) => style.set("endArrow", "open").set("endFill", 0),
            Some(MessageKind::Return) => style
                .set("endArrow", "open")
                .set("endFill", 0)
                .set("dashed", 1)
                .set(
                    "dashPattern",
                    format!("{} {}", fmt_num(RETURN_DASH.0), fmt_num(RETURN_DASH.1)),
                ),
        };
        let value = m
            .label
            .as_deref()
            .map(|l| escape_xml(&escape_xml(l)))
            .unwrap_or_default();
        let (points, offset) = if m.to_self {
            let right = m.from.x + SELF_WIDTH;
            (
                vec![
                    at(Point::new(right, m.from.y)),
                    at(Point::new(right, m.to.y)),
                ],
                None,
            )
        } else {
            (Vec::new(), Some(Point::new(0.0, -(LABEL_SIZE / 2.0 + 6.0))))
        };
        if m.to_self {
            style = style
                .set("align", "left")
                .num("spacingLeft", SELF_WIDTH / 2.0 + 6.0);
        }
        cells.push(edge(
            m.id.as_str(),
            &value,
            &style,
            (at(m.from), at(m.to)),
            &points,
            offset,
        ));
    }
    let height = area.height + heading.as_ref().map_or(0.0, |(_, h)| h + HEADING_GAP);
    wrap(title, Size::new(area.width, height), &cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ParticipantColor, SequenceMessage};
    use crate::test_engine as engine;
    use crate::timeline::{BAR_WIDTH, FIRST_MESSAGE, STEP};
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::model::ElementId;
    use hiero_core::palette::{BLUE, GREEN};

    fn message(
        eng: &mut Engine<Sequence>,
        from: &str,
        to: &str,
        kind: Option<MessageKind>,
        label: &str,
    ) {
        let id = eng.add_connection(&from.into(), &to.into()).unwrap();
        eng.set_connection_props(
            &id,
            SequenceMessage {
                kind,
                label: Some(label.into()),
            },
        );
    }

    /// A user, an application, and a database, with a call answered and a call left open.
    fn ordering() -> Engine<Sequence> {
        let mut eng = engine();
        eng.add_at_free_position(crate::model::SequenceKind::Component);
        message(&mut eng, "id1", "id2", None, "Place order");
        message(&mut eng, "id2", "id3", None, "Save order");
        message(
            &mut eng,
            "id3",
            "id2",
            Some(MessageKind::Return),
            "Order id",
        );
        message(&mut eng, "id2", "id2", None, "Send email");
        message(
            &mut eng,
            "id2",
            "id1",
            Some(MessageKind::Async),
            "Confirmation",
        );
        eng
    }

    #[test]
    fn messages_go_down_in_order_between_the_lifelines() {
        let eng = ordering();
        let t = timeline_of(&eng, eng.layout(), None);
        let heads = eng
            .layout()
            .iter()
            .map(|e| head_bottom(&eng, e))
            .fold(0.0, f64::max);
        let ys: Vec<f64> = t.messages.iter().map(Placed::y).collect();
        let first = heads + FIRST_MESSAGE;
        assert_eq!(
            ys,
            [
                first,
                first + STEP,
                first + STEP * 2.0,
                first + STEP * 3.0,
                first + STEP * 4.0 + SELF_HEIGHT
            ]
        );
        let x = |id: &str| eng.laid(&ElementId::from(id)).unwrap().rect.center().x;
        // The call into the application ends on its activation's left side.
        assert_eq!(t.messages[0].from.x, x("id1"));
        assert_eq!(t.messages[0].to.x, x("id2") - BAR_WIDTH / 2.0);
        // The database answers from its activation, which ends there.
        let db_bar = t
            .bars
            .iter()
            .find(|b| b.participant.as_str() == "id3")
            .unwrap();
        assert_eq!(db_bar.rect.y, ys[1]);
        assert_eq!(db_bar.rect.bottom(), ys[2]);
        // The application's call is never answered: its activation lasts to its last message.
        let app_bar = t
            .bars
            .iter()
            .find(|b| b.participant.as_str() == "id2")
            .unwrap();
        assert_eq!(app_bar.rect.y, ys[0]);
        assert_eq!(app_bar.rect.bottom(), ys[4]);
        // The message to itself loops out from the activation's right side.
        assert!(t.messages[3].to_self);
        assert_eq!(t.messages[3].from.x, x("id2") + BAR_WIDTH / 2.0);
        assert!(t.lifelines.iter().all(|l| l.bottom > ys[4]));
    }

    #[test]
    fn a_dragged_message_goes_before_the_first_one_below_it() {
        let eng = ordering();
        let last = eng.connections()[4].id.clone();
        let t = timeline_of(&eng, eng.layout(), None);
        let between_first_two = f64::midpoint(t.messages[0].y(), t.messages[1].y());
        let order = ordered(&eng, eng.layout(), Some((&last, between_first_two)));
        assert_eq!(order[1].id, last);
        assert_eq!(
            crate::timeline::index_at(&eng, eng.layout(), &last, between_first_two),
            1
        );
    }

    #[test]
    fn draws_participants_and_lifelines_in_their_colour() {
        let mut eng = ordering();
        eng.replace(&"id2".into(), |mut e| {
            e.props.color = Some(ParticipantColor::Green);
            e
        });
        let svg = scene_to_svg("Test", &scene(&eng));
        assert!(svg.contains(&format!(r#"stroke="{}""#, GREEN.border)));
        assert!(svg.contains(&format!(r#"fill="{}""#, GREEN.fill)));
        assert!(svg.contains(&format!(r#"stroke="{}""#, BLUE.border)));
        assert!(svg.contains("Place order"));
        assert!(svg.contains("stroke-dasharray"));
        let drawio = to_drawio(
            &eng,
            "Test",
            Some(&Heading {
                name: "Orders".into(),
                description: None,
            }),
        );
        assert!(drawio.contains("Place order"));
        assert!(drawio.contains("endArrow=open"));
        assert!(drawio.contains("id2-lifeline"));
        assert!(drawio.contains(GREEN.fill));
    }
}
