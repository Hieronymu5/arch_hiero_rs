//! How a high level conceptual diagram is drawn: groups and applications as rounded boxes in
//! their colours, solid or dashed, with their names in their own font size, and the arrows as
//! wide block arrows between them. Also its draw.io file.

use std::collections::HashMap;

use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{
    HEADING_GAP, HEADING_MIN_WIDTH, Heading, ORIGIN, Style, cell, constraint, heading_cell,
    person_shape, rect_attributes, wrap,
};
use hiero_core::export::svg::escape_xml;
use hiero_core::figures::{actor, actors};
use hiero_core::geometry::{Point, Rect, Size, fmt_num, round2};
use hiero_core::look::{BACKGROUND, PADDING_X, TEXT, actors_figure, text_lines};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::routing::{ConnectionEnds, route_connections};
use hiero_core::scene::{Anchor, Node, Paint, Scene};
use hiero_core::text::{MeasureText, TextStyle, Weight, wrap_text};

use crate::model::{FONT_SIZE_MIN, HighLevel, HighLevelKind};
use crate::rules::{NAME_PADDING, line_height};

/// The corner radius of applications and groups.
pub const RADIUS: f64 = 8.0;
/// A dashed border's dashes and gaps.
pub const DASH: (f64, f64) = (8.0, 4.0);

/// An arrow's shaft is this wide, and its head this wide and this long.
pub const SHAFT_WIDTH: f64 = 16.0;
pub const HEAD_WIDTH: f64 = 36.0;
pub const HEAD_LENGTH: f64 = 18.0;
/// The space an arrow leaves between its ends and the elements it joins.
pub const ARROW_GAP: f64 = 6.0;
/// An arrow's fill and outline: light enough not to compete with what it joins.
pub const ARROW_FILL: &str = "#d9d9d9";
pub const ARROW_BORDER: &str = "#8c8c8c";

/// A description's size, as a share of its name's.
pub const DESCRIPTION_SCALE: f64 = 0.85;
/// The space between a name and the description under it.
pub const DESCRIPTION_GAP: f64 = 2.0;

/// Where an element's name sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamePlace {
    /// On one line across the top, in a group's header, above what it holds.
    Top,
    /// Wrapped, in the middle: an application's, or an empty group's.
    Middle,
    /// Wrapped, under the figures: actors'.
    Under,
}

/// How far below the top of their area actors' figures start, and how far below the figures
/// their name starts.
pub const ACTORS_TOP: f64 = 4.0;
pub const ACTORS_NAME_GAP: f64 = 6.0;

/// Where the name of `element` sits.
pub fn name_place(element: &Laid<HighLevel>) -> NamePlace {
    match element.el.kind {
        HighLevelKind::Group if element.child_count > 0 => NamePlace::Top,
        HighLevelKind::Actors => NamePlace::Under,
        _ => NamePlace::Middle,
    }
}

/// The text style of an element's name: bold, in its own size.
pub fn name_style(element: &Laid<HighLevel>) -> TextStyle {
    let size = f64::from(element.el.props.font_size_of(element.el.kind));
    TextStyle::new(size, Weight::Bold, line_height(size), 1)
}

/// The text style of an application's description: regular, a little smaller than its name.
pub fn description_style(element: &Laid<HighLevel>) -> TextStyle {
    let size = description_size(f64::from(element.el.props.font_size_of(element.el.kind)));
    TextStyle::new(size, Weight::Regular, line_height(size), 1)
}

/// The size of a description under a name in `size`: as a conceptual diagram's 13 px under
/// its 16 px names, near enough, and never under the smallest font size.
pub fn description_size(size: f64) -> f64 {
    (size * DESCRIPTION_SCALE)
        .round()
        .max(f64::from(FONT_SIZE_MIN))
}

/// The description of `element`, if it can have one and has some text.
fn description_of(element: &Laid<HighLevel>) -> Option<&str> {
    if !element.info.has_description {
        return None;
    }
    element
        .el
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

/// An element's text as it's drawn: its name's lines, its description's under them, and the
/// top of the first.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementText {
    pub name: Vec<String>,
    pub description: Vec<String>,
    pub top: f64,
}

/// Where actors' name starts, under their figures, in their area `rect`.
fn names_top(rect: Rect) -> f64 {
    rect.y + ACTORS_TOP + actors::HEIGHT + ACTORS_NAME_GAP
}

