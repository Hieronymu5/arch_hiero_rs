//! How a conceptual diagram is drawn: each element's look in the diagram's colours, and its
//! arrows with their text, connectors, and transformations.

use std::collections::HashMap;

use hiero_core::color::readable_text;
use hiero_core::engine::{Engine, Laid};
use hiero_core::figures::cloud_edge_point;
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{ArrowLook, Look, LookElement, Shape, component_name_bottom, draw_diagram};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::routing::{
    CONNECTOR_BOX_FONT_SIZE, ConnectionEnds, Route, Side, connector_placement, route_connections,
    straight_range, transformation_placement,
};
use hiero_core::scene::Scene;
use hiero_core::text::{MeasureText, Weight};

use crate::model::{Conceptual, ConceptualKind};
use crate::rules::{Figure, figure_of, is_cloud};

/// An element's fill from the diagram's colours, and text that reads on it.
fn fill_of(
    eng: &Engine<Conceptual>,
    element: &Laid<Conceptual>,
) -> (Option<String>, Option<String>) {
    let colors = eng.colors();
    let figure = figure_of(&element.el);
    if element.el.kind == ConceptualKind::Section {
        return (None, Some(colors.section_names.clone()));
    }
    match figure {
        Some(Figure::Database | Figure::Storage | Figure::File) => {
            return (Some(colors.data.clone()), None);
        }
        _ if element.is_region => {
            return (
                Some(colors.domains.clone()),
                Some(readable_text(&colors.domains).to_owned()),
            );
        }
        Some(Figure::Bot) => return (Some(colors.connectors.clone()), None),
        Some(Figure::Person) => return (Some(colors.applications.clone()), None),
        None => {}
    }
    match element.el.kind.fill() {
        Some(role) => {
            let fill = role.of(colors);
            let text = readable_text(&fill).to_owned();
            (Some(fill), Some(text))
        }
        None => (None, None),
    }
}

/// The shape of an element's figure, such as an actor for a person or a bot, if it's drawn as
/// one. A region or section is drawn as itself instead.
fn figure_shape(element: &Laid<Conceptual>) -> Option<Shape> {
    if element.is_region || element.el.kind == ConceptualKind::Section {
        return None;
    }
    match figure_of(&element.el)? {
        Figure::Person | Figure::Bot => Some(Shape::Actor),
        Figure::Database => Some(Shape::Database),
        Figure::Storage => Some(Shape::Storage),
        Figure::File => Some(Shape::File),
    }
}

/// How an element looks.
pub fn look_of(eng: &Engine<Conceptual>, element: &Laid<Conceptual>) -> LookElement {
    let shape = if element.is_region {
        Shape::Container
    } else if element.el.kind == ConceptualKind::Section {
        Shape::Section
    } else {
        figure_shape(element).unwrap_or(if is_cloud(&element.el) {
            Shape::Cloud
        } else {
            Shape::Box
        })
    };
    let (fill, text_color) = fill_of(eng, element);
    LookElement {
        id: element.el.id.clone(),
        parent_id: element.el.parent_id.clone(),
        label: element.el.label.clone(),
        description: if element.info.has_description {
            element.el.description.clone()
        } else {
            None
        },
        look: Look {
            agent: element.el.props.agent,
            text_at_top: element.text_at_top,
            fill,
            text_color,
            ..Look::new(shape)
        },
        rect: element.rect,
    }
}

/// An arrow as it's drawn.
#[derive(Debug, Clone)]
pub struct DrawnArrow {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub look: ArrowLook,
    pub route: Route,
    /// Whether the arrow joins left and right sides, so it slides up and down.
    pub across_y: bool,
    /// Whether it can run straight between its elements.
    pub straightenable: bool,
}

/// How far under a figure's name an arrow at its bottom ends, so it almost touches it.
const NAME_CLEARANCE: f64 = 2.0;

/// How far a connector box on an arrow into a cloud overlaps the cloud, in pixels.
const CLOUD_CONNECTOR_INSET: f64 = 6.0;

/// `point` moved `distance` pixels into the element whose `side` it's on.
fn inward(point: Point, side: Side, distance: f64) -> Point {
    match side {
        Side::Top => Point::new(point.x, point.y + distance),
        Side::Bottom => Point::new(point.x, point.y - distance),
        Side::Left => Point::new(point.x + distance, point.y),
        Side::Right => Point::new(point.x - distance, point.y),
    }
}

