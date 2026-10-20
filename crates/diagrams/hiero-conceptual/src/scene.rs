//! How a conceptual diagram is drawn: each element's look in the diagram's colours, and its
//! arrows with their text, connectors, and transformations.

use std::collections::HashMap;

use hiero_core::color::readable_text;
use hiero_core::engine::{Engine, Laid};
use hiero_core::look::{ArrowLook, Look, LookElement, Shape, draw_diagram};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::routing::{
    CONNECTOR_BOX_FONT_SIZE, ConnectionEnds, Route, connector_placement, route_connections,
    straight_range, transformation_placement,
};
use hiero_core::scene::Scene;
use hiero_core::text::Weight;

use crate::model::{Conceptual, ConceptualKind};
use crate::rules::{Figure, figure_of, is_cloud};

/// An element's fill from the diagram's colours, and text that reads on it.
fn fill_of(eng: &Engine<Conceptual>, element: &Laid<Conceptual>) -> (Option<String>, Option<String>) {
    let colors = eng.colors();
    let figure = figure_of(&element.el);
    if element.el.kind == ConceptualKind::Section {
        return (None, Some(colors.section_names.clone()));
    }
    match figure {
        Some(Figure::Database | Figure::Storage) => return (Some(colors.data.clone()), None),
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
            let fill = role.of(colors).to_owned();
            let text = readable_text(&fill).to_owned();
            (Some(fill), Some(text))
        }
        None => (None, None),
    }
}

/// How an element looks.
pub fn look_of(eng: &Engine<Conceptual>, element: &Laid<Conceptual>) -> LookElement {
    let figure = figure_of(&element.el);
    let shape = if element.is_region {
        Shape::Container
    } else if element.el.kind == ConceptualKind::Section {
        Shape::Section
    } else {
        match figure {
            Some(Figure::Person | Figure::Bot) => Shape::Actor,
            Some(Figure::Database) => Shape::Database,
            Some(Figure::Storage) => Shape::Storage,
            None if is_cloud(&element.el) => Shape::Cloud,
            None => Shape::Box,
        }
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

/// Every arrow as a line between evenly spread points on its elements' facing sides.
/// `moving` shows an arrow being slid at its in-progress offset.
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
    let measure = eng.measure();
    eng.connections()
        .iter()
        .filter_map(|c| {
            let route = *routes.get(&c.id)?;
            let props = &c.props;
            let connector = props.connector.as_ref().map(|text| {
                let width = measure.width(text, CONNECTOR_BOX_FONT_SIZE, Weight::SemiBold);
                (text.clone(), connector_placement(route.to, route.to_side, width))
            });
            let to = connector.as_ref().map_or(route.to, |(_, p)| p.arrow_end);
            let transformation = props.transformation.as_ref().map(|text| {
                let width = measure.width(text, CONNECTOR_BOX_FONT_SIZE, Weight::SemiBold);
                (text.clone(), transformation_placement(route.from, to, width).0)
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
}
