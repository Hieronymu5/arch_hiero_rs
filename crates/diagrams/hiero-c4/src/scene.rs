//! How a C4 diagram is drawn: people as the actor figure with their text underneath, everything
//! else in the C4 shapes, each in its colour, and relationships as dashed black arrows with their description and
//! technology on the middle of the line.

use std::collections::HashMap;

use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{DrawioArrow, Heading, diagram_to_drawio};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{ArrowLook, Caption, DiagramColors, Look, LookElement, Shape, draw_diagram};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::palette::BLUE;
use hiero_core::routing::{ConnectionEnds, Side, route_connections};
use hiero_core::scene::Scene;
use hiero_core::text::MeasureText;

use hiero_core::c4::{C4Shape, hexagon_edge_point, person_text_bottom, pipe_edge_point};

use crate::model::{C4Type, Color, Role};
use crate::rules::type_line;

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
pub fn look_of<T: C4Type>(element: &Laid<T>) -> LookElement {
    let role = T::role(element.el.kind);
    let colors = Color::colors(element.el.props.color);
    let look = Look {
        // A boundary is only a line, in its colour.
        fill: (!role.is_boundary()).then(|| colors.fill.into()),
        // A person's figure has the usual dark outline, as other figures do.
        border_color: (role != Role::Person).then(|| colors.border.into()),
        type_line: type_line::<T>(&element.el),
        ..Look::new(Shape::C4(role.shape()))
    };
    LookElement {
        id: element.el.id.clone(),
        parent_id: element.el.parent_id.clone(),
        label: element.el.label.clone(),
        description: if element.info.has_description {
            element.el.description.clone()
        } else {
            None
        },
        look,
        rect: element.rect,
    }
}

/// How far under a person's text an arrow at their bottom starts or ends.
const TEXT_CLEARANCE: f64 = 2.0;

/// Where an arrow end at `point` on `side` of `element` is drawn: at a person's bottom, just
/// under their text, as nothing is drawn below it.
fn under_text<T: C4Type>(
    element: Option<&Laid<T>>,
    side: Side,
    point: Point,
    measure: &dyn MeasureText,
) -> Point {
    match element {
        Some(e) if side == Side::Bottom && T::role(e.el.kind) == Role::Person => {
            let bottom = person_text_bottom(&look_of(e), measure) + TEXT_CLEARANCE;
            Point::new(point.x, point.y.min(bottom))
        }
        _ => point,
    }
}

