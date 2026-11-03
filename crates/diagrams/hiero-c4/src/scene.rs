//! How a C4 diagram is drawn: people as the actor figure, everything else in the C4 shapes in
//! blue, orange, or red, and relationships as dashed black arrows with their description and
//! technology on the middle of the line.

use std::collections::HashMap;

use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{DrawioArrow, Heading, diagram_to_drawio};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{
    ArrowLook, Caption, DiagramColors, Look, LookElement, Shape, component_name_bottom,
    draw_diagram,
};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::palette::BLUE;
use hiero_core::routing::{ConnectionEnds, Side, route_connections};
use hiero_core::scene::Scene;
use hiero_core::text::MeasureText;

use crate::model::{C4Type, Role, Scope};
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
    let look = match role.shape() {
        None => Look {
            fill: Some(BLUE.fill.into()),
            ..Look::new(Shape::Actor)
        },
        Some(shape) => {
            let colors = Scope::colors(element.el.props.scope.filter(|_| role.has_scope()));
            Look {
                fill: (!role.is_boundary()).then(|| colors.fill.into()),
                border_color: Some(colors.border.into()),
                type_line: type_line::<T>(&element.el),
                ..Look::new(Shape::C4(shape))
            }
        }
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

/// How far under a person's name an arrow at their bottom ends.
const NAME_CLEARANCE: f64 = 2.0;

fn is_person<T: C4Type>(element: &Laid<T>) -> bool {
    T::role(element.el.kind) == Role::Person
}

/// The area an element is drawn in: a person's reaches down to the end of their name, which
/// goes below the area when it takes two lines.
pub fn drawn_rect<T: C4Type>(eng: &Engine<T>, element: &Laid<T>) -> Rect {
    let rect = element.rect;
    if !is_person(element) {
        return rect;
    }
    let bottom = component_name_bottom(rect, &element.el.label, Shape::Actor, eng.measure());
    Rect {
        height: rect.height.max(bottom - rect.y),
        ..rect
    }
}

/// Where an arrow end at `point` on `side` of `element` is drawn: at a person's bottom, under
/// their name.
fn clear_of_name<T: C4Type>(
    element: Option<&Laid<T>>,
    side: Side,
    point: Point,
    measure: &dyn MeasureText,
) -> Point {
    match element {
        Some(e) if side == Side::Bottom && is_person(e) => {
            let bottom = component_name_bottom(e.rect, &e.el.label, Shape::Actor, measure);
            Point::new(point.x, point.y.max(bottom + NAME_CLEARANCE))
        }
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
                    from: clear_of_name(source, route.from_side, route.from, measure),
                    to: clear_of_name(target, route.to_side, route.to, measure),
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
    fn colours_software_systems_by_scope() {
        let e = engine::<C4Context>(
            serde_json::json!([
                { "id": "u", "kind": "person", "label": "Customer", "x": 40, "y": 0 },
                { "id": "a", "kind": "software-system", "label": "Banking", "x": 0, "y": 200,
                  "description": "Lets customers bank online." },
                { "id": "b", "kind": "software-system", "label": "Mainframe", "x": 400, "y": 200,
                  "scope": "internal" },
                { "id": "c", "kind": "software-system", "label": "E-mail", "x": 800, "y": 200,
                  "scope": "external" },
            ]),
            serde_json::json!([
                { "id": "r1", "sourceId": "u", "targetId": "a", "description": "Views balances using" },
            ]),
        );
        let svg = scene_to_svg("Test", &scene(&e));
        for (fill, border) in [
            ("#dae8fc", "#2d5f9e"),
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
        // The person is the actor figure, in blue.
        assert!(svg.contains(r#"<path d="M4 45 A20 20 0 0 1 44 45 Z"/>"#));
        // The relationship is dashed and black, with its description on its middle.
        assert!(svg.contains(r##"stroke="#1e1e1e" stroke-width="2" stroke-dasharray="8 4""##));
        assert!(svg.contains(">Views balances using</text>"));
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
    fn a_persons_drawn_area_reaches_the_end_of_a_long_name() {
        let e = engine::<C4Context>(
            serde_json::json!([
                { "id": "u", "kind": "person", "label": "Personal Banking Customer", "x": 0, "y": 0 },
                { "id": "a", "kind": "software-system", "label": "A", "x": 0, "y": 300 },
            ]),
            serde_json::json!([]),
        );
        let person = e.laid(&"u".into()).unwrap();
        assert!(drawn_rect(&e, person).height > person.rect.height);
        let system = e.laid(&"a".into()).unwrap();
        assert_eq!(drawn_rect(&e, system), system.rect);
    }
}
