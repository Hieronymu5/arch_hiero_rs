//! How a C4 diagram is drawn: people as the actor figure with their text underneath, everything
//! else in the C4 shapes, each in its colour, and relationships as dashed black arrows with
//! their description and technology on the middle of the line.

use std::collections::HashMap;

use hiero_core::c4::{C4Shape, hexagon_edge_point, person_top_point, pipe_edge_point};
use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{DrawioArrow, Heading, diagram_to_drawio};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{ArrowLook, Caption, DiagramColors, Look, LookElement, Shape, draw_diagram};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::palette::BLUE;
use hiero_core::routing::{ConnectionEnds, Side, route_connections};
use hiero_core::scene::Scene;

use crate::model::{C4, C4Kind, Color};
use crate::rules::{shape_of, type_line};

/// The colour of relationships, and of people's outlines.
pub const LINES: &str = "#1e1e1e";

/// The colours the shared drawing code takes. C4 elements set their own; these are for the
/// people and the relationships.
pub fn colors() -> DiagramColors {
    DiagramColors {
        borders: LINES.into(),
        lines: LINES.into(),
        applications: BLUE.fill.into(),
        ..DiagramColors::default()
    }
}

/// How an element looks.
pub fn look_of(element: &Laid<C4>) -> LookElement {
    let kind = element.el.kind;
    let colors = Color::colors(element.el.props.color);
    LookElement {
        id: element.el.id.clone(),
        parent_id: element.el.parent_id.clone(),
        label: element.el.label.clone(),
        description: element.el.description.clone(),
        look: Look {
            // A boundary is only a line, in its colour.
            fill: (kind != C4Kind::Boundary).then(|| colors.fill.into()),
            border_color: Some(colors.border.into()),
            type_line: Some(type_line(&element.el)),
            ..Look::new(Shape::C4(shape_of(&element.el)))
        },
        rect: element.rect,
    }
}

/// Where an arrow end at `point` on `side` of `element` is drawn: on a service's hexagon, a
/// queue's or topic's cylinder, or the circle on a person's top, rather than the box around it.
fn on_outline(element: Option<&Laid<C4>>, side: Side, point: Point) -> Point {
    let Some(e) = element else {
        return point;
    };
    match shape_of(&e.el) {
        C4Shape::Hexagon => hexagon_edge_point(e.rect, side, point),
        C4Shape::Pipe => pipe_edge_point(e.rect, side, point),
        C4Shape::Person if side == Side::Top => person_top_point(e.rect, point),
        _ => point,
    }
}

/// A relationship as it's drawn.
#[derive(Debug, Clone)]
pub struct DrawnArrow {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub look: ArrowLook,
}

/// Every relationship as a dashed line between its elements' facing sides, with its
/// description and technology on its middle.
pub fn drawn_arrows(eng: &Engine<C4>, layout: &[Laid<C4>]) -> Vec<DrawnArrow> {
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
    let by_id: HashMap<&ElementId, &Laid<C4>> = layout.iter().map(|e| (&e.el.id, e)).collect();
    let measure = eng.measure();
    eng.connections()
        .iter()
        .filter_map(|c| {
            let route = routes.get(&c.id)?;
            let source = by_id.get(&c.source_id).copied();
            let target = by_id.get(&c.target_id).copied();
            let caption = Caption::new(
                c.props.description.as_deref(),
                c.props.technology.as_deref(),
                measure,
            );
            let end = on_outline;
            Some(DrawnArrow {
                id: c.id.clone(),
                source_id: c.source_id.clone(),
                target_id: c.target_id.clone(),
                look: ArrowLook {
                    from: end(source, route.from_side, route.from),
                    to: end(target, route.to_side, route.to),
                    dashed: true,
                    caption: (!caption.is_empty()).then_some(caption),
                    ..ArrowLook::default()
                },
            })
        })
        .collect()
}

/// The diagram drawn from `layout`, such as the engine's, or one being dragged.
pub fn scene_of(eng: &Engine<C4>, layout: &[Laid<C4>]) -> Scene {
    let elements: Vec<LookElement> = layout.iter().map(look_of).collect();
    let arrows: Vec<ArrowLook> = drawn_arrows(eng, layout)
        .into_iter()
        .map(|a| a.look)
        .collect();
    draw_diagram(
        &elements,
        &arrows,
        eng.measure(),
        &colors(),
        eng.line_weight(),
    )
}