/// Where an arrow end at `point`, on `side` of `element`, is drawn: on the cloud for an
/// element hosted in the cloud, which is drawn inset by half its 2px line, else where it is.
fn on_cloud(element: Option<&Laid<Conceptual>>, side: Side, point: Point) -> Point {
    match element {
        Some(e) if is_cloud(&e.el) => {
            let Rect {
                x,
                y,
                width,
                height,
            } = e.rect;
            cloud_edge_point(
                Rect::new(x + 1.0, y + 1.0, width - 2.0, height - 2.0),
                side,
                point,
            )
        }
        _ => point,
    }
}

/// Where an arrow end at `point`, on `side` of `element`, is drawn: at a figure's bottom, just
/// under its name when the name reaches below the area, such as on two lines, so the line is
/// shortened rather than crossing the name.
fn clear_of_name(
    element: Option<&Laid<Conceptual>>,
    side: Side,
    point: Point,
    measure: &dyn MeasureText,
) -> Point {
    match element.and_then(|e| figure_shape(e).map(|shape| (e, shape))) {
        Some((e, shape)) if side == Side::Bottom => {
            let name_bottom = component_name_bottom(e.rect, &e.el.label, shape, measure);
            Point::new(point.x, point.y.max(name_bottom + NAME_CLEARANCE))
        }
        _ => point,
    }
}

/// Every arrow as a line between its elements' facing sides: straight where they overlap,
/// otherwise between evenly spread points on them. An arrow to or from an application in the
/// cloud ends on the cloud, not its box. `moving` shows an arrow being slid at its in-progress offset.
pub fn drawn_arrows(
    eng: &Engine<Conceptual>,
    layout: &[Laid<Conceptual>],
    moving: Option<(&ConnectionId, f64)>,
) -> Vec<DrawnArrow> {
    let rects: HashMap<ElementId, _> = layout.iter().map(|e| (e.el.id.clone(), e.rect)).collect();
    let ends: Vec<ConnectionEnds> = eng
        .connections()
        .iter()
        .map(|c| ConnectionEnds {
            id: c.id.clone(),
            source_id: c.source_id.clone(),
            target_id: c.target_id.clone(),
            manual_offset: match moving {
                Some((id, offset)) if *id == c.id => Some(offset),
                _ => c.props.manual_offset,
            },
        })
        .collect();
    let routes = route_connections(&rects, &ends);
    let by_id: HashMap<&ElementId, &Laid<Conceptual>> =
        layout.iter().map(|e| (&e.el.id, e)).collect();
    let measure = eng.measure();
    eng.connections()
        .iter()
        .filter_map(|c| {
            let routed = *routes.get(&c.id)?;
            let source = by_id.get(&c.source_id).copied();
            let target = by_id.get(&c.target_id).copied();
            // One at a figure's bottom ends under its name, which may reach below the area.
            let end = |element, side, point| {
                clear_of_name(element, side, on_cloud(element, side, point), measure)
            };
            let route = Route {
                from: end(source, routed.from_side, routed.from),
                to: end(target, routed.to_side, routed.to),
                ..routed
            };
            let props = &c.props;
            // On a cloud the connector box sits a little way in, so its corners don't stand
            // off the cloud's curves.
            let connector_at = if target.is_some_and(|t| is_cloud(&t.el)) {
                inward(route.to, route.to_side, CLOUD_CONNECTOR_INSET)
            } else {
                route.to
            };
            let connector = props.connector.as_ref().map(|text| {
                let width = measure.width(text, CONNECTOR_BOX_FONT_SIZE, Weight::SemiBold);
                (
                    text.clone(),
                    connector_placement(connector_at, route.to_side, width),
                )
            });
            let to = connector.as_ref().map_or(route.to, |(_, p)| p.arrow_end);
            let transformation = props.transformation.as_ref().map(|text| {
                let width = measure.width(text, CONNECTOR_BOX_FONT_SIZE, Weight::SemiBold);
                (
                    text.clone(),
                    transformation_placement(route.from, to, width).0,
                )
            });
            let straightenable = matches!(
                (rects.get(&c.source_id), rects.get(&c.target_id)),
                (Some(s), Some(t)) if straight_range(s, t).is_some()
            );
            Some(DrawnArrow {
                id: c.id.clone(),
                source_id: c.source_id.clone(),
                target_id: c.target_id.clone(),
                look: ArrowLook {
                    from: route.from,
                    to,
                    dashed: props.is_manual(),
                    label: props.label.clone(),
                    bidirectional: props.bidirectional,
                    connector: connector.map(|(text, p)| (text, p.rect)),
                    transformation,
                    tag: None,
                },
                route,
                across_y: route.from_side.is_vertical(),
                straightenable,
            })
        })
        .collect()
}