/// Where actors' figures go in their area `rect`: centred across its top.
pub fn actors_at(rect: Rect) -> Point {
    Point::new(
        round2(rect.center().x - actors::WIDTH / 2.0),
        round2(rect.y + ACTORS_TOP),
    )
}

/// How many lines of `style` fit in `room`, at least `min`.
fn lines_in(room: f64, style: TextStyle, min: f64) -> usize {
    // At least `min` lines, however small the room.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lines = (room / style.line_height).floor().max(min) as usize;
    lines
}

/// The text of `element` as it's drawn: at the top of a group that holds something, under
/// actors' figures, and in the middle otherwise, with an application's or actors' description
/// under the name as far as it fits.
pub fn element_text(element: &Laid<HighLevel>, measure: &dyn MeasureText) -> ElementText {
    let rect = element.rect;
    let style = name_style(element);
    let width = (rect.width - PADDING_X * 2.0).max(1.0);
    match name_place(element) {
        NamePlace::Top => {
            let name = wrap_text(&element.el.label, width, style, measure);
            let top = rect.y + (element.header - style.line_height) / 2.0;
            ElementText {
                name,
                description: Vec::new(),
                top,
            }
        }
        NamePlace::Middle | NamePlace::Under => {
            let under = name_place(element) == NamePlace::Under;
            let room = if under {
                rect.bottom() - names_top(rect) - NAME_PADDING
            } else {
                rect.height - NAME_PADDING * 2.0
            };
            let name = wrap_text(
                &element.el.label,
                width,
                style.with_max_lines(lines_in(room, style, 1.0)),
                measure,
            );
            let mut height = name.len() as f64 * style.line_height;
            let mut description = Vec::new();
            if let Some(text) = description_of(element) {
                let small = description_style(element);
                let fits = lines_in(room - height - DESCRIPTION_GAP, small, 0.0);
                if fits > 0 {
                    description = wrap_text(text, width, small.with_max_lines(fits), measure);
                    height += DESCRIPTION_GAP + description.len() as f64 * small.line_height;
                }
            }
            ElementText {
                name,
                description,
                top: if under {
                    names_top(rect)
                } else {
                    rect.y + (rect.height - height) / 2.0
                },
            }
        }
    }
}

/// The nodes that draw an element: its box, its name, then any description.
fn element_nodes(element: &Laid<HighLevel>, measure: &dyn MeasureText, width: f64) -> Vec<Node> {
    let props = &element.el.props;
    let colors = props.color_of(element.el.kind).colors();
    let paint = Paint::fill_stroke(colors.fill, colors.border, width);
    let mut nodes = vec![if element.el.kind == HighLevelKind::Actors {
        actors_figure(actors_at(element.rect), paint)
    } else {
        Node::rect(
            element.rect,
            RADIUS,
            paint.dashed(props.dashed.then_some(DASH)),
        )
    }];
    let text = element_text(element, measure);
    let center = round2(element.rect.center().x);
    let style = name_style(element);
    nodes.extend(text_lines(
        &text.name,
        center,
        text.top,
        style,
        Anchor::Middle,
    ));
    if !text.description.is_empty() {
        nodes.extend(text_lines(
            &text.description,
            center,
            text.top + text.name.len() as f64 * style.line_height + DESCRIPTION_GAP,
            description_style(element),
            Anchor::Middle,
        ));
    }
    nodes
}

/// An arrow as it's drawn: between two points just off the elements it joins.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnArrow {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub from: Point,
    pub to: Point,
    pub two_way: bool,
}

/// Every arrow between its elements' facing sides, straight where they overlap, pulled back
/// from each end by `ARROW_GAP` unless that would leave too little of it.
pub fn drawn_arrows(eng: &Engine<HighLevel>, layout: &[Laid<HighLevel>]) -> Vec<DrawnArrow> {
    let rects: HashMap<ElementId, Rect> =
        layout.iter().map(|e| (e.el.id.clone(), e.rect)).collect();
    let ends: Vec<ConnectionEnds> = eng
        .connections()
        .iter()
        .map(|c| ConnectionEnds {
            id: c.id.clone(),
            source_id: c.source_id.clone(),
            target_id: c.target_id.clone(),
            manual_offset: None,
        })
        .collect();
    let routes = route_connections(&rects, &ends);
    eng.connections()
        .iter()
        .filter_map(|c| {
            let route = routes.get(&c.id)?;
            let (from, to) = (route.from, route.to);
            let (dx, dy) = (to.x - from.x, to.y - from.y);
            let length = dx.hypot(dy);
            let heads = if c.props.two_way { 2.0 } else { 1.0 };
            let (from, to) = if length > ARROW_GAP * 2.0 + HEAD_LENGTH * heads {
                let (ux, uy) = (dx / length, dy / length);
                (
                    Point::new(from.x + ux * ARROW_GAP, from.y + uy * ARROW_GAP),
                    Point::new(to.x - ux * ARROW_GAP, to.y - uy * ARROW_GAP),
                )
            } else {
                (from, to)
            };
            Some(DrawnArrow {
                id: c.id.clone(),
                source_id: c.source_id.clone(),
                target_id: c.target_id.clone(),
                from,
                to,
                two_way: c.props.two_way,
            })
        })
        .collect()
}

