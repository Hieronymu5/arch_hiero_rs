//! How a flowchart is drawn: each element in its shape and colour, and arrows as solid lines
//! for the flow, dashed for data, and dotted without a head for a note's link, each ending on
//! its elements' outlines.

use std::collections::{HashMap, HashSet};

use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{DrawioArrow, Heading, diagram_to_drawio};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{ArrowLook, DiagramColors, Look, LookElement, NameAt, Shape, draw_diagram};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::routing::{ConnectionEnds, Side, route_connections};
use hiero_core::scene::Scene;

use crate::lanes::is_vertical;
use crate::model::{Flowchart, FlowchartKind, LANE_ALTERNATE};

/// The colour of arrows.
pub const LINES: &str = "#333333";

/// The colours the shared drawing code takes. Elements set their own; these are for arrows.
pub fn colors() -> DiagramColors {
    DiagramColors {
        borders: LINES.into(),
        lines: LINES.into(),
        ..DiagramColors::default()
    }
}

/// How an element looks. `alternate` gives a lane the second of the two lane fills.
pub fn look_of(element: &Laid<Flowchart>, alternate: bool) -> LookElement {
    let kind = element.el.kind;
    let colors = kind.colors();
    let look = match kind.shape() {
        Some(shape) => Look {
            fill: Some(colors.fill.into()),
            border_color: Some(colors.border.into()),
            bold_border: kind == FlowchartKind::End,
            ..Look::new(Shape::Flow(shape))
        },
        // A pool's fill shows only in its header, as its lanes cover the rest.
        None => Look {
            fill: Some(if alternate {
                LANE_ALTERNATE.into()
            } else {
                colors.fill.into()
            }),
            border_color: Some(colors.border.into()),
            name_at: NameAt::TopLeft,
            square: true,
            ..Look::new(Shape::Container)
        },
    };
    LookElement {
        id: element.el.id.clone(),
        parent_id: element.el.parent_id.clone(),
        label: element.el.label.clone(),
        description: None,
        look,
        rect: element.rect,
    }
}