/// Where an arrow end at `point` on `side` of `element` is drawn: on a service's hexagon, or
/// a queue's or topic's cylinder, rather than the box around it.
fn on_outline<T: C4Type>(element: Option<&Laid<T>>, side: Side, point: Point) -> Point {
    let Some(e) = element else {
        return point;
    };
    match T::role(e.el.kind).shape() {
        C4Shape::Hexagon => hexagon_edge_point(e.rect, side, point),
        C4Shape::Pipe => pipe_edge_point(e.rect, side, point),
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
pub fn drawn_arrows<T: C4Type>(eng: &Engine<T>, layout: &[Laid<T>]) -> Vec<DrawnArrow> {
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
    let by_id: HashMap<&ElementId, &Laid<T>> = layout.iter().map(|e| (&e.el.id, e)).collect();
    let measure = eng.measure();
    eng.connections()
        .iter()
        .filter_map(|c| {
            let route = routes.get(&c.id)?;
            let source = by_id.get(&c.source_id).copied();
            let target = by_id.get(&c.target_id).copied();
            let caption = Caption::new(
                c.props.description.as_deref(),
                c.props
                    .technology
                    .as_deref()
                    .filter(|_| T::ARROW_TECHNOLOGY),
                measure,
            );
            Some(DrawnArrow {
                id: c.id.clone(),
                source_id: c.source_id.clone(),
                target_id: c.target_id.clone(),
                look: ArrowLook {
                    from: under_text(
                        source,
                        route.from_side,
                        on_outline(source, route.from_side, route.from),
                        measure,
                    ),
                    to: under_text(
                        target,
                        route.to_side,
                        on_outline(target, route.to_side, route.to),
                        measure,
                    ),
                    dashed: true,
                    caption: (!caption.is_empty()).then_some(caption),
                    ..ArrowLook::default()
                },
            })
        })
        .collect()
}

/// The diagram drawn from `layout`, such as the engine's, or one being dragged.
pub fn scene_of<T: C4Type>(eng: &Engine<T>, layout: &[Laid<T>]) -> Scene {
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
pub fn scene<T: C4Type>(eng: &Engine<T>) -> Scene {
    scene_of(eng, eng.layout())
}

/// The diagram as a draw.io file, with `heading` above it.
pub fn to_drawio<T: C4Type>(eng: &Engine<T>, title: &str, heading: Option<&Heading>) -> String {
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
    use crate::model::{C4Component, C4Container, C4Context};
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    fn engine<T: C4Type>(elements: serde_json::Value, connections: serde_json::Value) -> Engine<T> {
        Engine::<T>::new(
            serde_json::from_value(elements).unwrap(),
            serde_json::from_value(connections).unwrap(),
            NoColors {},
        )
        .with_measure(&ESTIMATE)
    }

    #[test]
    fn colours_elements_blue_green_orange_or_red() {
        let e = engine::<C4Context>(
            serde_json::json!([
                { "id": "u", "kind": "person", "label": "Customer", "x": 40, "y": 0 },
                { "id": "v", "kind": "person", "label": "Clerk", "x": 400, "y": 0,
                  "color": "green" },
                { "id": "a", "kind": "software-system", "label": "Banking", "x": 0, "y": 200,
                  "description": "Lets customers bank online." },
                { "id": "b", "kind": "software-system", "label": "Mainframe", "x": 400, "y": 200,
                  "color": "orange" },
                { "id": "c", "kind": "software-system", "label": "E-mail", "x": 800, "y": 200,
                  "color": "red" },
                { "id": "d", "kind": "software-system", "label": "Ledger", "x": 1200, "y": 200,
                  "color": "green" },
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
        // People are the actor figure, in their colour.
        assert!(svg.contains(r#"<path d="M4 45 A20 20 0 0 1 44 45 Z"/>"#));
        assert!(
            svg.contains(r##"<g transform="translate(116 5)" fill="#dae8fc""##),
            "{svg}"
        );
        assert!(svg.contains(r##"<g transform="translate(476 5)" fill="#d5e8d4""##));
        // The relationship is dashed and black, with its description on its middle.
        assert!(svg.contains(r##"stroke="#1e1e1e" stroke-width="2" stroke-dasharray="8 4""##));
        assert!(svg.contains(">Views balances using</text>"));
    }

    #[test]
    fn boundaries_take_a_colour_and_older_scopes_read_as_colours() {
        let e = engine::<C4Container>(
            serde_json::json!([
                { "id": "b", "kind": "system-boundary", "label": "Shop", "x": 0, "y": 0,
                  "color": "red" },
                { "id": "i", "kind": "software-system", "label": "Old internal", "x": 800,
                  "y": 0, "scope": "internal" },
                { "id": "x", "kind": "software-system", "label": "Old external", "x": 800,
                  "y": 400, "scope": "external" },
            ]),
            serde_json::json!([]),
        );
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(r##"fill="none" stroke="#9c3530""##));
        let color = |id: &str| e.element(&id.into()).unwrap().props.color;
        assert_eq!(color("i"), Some(Color::Orange));
        assert_eq!(color("x"), Some(Color::Red));
        // Written back as a colour.
        let json = serde_json::to_value(e.element(&"i".into()).unwrap()).unwrap();
        assert_eq!(json["color"], "orange");
        assert!(json.get("scope").is_none());
    }

    #[test]
    fn relationships_show_technology_in_square_brackets_but_not_on_context_diagrams() {
        let elements = serde_json::json!([
            { "id": "a", "kind": "web-application", "label": "UI", "x": 0, "y": 0,
              "technology": "Angular" },
            { "id": "b", "kind": "backend", "label": "API", "x": 0, "y": 400 },
        ]);
        let connections = serde_json::json!([
            { "id": "r", "sourceId": "a", "targetId": "b", "description": "Calls",
              "technology": "JSON/HTTPS" },
        ]);
        let e = engine::<C4Container>(elements, connections.clone());
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(">[JSON/HTTPS]</text>"));
        assert!(svg.contains(">[Container: Angular]</text>"));
        let arrow = &drawn_arrows(&e, e.layout())[0].look;
        // Straight down the middle, from the website's bottom to the backend's top.
        assert_eq!((arrow.from.y, arrow.to.y), (160.0, 400.0));

        let context = engine::<C4Context>(
            serde_json::json!([
                { "id": "a", "kind": "software-system", "label": "A", "x": 0, "y": 0 },
                { "id": "b", "kind": "software-system", "label": "B", "x": 0, "y": 400 },
            ]),
            connections,
        );
        let svg = scene_to_svg("Test", &scene(&context));
        assert!(svg.contains(">Calls</text>"));
        assert!(!svg.contains("JSON/HTTPS"));
    }

    #[test]
    fn exports_boundaries_as_drawio_containers_with_their_elements_inside() {
        let e = engine::<C4Component>(
            serde_json::json!([
                { "id": "s", "kind": "system-boundary", "label": "Banking", "x": 0, "y": 0 },
                { "id": "k", "kind": "container-boundary", "label": "API", "x": 20, "y": 40,
                  "parentId": "s" },
                { "id": "c", "kind": "component", "label": "Sign In", "x": 40, "y": 80,
                  "parentId": "k", "technology": "Spring MVC" },
                { "id": "d", "kind": "database", "label": "Accounts", "x": 1000, "y": 80 },
            ]),
            serde_json::json!([
                { "id": "r", "sourceId": "c", "targetId": "d", "description": "Reads from",
                  "technology": "SQL" },
            ]),
        );
        let xml = to_drawio(&e, "Test", None);
        assert!(xml.contains(r#"id="s" value="&lt;b&gt;Banking&lt;/b&gt;"#));
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
    fn services_are_hexagons_that_arrows_meet_on_their_outline() {
        let e = engine::<C4Container>(
            serde_json::json!([
                { "id": "b", "kind": "system-boundary", "label": "Shop", "x": 0, "y": 0 },
                { "id": "s", "kind": "service", "label": "Orders API", "x": 20, "y": 40,
                  "parentId": "b", "technology": "Spring",
                  "description": "Implements ordering." },
                { "id": "d", "kind": "database", "label": "Orders", "x": 400, "y": 40,
                  "parentId": "b" },
                { "id": "w", "kind": "web-application", "label": "Shop UI", "x": 800, "y": 400 },
            ]),
            serde_json::json!([
                { "id": "r1", "sourceId": "s", "targetId": "d", "description": "Reads from" },
                { "id": "r2", "sourceId": "w", "targetId": "s", "description": "Calls" },
            ]),
        );
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(">[Container: Spring]</text>"));
        assert!(
            svg.contains(r#"<path d="M20 120L70 40H170L220 120L170 200H70Z""#),
            "{svg}"
        );
        let arrows = drawn_arrows(&e, e.layout());
        // Level with the hexagon's right point, the arrow starts at the point.
        assert_eq!(arrows[0].look.from, Point::new(220.0, 120.0));
        // One in from below and to the right meets the hexagon's edge, not its box.
        let to = arrows[1].look.to;
        let rect = e.laid(&"s".into()).unwrap().rect;
        assert!(rect.contains_point(to));
        assert!(to.x < rect.right() || to.y < rect.bottom(), "{to:?}");
        assert!(to_drawio(&e, "Test", None).contains("shape=hexagon"));
    }

    #[test]
    fn queues_and_topics_are_containers_on_their_side() {
        let e = engine::<C4Container>(
            serde_json::json!([
                { "id": "a", "kind": "service", "label": "Service A", "x": 0, "y": 0 },
                { "id": "q", "kind": "queue", "label": "Queue X", "x": 400, "y": 20,
                  "technology": "RabbitMQ" },
                { "id": "t", "kind": "topic", "label": "Topic Y", "x": 400, "y": 400 },
                { "id": "c", "kind": "service", "label": "Service C", "x": 800, "y": 0 },
            ]),
            serde_json::json!([
                { "id": "r1", "sourceId": "a", "targetId": "q", "description": "Publishes messages to" },
                { "id": "r2", "sourceId": "c", "targetId": "q", "description": "Subscribes to messages from" },
                { "id": "r3", "sourceId": "a", "targetId": "t" },
            ]),
        );
        let queue = e.laid(&"q".into()).unwrap();
        assert_eq!(queue.rect.size(), crate::model::QUEUE_SIZE);
        assert_eq!(
            <C4Container as hiero_core::engine::DiagramType>::kind_description(&e, queue),
            "Queue (RabbitMQ)"
        );
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(">[Container: RabbitMQ]</text>"));
        assert!(
            svg.contains(r#"<ellipse cx="414" cy="80" rx="14" ry="60""#),
            "{svg}"
        );
        let arrows = drawn_arrows(&e, e.layout());
        // Level with the middle, in at the left end's outer point and out of the right end's.
        assert_eq!(arrows[0].look.to, Point::new(400.0, 80.0));
        assert_eq!(arrows[1].look.to, Point::new(640.0, 80.0));
        // From above, between the ends, at the top.
        assert_eq!(arrows[2].look.to.y, 400.0);
    }

    #[test]
    fn people_show_their_type_and_description_and_arrows_leave_under_their_text() {
        let e = engine::<C4Context>(
            serde_json::json!([
                { "id": "u", "kind": "person", "label": "Customer", "x": 0, "y": 0,
                  "description": "Banks online." },
                { "id": "v", "kind": "person", "label": "Clerk", "x": 400, "y": 0,
                  "typeName": "Bank Staff" },
                { "id": "a", "kind": "software-system", "label": "Banking", "x": 0, "y": 400,
                  "typeName": "Core System" },
            ]),
            serde_json::json!([
                { "id": "r", "sourceId": "u", "targetId": "a" },
            ]),
        );
        let person = e.laid(&"u".into()).unwrap();
        assert_eq!(person.rect.size(), crate::model::PERSON_SIZE);
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(r#"font-size="11" font-weight="400">[Person]</text>"#));
        assert!(svg.contains(">[Bank Staff]</text>"));
        assert!(svg.contains(">[Core System]</text>"));
        assert!(svg.contains(r#"font-size="13" font-weight="400">Banks online.</text>"#));
        // The arrow down from the customer starts just under their text, not at the bottom
        // of their area, where nothing is drawn.
        let from = drawn_arrows(&e, e.layout())[0].look.from;
        let bottom = person_text_bottom(&look_of(person), &ESTIMATE);
        assert_eq!(from.y, bottom + 2.0);
        assert!(from.y < person.rect.bottom());
    }
}