/// The diagram drawn from `layout`, such as the engine's, or one being dragged.
pub fn scene_of(
    eng: &Engine<Conceptual>,
    layout: &[Laid<Conceptual>],
    moving: Option<(&ConnectionId, f64)>,
) -> Scene {
    let elements: Vec<LookElement> = layout.iter().map(|e| look_of(eng, e)).collect();
    let arrows: Vec<ArrowLook> = drawn_arrows(eng, layout, moving)
        .into_iter()
        .map(|a| a.look)
        .collect();
    draw_diagram(&elements, &arrows, eng.measure(), eng.colors())
}

/// The diagram as it's stored, drawn.
pub fn scene(eng: &Engine<Conceptual>) -> Scene {
    scene_of(eng, eng.layout(), None)
}

/// The diagram as a draw.io file, with `heading` above it.
pub fn to_drawio(
    eng: &Engine<Conceptual>,
    title: &str,
    heading: Option<&hiero_core::export::drawio::Heading>,
) -> String {
    let elements: Vec<LookElement> = eng.layout().iter().map(|e| look_of(eng, e)).collect();
    let arrows: Vec<hiero_core::export::drawio::DrawioArrow> =
        drawn_arrows(eng, eng.layout(), None)
            .into_iter()
            .map(|a| hiero_core::export::drawio::DrawioArrow {
                id: a.id,
                source_id: a.source_id,
                target_id: a.target_id,
                look: a.look,
            })
            .collect();
    hiero_core::export::drawio::diagram_to_drawio(title, &elements, &arrows, heading, eng.colors())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::look::DiagramColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    #[test]
    fn draws_figures_regions_and_connectors() {
        let mut n = 0;
        let mut e = Engine::<Conceptual>::new(Vec::new(), Vec::new(), DiagramColors::default())
            .with_measure(&ESTIMATE)
            .with_ids(move || {
                n += 1;
                format!("id{n}")
            });
        e.add_at_free_position(ConceptualKind::Domain);
        e.add_at_free_position(ConceptualKind::Actor);
        e.add_at_free_position(ConceptualKind::Application);
        let cid = e.add_connection(&"id2".into(), &"id3".into()).unwrap();
        let mut props = e.connection(&cid).unwrap().props.clone();
        props.connector = Some("API".into());
        e.set_connection_props(&cid, props);
        let svg = scene_to_svg("Test", &scene(&e));
        // The domain in the domain colour with its name on top.
        assert!(svg.contains(r##"fill="#f5f5f5" stroke="#1e1e1e" stroke-width="3""##));
        // The actor's figure.
        assert!(svg.contains(r#"<path d="M4 45 A20 20 0 0 1 44 45 Z"/>"#));
        // The connector box in the connectors colour.
        assert!(svg.contains(r##"rx="3" fill="#cfd4da""##));
        assert!(svg.contains(">API</text>"));
    }

    #[test]
    fn ends_arrows_on_the_cloud_with_connector_boxes_a_little_way_in() {
        let elements = serde_json::from_value(serde_json::json!([
            { "id": "u1", "kind": "actor", "label": "Rep", "x": 40, "y": 0 },
            { "id": "a1", "kind": "application", "label": "CRM", "x": 0, "y": 200, "cloudHosted": true },
            { "id": "a2", "kind": "application", "label": "ERP", "x": 0, "y": 400 },
            { "id": "j1", "kind": "job", "label": "Sync", "x": 400, "y": 230 },
        ]))
        .unwrap();
        let connections = serde_json::from_value(serde_json::json!([
            { "id": "c1", "sourceId": "u1", "targetId": "a1" },
            { "id": "c2", "sourceId": "a1", "targetId": "a2", "connector": "API" },
            { "id": "c3", "sourceId": "j1", "targetId": "a1", "connector": "REST" },
        ]))
        .unwrap();
        let e = Engine::<Conceptual>::new(elements, connections, DiagramColors::default())
            .with_measure(&ESTIMATE);
        let arrows = drawn_arrows(&e, e.layout(), None);
        let arrow = |id: &str| arrows.iter().find(|a| a.id == id.into()).unwrap();
        let cloud = e.laid(&"a1".into()).unwrap().rect;
        assert_eq!(cloud, Rect::new(0.0, 200.0, 200.0, 100.0));

        // The cloud's top dips about 16% of its 98px drawn height below the box's top.
        let down = &arrow("c1").look;
        assert_eq!(down.to.x, down.from.x);
        assert!(down.to.y > 214.0 && down.to.y < 218.0, "{down:?}");
        // From the bottom of the cloud, which touches its box in the middle.
        let below = &arrow("c2").look;
        assert_eq!(below.from.x, below.to.x);
        assert!(below.from.y > 298.0 && below.from.y <= 300.0, "{below:?}");
        // A connector box on an arrow into the cloud overlaps it by 6px.
        let edge = cloud_edge_point(
            Rect::new(1.0, 201.0, 198.0, 98.0),
            Side::Right,
            Point::new(200.0, 250.0),
        );
        let into = arrow("c3");
        let (_, rect) = into.look.connector.clone().unwrap();
        assert!((rect.x - (edge.x - 6.0)).abs() < 0.5, "{rect:?} {edge:?}");
        assert_eq!(into.route.to.y, 250.0);
    }

    #[test]
    fn ends_arrows_from_below_a_figure_under_its_name() {
        let elements = serde_json::from_value(serde_json::json!([
            { "id": "u1", "kind": "actor", "label": "Rep", "x": 40, "y": 0 },
            { "id": "u2", "kind": "actor", "label": "Customer Service Representative", "x": 400, "y": 0 },
            { "id": "a1", "kind": "application", "label": "CRM", "x": 0, "y": 200 },
            { "id": "a2", "kind": "application", "label": "ERP", "x": 360, "y": 200 },
        ]))
        .unwrap();
        let connections = serde_json::from_value(serde_json::json!([
            { "id": "c1", "sourceId": "u1", "targetId": "a1" },
            { "id": "c2", "sourceId": "u2", "targetId": "a2" },
        ]))
        .unwrap();
        let e = Engine::<Conceptual>::new(elements, connections, DiagramColors::default())
            .with_measure(&ESTIMATE);
        let arrows = drawn_arrows(&e, e.layout(), None);
        let arrow = |id: &str| &arrows.iter().find(|a| a.id == id.into()).unwrap().look;
        let actor = |id: &str| e.laid(&id.into()).unwrap().rect;

        // A name on one line fits in the area, so the arrow starts at its bottom.
        let short = actor("u1");
        assert_eq!(arrow("c1").from.y, short.y + short.height);
        // One on two lines reaches below it, so the arrow starts just under the name.
        let long = actor("u2");
        let name_bottom = component_name_bottom(
            long,
            "Customer Service Representative",
            Shape::Actor,
            &ESTIMATE,
        );
        assert!(name_bottom > long.y + long.height);
        assert_eq!(arrow("c2").from.y, name_bottom + NAME_CLEARANCE);
    }

    #[test]
    fn fills_a_component_in_its_own_colour_with_text_that_reads_on_it() {
        let component = |colors| {
            let mut e = Engine::<Conceptual>::new(Vec::new(), Vec::new(), colors)
                .with_measure(&ESTIMATE)
                .with_ids(|| "c1".into());
            e.add_at_free_position(ConceptualKind::Component);
            let laid = e.laid(&"c1".into()).unwrap().clone();
            fill_of(&e, &laid)
        };
        // The default is light enough for dark text.
        let (fill, text) = component(DiagramColors::default());
        assert_eq!(fill.as_deref(), Some("#a1c6e9"));
        assert_eq!(text.as_deref(), Some(hiero_core::color::DARK_TEXT));
        // A dark one flips it to white.
        let dark = DiagramColors {
            components: "#0b3d91".into(),
            ..DiagramColors::default()
        };
        let (fill, text) = component(dark);
        assert_eq!(fill.as_deref(), Some("#0b3d91"));
        assert_eq!(text.as_deref(), Some(hiero_core::color::LIGHT_TEXT));
    }

    #[test]
    fn draws_a_file_as_a_page_with_a_folded_corner_in_the_data_colour() {
        let mut e = Engine::<Conceptual>::new(Vec::new(), Vec::new(), DiagramColors::default())
            .with_measure(&ESTIMATE)
            .with_ids(|| "f1".into());
        e.add_at_free_position(ConceptualKind::File);
        let svg = scene_to_svg("Test", &scene(&e));
        assert!(svg.contains(r##"fill="#ffe599""##));
        assert!(svg.contains(r#"<path d="M17 1 H41 V45 H7 V11 Z"/>"#));
        assert!(svg.contains(r#"<path d="M17 1 V11 H7 Z"/>"#));
        assert!(svg.contains(r#"<line x1="20" y1="9" x2="34" y2="9"/>"#));
        assert!(svg.contains(r#"<line x1="14" y1="37" x2="34" y2="37"/>"#));
        assert!(svg.contains(">File</text>"));
        let drawio = to_drawio(&e, "Test", None);
        assert!(drawio.contains("stencil(pZTdbsMgDIWfhvsEt812"));
    }
}