/// Every element's look, every other lane in each pool in the second lane fill.
pub fn looks(layout: &[Laid<Flowchart>]) -> Vec<LookElement> {
    let alternate: HashSet<&ElementId> = layout
        .iter()
        .filter(|e| e.el.kind == FlowchartKind::Pool)
        .flat_map(|pool| {
            let vertical = is_vertical(&pool.el);
            let mut lanes: Vec<&Laid<Flowchart>> = layout
                .iter()
                .filter(|l| l.el.parent_id.as_ref() == Some(pool.id()))
                .collect();
            lanes.sort_by(|a, b| {
                let at = |r: Rect| if vertical { r.x } else { r.y };
                at(a.rect)
                    .partial_cmp(&at(b.rect))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            lanes.into_iter().skip(1).step_by(2).map(Laid::id)
        })
        .collect();
    layout
        .iter()
        .map(|e| look_of(e, alternate.contains(e.id())))
        .collect()
}

/// Where an arrow end at `point` on `side` of `element` is drawn: on its shape's outline.
fn on_outline(element: Option<&Laid<Flowchart>>, side: Side, point: Point) -> Point {
    match element.and_then(|e| Some((e, e.el.kind.shape()?))) {
        Some((e, shape)) => shape.edge_point(e.rect, side, point),
        None => point,
    }
}

/// An arrow as it's drawn.
#[derive(Debug, Clone)]
pub struct DrawnArrow {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub look: ArrowLook,
}

/// Every arrow, between its elements' facing sides.
pub fn drawn_arrows(eng: &Engine<Flowchart>, layout: &[Laid<Flowchart>]) -> Vec<DrawnArrow> {
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
    let by_id: HashMap<&ElementId, &Laid<Flowchart>> =
        layout.iter().map(|e| (&e.el.id, e)).collect();
    eng.connections()
        .iter()
        .filter_map(|c| {
            let route = routes.get(&c.id)?;
            let source = by_id.get(&c.source_id).copied();
            let target = by_id.get(&c.target_id).copied();
            let kinds = [source, target].map(|e| e.map(|e| e.el.kind));
            let annotation = kinds.contains(&Some(FlowchartKind::Note));
            let data = kinds.iter().flatten().any(|k| k.is_data());
            Some(DrawnArrow {
                id: c.id.clone(),
                source_id: c.source_id.clone(),
                target_id: c.target_id.clone(),
                look: ArrowLook {
                    from: on_outline(source, route.from_side, route.from),
                    to: on_outline(target, route.to_side, route.to),
                    dashed: data && !annotation,
                    annotation,
                    label: c.props.label.clone(),
                    ..ArrowLook::default()
                },
            })
        })
        .collect()
}

/// The diagram drawn from `layout`, such as the engine's, or one being dragged.
pub fn scene_of(eng: &Engine<Flowchart>, layout: &[Laid<Flowchart>]) -> Scene {
    let elements = looks(layout);
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
pub fn scene(eng: &Engine<Flowchart>) -> Scene {
    scene_of(eng, eng.layout())
}

/// The diagram as a draw.io file, with `heading` above it.
pub fn to_drawio(eng: &Engine<Flowchart>, title: &str, heading: Option<&Heading>) -> String {
    let elements = looks(eng.layout());
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
        None,
        &colors(),
        eng.line_weight(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    fn engine() -> Engine<Flowchart> {
        Engine::<Flowchart>::new(
            serde_json::from_value(serde_json::json!([
                { "id": "s", "kind": "start", "label": "Order placed", "x": 0, "y": 0 },
                { "id": "d", "kind": "decision", "label": "In stock?", "x": 0, "y": 200 },
                { "id": "p", "kind": "process", "label": "Ship order", "x": 400, "y": 220 },
                { "id": "e", "kind": "end", "label": "Done", "x": 400, "y": 400 },
                { "id": "db", "kind": "data-store", "label": "Stock", "x": 0, "y": 400 },
                { "id": "n", "kind": "note", "label": "Ships daily", "x": 800, "y": 200 },
            ]))
            .unwrap(),
            serde_json::from_value(serde_json::json!([
                { "id": "a1", "sourceId": "s", "targetId": "d" },
                { "id": "a2", "sourceId": "d", "targetId": "p", "label": "Yes" },
                { "id": "a3", "sourceId": "p", "targetId": "e" },
                { "id": "a4", "sourceId": "db", "targetId": "d" },
                { "id": "a5", "sourceId": "n", "targetId": "p" },
            ]))
            .unwrap(),
            NoColors {},
        )
        .with_measure(&ESTIMATE)
    }

    #[test]
    fn arrows_look_like_what_they_join() {
        let e = engine();
        let arrows = drawn_arrows(&e, e.layout());
        let look = |id: &str| &arrows.iter().find(|a| a.id.as_str() == id).unwrap().look;
        assert!(!look("a1").dashed && !look("a1").annotation);
        assert!(look("a4").dashed);
        assert!(look("a5").annotation && !look("a5").dashed);
        assert_eq!(look("a2").label.as_deref(), Some("Yes"));
        // Down from the start's flat bottom onto the diamond's top point.
        assert_eq!(look("a1").from, Point::new(80.0, 40.0));
        assert_eq!(look("a1").to, Point::new(80.0, 200.0));
        // Centred level with each other, out of the diamond's right point.
        assert_eq!(look("a2").from, Point::new(160.0, 260.0));
    }

    #[test]
    fn draws_each_kind_in_its_colour() {
        let svg = scene_to_svg("Test", &scene(&engine()));
        // The start green, the end red with a bold border, the decision amber.
        assert!(
            svg.contains(r##"fill="#d5e8d4" stroke="#356b24""##),
            "{svg}"
        );
        assert!(svg.contains(r##"fill="#f8cecc" stroke="#9c3530" stroke-width="3""##));
        assert!(svg.contains(r##"fill="#fff2cc" stroke="#9a7210""##));
        assert!(svg.contains(">In stock?</text>"));
        assert!(svg.contains(">Yes</text>"));
        // The note's link is dotted, with no head.
        assert!(svg.contains(r#"stroke-dasharray="2 4""#));
    }

    #[test]
    fn exports_to_drawio() {
        let xml = to_drawio(&engine(), "Test", None);
        assert!(xml.contains("rhombus=1"));
        assert!(xml.contains("shape=cylinder3"));
        assert!(xml.contains("endArrow=none"));
        assert!(xml.contains(r#"value="Yes""#));
    }
}