/// The diagram as it's stored, drawn.
pub fn scene(eng: &Engine<C4>) -> Scene {
    scene_of(eng, eng.layout())
}

/// The diagram as a draw.io file, with `heading` above it.
pub fn to_drawio(eng: &Engine<C4>, title: &str, heading: Option<&Heading>) -> String {
    let elements: Vec<LookElement> = eng.layout().iter().map(look_of).collect();
    let arrows: Vec<DrawioArrow> = drawn_arrows(eng, eng.layout())
        .into_iter()
        .map(|a| DrawioArrow {
            id: a.id,
            source_id: a.source_id,
            target_id: a.target_id,
            look: a.look,
        })
        .collect();
    diagram_to_drawio(
        title,
        &elements,
        &arrows,
        heading,
        &colors(),
        eng.line_weight(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PERSON_SIZE;
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    fn engine(elements: serde_json::Value, connections: serde_json::Value) -> Engine<C4> {
        Engine::<C4>::new(
            serde_json::from_value(elements).unwrap(),
            serde_json::from_value(connections).unwrap(),
            NoColors {},
        )
        .with_measure(&ESTIMATE)
    }

    #[test]
    fn colours_elements_blue_green_orange_or_red() {
        let e = engine(
            serde_json::json!([
                { "id": "u", "kind": "person", "label": "Customer", "x": 40, "y": 0 },
                { "id": "v", "kind": "person", "label": "Clerk", "x": 400, "y": 0,
                  "color": "green" },
                { "id": "a", "kind": "system", "label": "Banking", "x": 0, "y": 200,
                  "description": "Lets customers bank online." },
                { "id": "b", "kind": "system", "label": "Mainframe", "x": 400, "y": 200,
                  "color": "orange" },
                { "id": "c", "kind": "system", "label": "E-mail", "x": 800, "y": 200,
                  "color": "red" },
                { "id": "d", "kind": "system", "label": "Ledger", "x": 1200, "y": 200,
                  "color": "green" },
                { "id": "x", "kind": "boundary", "label": "Shop", "x": 0, "y": 600,
                  "color": "red" },
            ]),
            serde_json::json!([
                { "id": "r1", "sourceId": "u", "targetId": "a", "description": "Views balances using" },
            ]),
        );
        let svg = scene_to_svg("Test", &scene(&e));
        for (fill, border) in [
            ("#dae8fc", "#2d5f9e"),
            ("#d5e8d4", "#356b24"),
            ("#ffe6cc", "#b46504"),
            ("#f8cecc", "#9c3530"),
        ] {
            assert!(
                svg.contains(&format!(r#"fill="{fill}" stroke="{border}""#)),
                "{fill}"
            );
        }
        assert!(svg.contains(">[Software System]</text>"));
        assert!(svg.contains(">Lets customers bank online.</text>"));
        // People's circles, in their colour.
        assert!(
            svg.contains(r##"<circle cx="140" cy="30" r="30" fill="#dae8fc""##),
            "{svg}"
        );
        assert!(svg.contains(r##"<circle cx="500" cy="30" r="30" fill="#d5e8d4""##));
        // A boundary is only a line, in its colour.
        assert!(svg.contains(r##"fill="none" stroke="#9c3530""##));
        // The relationship is dashed and black, with its description on its middle.
        assert!(svg.contains(r##"stroke="#1e1e1e" stroke-width="2" stroke-dasharray="8 4""##));
        assert!(svg.contains(">Views balances using</text>"));
    }

    #[test]
    fn containers_are_drawn_in_their_types_shape() {
        let e = engine(
            serde_json::json!([
                { "id": "w", "kind": "container", "label": "UI", "x": 0, "y": 0,
                  "containerType": "web-application", "technology": "Angular" },
                { "id": "s", "kind": "container", "label": "Orders API", "x": 0, "y": 400,
                  "containerType": "service" },
                { "id": "q", "kind": "container", "label": "Queue X", "x": 400, "y": 420,
                  "containerType": "queue" },
                { "id": "p", "kind": "container", "label": "Plain", "x": 800, "y": 0 },
            ]),
            serde_json::json!([
                { "id": "r", "sourceId": "w", "targetId": "s", "description": "Calls",
                  "technology": "JSON/HTTPS" },
                { "id": "r2", "sourceId": "s", "targetId": "q" },
            ]),
        );
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(">[Container: Angular]</text>"));
        assert!(svg.contains(">[JSON/HTTPS]</text>"));
        // The service's hexagon, 200 wide.
        assert!(
            svg.contains(r#"<path d="M0 480L50 400H150L200 480L150 560H50Z""#),
            "{svg}"
        );
        // The plain container is a rounded box.
        assert!(svg.contains(r#"<rect x="800" y="0" width="240" height="160" rx="8""#));
        let arrows = drawn_arrows(&e, e.layout());
        // Down the middle from the website to the service's top.
        assert_eq!(arrows[0].look.to.y, 400.0);
        // Out of the service's right point, level with its middle, into the queue's open end.
        assert_eq!(arrows[1].look.from, Point::new(200.0, 480.0));
        assert_eq!(arrows[1].look.to, Point::new(400.0, 480.0));
    }

    #[test]
    fn exports_boundaries_as_drawio_containers_with_their_elements_inside() {
        let e = engine(
            serde_json::json!([
                { "id": "s", "kind": "boundary", "label": "Banking", "x": 0, "y": 0 },
                { "id": "k", "kind": "boundary", "label": "API", "x": 20, "y": 40,
                  "parentId": "s", "boundaryType": "container" },
                { "id": "c", "kind": "component", "label": "Sign In", "x": 40, "y": 80,
                  "parentId": "k", "technology": "Spring MVC" },
                { "id": "d", "kind": "container", "label": "Accounts", "x": 1000, "y": 80,
                  "containerType": "database" },
            ]),
            serde_json::json!([
                { "id": "r", "sourceId": "c", "targetId": "d", "description": "Reads from",
                  "technology": "SQL" },
            ]),
        );
        let xml = to_drawio(&e, "Test", None);
        assert!(xml.contains(r#"id="s" value="&lt;b&gt;Banking&lt;/b&gt;"#));
        assert!(xml.contains("[Software System]"));
        assert!(xml.contains(r#"parent="s""#));
        assert!(xml.contains(r#"parent="k""#));
        assert!(xml.contains("id=\"c-tab-0\""));
        assert!(xml.contains("shape=cylinder3"));
        // The relationship's text on its middle, the technology smaller under it.
        assert!(xml.contains(
            r#"value="Reads from&lt;br&gt;&lt;span style=&quot;font-size: 11px&quot;&gt;[SQL]&lt;/span&gt;""#
        ));
        assert!(xml.contains("dashed=1"));
    }

    #[test]
    fn people_show_their_type_and_description_and_arrows_leave_their_box() {
        let e = engine(
            serde_json::json!([
                { "id": "u", "kind": "person", "label": "Customer", "x": 0, "y": 0,
                  "description": "Banks online." },
                { "id": "v", "kind": "person", "label": "Clerk", "x": 400, "y": 0,
                  "typeName": "Bank Staff" },
                { "id": "a", "kind": "system", "label": "Banking", "x": 0, "y": 400,
                  "typeName": "Core System" },
            ]),
            serde_json::json!([
                { "id": "r", "sourceId": "u", "targetId": "a" },
            ]),
        );
        let person = e.laid(&"u".into()).unwrap();
        assert_eq!(person.rect.size(), PERSON_SIZE);
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(r#"font-size="11" font-weight="400">[Person]</text>"#));
        assert!(svg.contains(">[Bank Staff]</text>"));
        assert!(svg.contains(">[Core System]</text>"));
        assert!(svg.contains(r#"font-size="13" font-weight="400">Banks online.</text>"#));
        // The arrow down from the customer starts on the bottom of their box.
        let from = drawn_arrows(&e, e.layout())[0].look.from;
        assert_eq!(from.y, person.rect.bottom());
    }
}