/// The outline of a block arrow from `from` to `to`, with a head at `to`, and one at `from`
/// too when `two_way`. A head never takes more than half the arrow.
pub fn arrow_outline(from: Point, to: Point, two_way: bool) -> Vec<Point> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return Vec::new();
    }
    let (ux, uy) = (dx / length, dy / length);
    // To the arrow's left, looking from `from` to `to`.
    let (nx, ny) = (-uy, ux);
    let heads = if two_way { 2.0 } else { 1.0 };
    let head = HEAD_LENGTH.min(length / heads);
    let at = |along: f64, across: f64| {
        Point::new(
            round2(from.x + ux * along + nx * across),
            round2(from.y + uy * along + ny * across),
        )
    };
    let (shaft, wide) = (SHAFT_WIDTH / 2.0, HEAD_WIDTH / 2.0);
    let neck = length - head;
    let mut points = Vec::new();
    if two_way {
        points.extend([at(0.0, 0.0), at(head, wide), at(head, shaft)]);
    } else {
        points.extend([at(0.0, shaft)]);
    }
    points.extend([at(neck, shaft), at(neck, wide), at(length, 0.0)]);
    points.extend([at(neck, -wide), at(neck, -shaft)]);
    if two_way {
        points.extend([at(head, -shaft), at(head, -wide)]);
    } else {
        points.push(at(0.0, -shaft));
    }
    points
}

fn path_of(points: &[Point]) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        d.push(if i == 0 { 'M' } else { 'L' });
        d.push_str(&format!("{} {}", fmt_num(p.x), fmt_num(p.y)));
    }
    d.push('Z');
    d
}

/// The area everything is drawn in.
fn bounds(layout: &[Laid<HighLevel>], arrows: &[DrawnArrow]) -> Rect {
    let points: Vec<Point> = layout
        .iter()
        .flat_map(|e| {
            let r = e.rect;
            [r.position(), Point::new(r.right(), r.bottom())]
        })
        .chain(
            arrows
                .iter()
                .flat_map(|a| arrow_outline(a.from, a.to, a.two_way)),
        )
        .collect();
    if points.is_empty() {
        return Rect::default();
    }
    let left = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let top = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let right = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let bottom = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    Rect::new(left, top, right - left, bottom - top)
}

/// The diagram drawn from `layout`, such as the engine's, or one being dragged.
pub fn scene_of(eng: &Engine<HighLevel>, layout: &[Laid<HighLevel>]) -> Scene {
    let width = eng.line_weight().width();
    let mut nodes: Vec<Node> = layout
        .iter()
        .flat_map(|e| element_nodes(e, eng.measure(), width))
        .collect();
    let arrows = drawn_arrows(eng, layout);
    for a in &arrows {
        let outline = arrow_outline(a.from, a.to, a.two_way);
        if !outline.is_empty() {
            nodes.push(Node::path(
                path_of(&outline),
                Paint::fill_stroke(ARROW_FILL, ARROW_BORDER, width),
            ));
        }
    }
    Scene {
        nodes,
        bounds: bounds(layout, &arrows),
        text_color: TEXT.into(),
        marker_color: ARROW_BORDER.into(),
        background: BACKGROUND.into(),
    }
}

/// The diagram as it's stored, drawn.
pub fn scene(eng: &Engine<HighLevel>) -> Scene {
    scene_of(eng, eng.layout())
}

