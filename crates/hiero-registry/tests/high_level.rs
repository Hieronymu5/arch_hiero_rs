//! A high level conceptual diagram built by an agent's edits: applications in groups, a group
//! in a group, an empty one, looks of their own, and wide arrows, kept through a file.

use hiero_core::file::diagram_to_zip;
use hiero_core::model::DiagramTypeId;
use hiero_registry::edit::EditOp;
use hiero_registry::{new_diagram, ops, read_diagram_file};
use serde_json::{Value, json};

fn edits(value: Value) -> Vec<EditOp> {
    serde_json::from_value(value).unwrap()
}

#[test]
fn agents_build_groups_of_groups_with_arrows_between_them() {
    let t = ops(DiagramTypeId::HighLevelConceptual);
    let mut diagram = new_diagram(DiagramTypeId::HighLevelConceptual, "Enterprise", None);
    assert!(diagram.content.elements.is_empty());
    let outcome = t
        .apply(
            &diagram,
            &edits(json!([
                {"op": "add_element", "ref": "ea", "kind": "group", "label": "Enterprise",
                 "x": 40, "y": 40, "props": {"fontSize": 24}},
                {"op": "add_element", "ref": "is", "kind": "group", "label": "Information Systems",
                 "parent_id": "ea", "props": {"color": "blue"}},
                {"op": "add_element", "ref": "crm", "kind": "application", "label": "CRM",
                 "parent_id": "is", "props": {"color": "green"}},
                {"op": "add_element", "ref": "sec", "kind": "group", "label": "Security",
                 "x": 1200, "y": 40, "props": {"dashed": true, "color": "red"}},
                {"op": "add_element", "ref": "app", "kind": "application", "label": "Portal",
                 "x": 1200, "y": 800, "props": {"color": "grey", "fontSize": 18}},
                {"op": "add_connection", "ref": "a1", "source_id": "ea", "target_id": "sec",
                 "props": {"twoWay": true}},
                {"op": "add_connection", "ref": "a2", "source_id": "app", "target_id": "crm"}
            ])),
        )
        .unwrap();
    let id = |r: &str| outcome.created[r].clone();
    let element = |r: &str| {
        outcome
            .content
            .elements
            .iter()
            .find(|e| e.id.0 == id(r))
            .unwrap()
            .clone()
    };
    assert_eq!(element("is").parent_id.unwrap().0, id("ea"));
    assert_eq!(element("crm").parent_id.unwrap().0, id("is"));
    assert_eq!(element("sec").props["dashed"], true);
    assert_eq!(element("app").props["fontSize"], 18);
    assert_eq!(outcome.content.connections[0].props["twoWay"], true);
    // A group and what it holds can't be joined.
    assert!(
        t.apply(
            &diagram,
            &edits(json!([
                {"op": "add_element", "ref": "g", "kind": "group", "label": "G"},
                {"op": "add_element", "ref": "a", "kind": "application", "label": "A",
                 "parent_id": "g"},
                {"op": "add_connection", "source_id": "g", "target_id": "a"}
            ]))
        )
        .is_err()
    );
    // Nor is a font size out of range.
    assert!(
        t.apply(
            &diagram,
            &edits(json!([
                {"op": "add_element", "kind": "application", "label": "A",
                 "props": {"fontSize": 100}}
            ]))
        )
        .is_err()
    );
    diagram.content = outcome.content;
    diagram.meta.created_at = hiero_core::model::now_rfc3339();
    diagram.meta.updated_at = diagram.meta.created_at.clone();
    t.validate(&diagram.content).unwrap();
    let back = read_diagram_file(&diagram_to_zip(&diagram)).unwrap();
    assert_eq!(back.content.elements, diagram.content.elements);
    assert_eq!(back.content.connections, diagram.content.connections);
    let svg = t.to_svg(&diagram, None).unwrap();
    assert!(svg.contains(">Information Systems</text>"));
    let xml = t.to_drawio(&diagram, None).unwrap();
    assert!(xml.contains("shape=flexArrow"));
    assert!(xml.contains("Portal"));
}