/// An element's draw.io style: its box, colours, border, and name, at the top of a group that
/// holds something and in the middle otherwise. A group is a draw.io container, so what it
/// holds moves with it. Actors are a container too, with no box, holding their figures, and
/// their text under them.
fn element_style(element: &Laid<HighLevel>, width: f64) -> Style {
    let props = &element.el.props;
    if element.el.kind == HighLevelKind::Actors {
        return Style::new()
            .set("html", 1)
            .set("whiteSpace", "wrap")
            .set("fillColor", "none")
            .set("strokeColor", "none")
            .set("fontColor", TEXT)
            .num("fontSize", name_style(element).size)
            .set("verticalAlign", "top")
            .set("spacing", 0)
            .num("spacingTop", names_top(element.rect) - element.rect.y)
            .num("spacingLeft", PADDING_X)
            .num("spacingRight", PADDING_X)
            .set("container", 1)
            .set("collapsible", 0);
    }
    let colors = props.color_of(element.el.kind).colors();
    let style = name_style(element);
    let mut out = Style::new()
        .set("rounded", 1)
        .set("absoluteArcSize", 1)
        .num("arcSize", RADIUS * 2.0)
        .set("html", 1)
        .set("whiteSpace", "wrap")
        .set("fillColor", colors.fill)
        .set("strokeColor", colors.border)
        .num("strokeWidth", width)
        .set("fontColor", TEXT)
        .num("fontSize", style.size)
        // An application's name is bold in its value, so its description can be regular.
        .set(
            "fontStyle",
            i32::from(element.el.kind == HighLevelKind::Group),
        )
        .num("spacingLeft", PADDING_X)
        .num("spacingRight", PADDING_X);
    if props.dashed {
        out = out.set("dashed", 1).set(
            "dashPattern",
            format!("{} {}", fmt_num(DASH.0), fmt_num(DASH.1)),
        );
    }
    if element.el.kind == HighLevelKind::Group {
        out = out.set("container", 1).set("collapsible", 0);
    }
    match name_place(element) {
        NamePlace::Top => out.set("verticalAlign", "top").set("spacing", 0).num(
            "spacingTop",
            round2((element.header - style.line_height) / 2.0),
        ),
        NamePlace::Middle | NamePlace::Under => out.set("verticalAlign", "middle"),
    }
}

/// Actors' three figures, as draw.io cells inside actors' own, `rect` being their area.
fn actors_cells(element: &Laid<HighLevel>, width: f64) -> Vec<String> {
    let colors = element.el.props.color_of(element.el.kind).colors();
    let at = actors_at(element.rect);
    let style = Style::new()
        .set("shape", person_shape())
        .set("html", 1)
        .set("fillColor", colors.fill)
        .set("strokeColor", colors.border)
        .num("strokeWidth", width)
        .build();
    actors::PLACES
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let rect = Rect::new(
                round2(at.x - element.rect.x + p.x),
                round2(at.y - element.rect.y + p.y),
                actor::WIDTH,
                actor::HEIGHT,
            );
            cell(
                &format!("{}-person-{}", element.id().as_str(), i + 1),
                "",
                &style,
                &escape_xml(element.id().as_str()),
                &rect_attributes(rect),
            )
        })
        .collect()
}

/// An element's draw.io value, as HTML: a group's name, or an application's in bold with any
/// description under it in its smaller regular font, keeping its line breaks.
fn drawio_value(element: &Laid<HighLevel>) -> String {
    let name = escape_xml(&element.el.label);
    if element.el.kind == HighLevelKind::Group {
        return name;
    }
    let name = format!("<b>{name}</b>");
    match description_of(element) {
        Some(text) => {
            let lines: Vec<String> = text.lines().map(escape_xml).collect();
            format!(
                r#"{name}<br><span style="font-size: {}px">{}</span>"#,
                fmt_num(description_style(element).size),
                lines.join("<br>")
            )
        }
        None => name,
    }
}

/// The diagram as a draw.io file, with `heading` above it: groups as containers holding what
/// they hold, applications as boxes, and arrows as wide arrows attached to their elements.
pub fn to_drawio(eng: &Engine<HighLevel>, title: &str, heading: Option<&Heading>) -> String {
    let layout = eng.layout();
    let arrows = drawn_arrows(eng, layout);
    let area = bounds(layout, &arrows);
    let width = eng.line_weight().width();
    let heading = heading.map(|h| heading_cell(h, area.width.max(HEADING_MIN_WIDTH)));
    let shift = Point::new(
        ORIGIN - area.x,
        ORIGIN + heading.as_ref().map_or(0.0, |(_, h)| h + HEADING_GAP) - area.y,
    );
    let by_id: HashMap<&ElementId, &Laid<HighLevel>> = layout.iter().map(|e| (e.id(), e)).collect();
    let mut cells = Vec::new();
    if let Some((xml, _)) = &heading {
        cells.push(xml.clone());
    }
    for e in layout {
        let (rect, parent) = match e.el.parent_id.as_ref().and_then(|p| by_id.get(p)) {
            Some(group) => (
                Rect {
                    x: e.rect.x - group.rect.x,
                    y: e.rect.y - group.rect.y,
                    ..e.rect
                },
                group.id().as_str().to_owned(),
            ),
            None => (
                Rect {
                    x: e.rect.x + shift.x,
                    y: e.rect.y + shift.y,
                    ..e.rect
                },
                "1".to_owned(),
            ),
        };
        cells.push(cell(
            e.id().as_str(),
            &escape_xml(&drawio_value(e)),
            &element_style(e, width).build(),
            &escape_xml(&parent),
            &rect_attributes(rect),
        ));
        if e.el.kind == HighLevelKind::Actors {
            cells.extend(actors_cells(e, width));
        }
    }
    for a in &arrows {
        let (Some(source), Some(target)) = (by_id.get(&a.source_id), by_id.get(&a.target_id))
        else {
            continue;
        };
        let exit = constraint(a.from, source.rect);
        let entry = constraint(a.to, target.rect);
        let style = Style::new()
            .set("shape", "flexArrow")
            .set("html", 1)
            .set("rounded", 0)
            .num("width", SHAFT_WIDTH)
            .num("endWidth", HEAD_WIDTH - SHAFT_WIDTH)
            .num("endSize", HEAD_LENGTH / 3.0)
            .set("endArrow", "block")
            .set("startArrow", if a.two_way { "block" } else { "none" })
            .num("startWidth", HEAD_WIDTH - SHAFT_WIDTH)
            .num("startSize", HEAD_LENGTH / 3.0)
            .set("fillColor", ARROW_FILL)
            .set("strokeColor", ARROW_BORDER)
            .num("strokeWidth", width)
            .num("exitX", exit.0)
            .num("exitY", exit.1)
            .num("exitDx", exit.2)
            .num("exitDy", exit.3)
            .set("exitPerimeter", 0)
            .num("entryX", entry.0)
            .num("entryY", entry.1)
            .num("entryDx", entry.2)
            .num("entryDy", entry.3)
            .set("entryPerimeter", 0);
        cells.push(format!(
            "        <mxCell id=\"{}\" style=\"{}\" edge=\"1\" parent=\"1\" source=\"{}\" target=\"{}\">\n          <mxGeometry relative=\"1\" as=\"geometry\" />\n        </mxCell>",
            escape_xml(a.id.as_str()),
            style.build(),
            escape_xml(a.source_id.as_str()),
            escape_xml(a.target_id.as_str()),
        ));
    }
    let height = area.height + heading.as_ref().map_or(0.0, |(_, h)| h + HEADING_GAP);
    wrap(title, Size::new(area.width, height), &cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    fn engine(elements: serde_json::Value, connections: serde_json::Value) -> Engine<HighLevel> {
        Engine::<HighLevel>::new(
            serde_json::from_value(elements).unwrap(),
            serde_json::from_value(connections).unwrap(),
            NoColors {},
        )
        .with_measure(&ESTIMATE)
    }

    fn landscape() -> Engine<HighLevel> {
        engine(
            serde_json::json!([
                { "id": "g", "kind": "group", "label": "Business", "x": 0, "y": 0,
                  "width": 400, "height": 200, "color": "green", "fontSize": 24 },
                { "id": "a", "kind": "application", "label": "CRM", "x": 20, "y": 60,
                  "parentId": "g" },
                { "id": "e", "kind": "group", "label": "Security", "x": 600, "y": 0,
                  "width": 200, "height": 200, "dashed": true },
                { "id": "b", "kind": "application", "label": "Billing", "x": 0, "y": 400,
                  "color": "yellow", "fontSize": 20 },
            ]),
            serde_json::json!([
                { "id": "r1", "sourceId": "g", "targetId": "e", "twoWay": true },
                { "id": "r2", "sourceId": "b", "targetId": "g" },
            ]),
        )
    }

    #[test]
    fn names_sit_at_the_top_of_a_group_that_holds_something_and_in_the_middle_otherwise() {
        let e = landscape();
        let laid = |id: &str| e.laid(&id.into()).unwrap().clone();
        let (group, empty, app) = (laid("g"), laid("e"), laid("b"));
        assert_eq!(name_place(&group), NamePlace::Top);
        assert_eq!(name_place(&empty), NamePlace::Middle);
        assert_eq!(name_place(&app), NamePlace::Middle);
        // A 24 px name has a 60 px header, its 30 px line in the middle of it.
        assert_eq!(group.header, 60.0);
        let text = element_text(&group, &ESTIMATE);
        assert_eq!((text.name, text.top), (vec!["Business".into()], 15.0));
        // The empty group's name is in its middle: 200 high, one 20 px line.
        assert_eq!(element_text(&empty, &ESTIMATE).top, 90.0);
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(r#"font-size="24" font-weight="700">Business</text>"#));
        // Every name is bold.
        assert!(svg.contains(r#"font-size="20" font-weight="700">Billing</text>"#));
        assert!(svg.contains(r#"font-size="14" font-weight="700">CRM</text>"#));
    }

    #[test]
    fn an_application_has_its_description_under_its_name_in_a_smaller_regular_font() {
        assert_eq!(description_size(14.0), 12.0);
        assert_eq!(description_size(8.0), 8.0);
        let e = engine(
            serde_json::json!([
                { "id": "a", "kind": "application", "label": "CRM", "x": 0, "y": 0,
                  "description": "Leads\nAccounts" },
                { "id": "b", "kind": "application", "label": "Billing", "x": 0, "y": 200,
                  "height": 40, "description": "Doesn't fit" },
            ]),
            serde_json::json!([]),
        );
        let laid = |id: &str| e.laid(&id.into()).unwrap().clone();
        // An 18 px name line, a 2 px gap, and two 15 px lines, in the middle of 80 px.
        let text = element_text(&laid("a"), &ESTIMATE);
        assert_eq!(text.name, ["CRM"]);
        assert_eq!(text.description, ["Leads", "Accounts"]);
        assert_eq!(text.top, 15.0);
        // A box with room for only its name leaves the description out.
        assert!(element_text(&laid("b"), &ESTIMATE).description.is_empty());
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(r#"font-size="14" font-weight="700">CRM</text>"#));
        assert!(svg.contains(r#"font-size="12" font-weight="400">Leads</text>"#));
        assert!(svg.contains(r#"font-size="12" font-weight="400">Accounts</text>"#));
        let xml = to_drawio(&e, "Test", None);
        assert!(xml.contains(
            r#"value="&lt;b&gt;CRM&lt;/b&gt;&lt;br&gt;&lt;span style=&quot;font-size: 12px&quot;&gt;Leads&lt;br&gt;Accounts&lt;/span&gt;""#
        ));
        assert!(xml.contains("fontSize=14;fontStyle=0;"));
    }

    #[test]
    fn actors_are_three_people_with_their_name_and_description_under_them() {
        let e = engine(
            serde_json::json!([
                { "id": "p", "kind": "actors", "label": "Technical Team", "x": 40, "y": 40,
                  "description": "Builds and runs it", "color": "grey" },
            ]),
            serde_json::json!([]),
        );
        let laid = e.laid(&"p".into()).unwrap().clone();
        assert_eq!(laid.rect, Rect::new(40.0, 40.0, 160.0, 120.0));
        assert_eq!(name_place(&laid), NamePlace::Under);
        // Under the 56 px figures, 4 px down, and 6 px below them.
        let text = element_text(&laid, &ESTIMATE);
        assert_eq!(text.top, 40.0 + 4.0 + 56.0 + 6.0);
        assert_eq!(text.name, ["Technical Team"]);
        assert_eq!(text.description, ["Builds and runs it"]);
        assert_eq!(actors_at(laid.rect), Point::new(80.0, 44.0));
        let svg = scene_to_svg("Test", &scene(&e));
        // Three heads, filled and outlined in grey, and no box.
        assert_eq!(svg.matches("<circle").count(), 3);
        assert!(
            svg.contains(r##"<g transform="translate(80 44)" fill="#f5f5f5" stroke="#666666""##)
        );
        assert!(!svg.contains(r#"rx="8""#));
        assert!(svg.contains(r#"font-size="14" font-weight="700">Technical Team</text>"#));
        assert!(svg.contains(r#"font-size="12" font-weight="400">Builds and runs it</text>"#));
        let xml = to_drawio(&e, "Test", None);
        // A borderless container for the text, holding the three figures.
        assert!(xml.contains("fillColor=none;strokeColor=none;"));
        assert!(xml.contains("spacingTop=66;"));
        for i in 1..=3 {
            assert!(xml.contains(&format!(r#"id="p-person-{i}""#)));
        }
        assert_eq!(xml.matches(r#"parent="p""#).count(), 3);
        // Each a person in their own box, undistorted: the back ones 32 px apart, and the front
        // one lower, between them.
        assert!(xml.contains(r#"x="40" y="4" width="48" height="46""#));
        assert!(xml.contains(r#"x="72" y="4" width="48" height="46""#));
        assert!(xml.contains(r#"x="56" y="14" width="48" height="46""#));
        assert!(xml.contains(&format!("shape={}", person_shape())));
        // Actors have no border to dash.
        let mut with = e.element(&"p".into()).unwrap().props.clone();
        with.dashed = true;
        assert!(!crate::rules::kept(HighLevelKind::Actors, with).dashed);
    }

    #[test]
    fn draws_each_element_in_its_colour_and_border() {
        let e = landscape();
        let svg = scene_to_svg("Test", &scene(&e));
        // Green, yellow, the application's usual blue, and the empty group's usual grey, which is
        // dashed.
        for (fill, border) in [
            ("#d5e8d4", "#356b24"),
            ("#fff2cc", "#9a7210"),
            ("#dae8fc", "#2d5f9e"),
            ("#f5f5f5", "#666666"),
        ] {
            assert!(
                svg.contains(&format!(r#"fill="{fill}" stroke="{border}""#)),
                "{fill}"
            );
        }
        assert!(svg.contains(r##"stroke="#666666" stroke-width="1.5" stroke-dasharray="8 4""##));
    }

    #[test]
    fn arrows_are_wide_and_leave_a_gap_at_each_end() {
        let e = landscape();
        let arrows = drawn_arrows(&e, e.layout());
        // Straight across from the group's right to the empty group's left, both 200 high.
        let a = &arrows[0];
        assert_eq!(a.from, Point::new(406.0, 100.0));
        assert_eq!(a.to, Point::new(594.0, 100.0));
        assert!(a.two_way);
        let outline = arrow_outline(a.from, a.to, true);
        assert_eq!(outline.len(), 10);
        assert!(outline.contains(&Point::new(406.0, 100.0)));
        assert!(outline.contains(&Point::new(594.0, 100.0)));
        assert!(outline.contains(&Point::new(576.0, 82.0)));
        // A one-way arrow's tail is as wide as its shaft.
        let one = arrow_outline(Point::new(0.0, 0.0), Point::new(100.0, 0.0), false);
        assert_eq!(
            one,
            [
                Point::new(0.0, 8.0),
                Point::new(82.0, 8.0),
                Point::new(82.0, 18.0),
                Point::new(100.0, 0.0),
                Point::new(82.0, -18.0),
                Point::new(82.0, -8.0),
                Point::new(0.0, -8.0),
            ]
        );
    }

    #[test]
    fn exports_groups_as_drawio_containers_and_arrows_as_attached_wide_arrows() {
        let e = landscape();
        let xml = to_drawio(&e, "Test", None);
        assert!(xml.contains(r#"id="a" value="&lt;b&gt;CRM&lt;/b&gt;""#));
        assert!(xml.contains(r#"parent="g""#));
        assert!(xml.contains("container=1"));
        assert!(xml.contains("fontSize=24;fontStyle=1;"));
        assert!(xml.contains("verticalAlign=top;spacing=0;spacingTop=15"));
        // The empty group's name is in its middle, and its border dashed.
        let empty = xml.find(r#"id="e""#).unwrap();
        let line = &xml[empty..xml[empty..].find('\n').unwrap() + empty];
        assert!(line.contains("verticalAlign=middle"));
        assert!(line.contains("dashed=1;dashPattern=8 4"));
        assert!(xml.contains("shape=flexArrow"));
        assert!(xml.contains(r#"source="g" target="e""#));
        assert!(xml.contains("startArrow=block"));
        assert!(xml.contains("startArrow=none"));
        // The arrow leaves the group 6 px off its right side, half way down.
        assert!(xml.contains("exitX=0;exitY=0;exitDx=406;exitDy=100"));
    }
}
