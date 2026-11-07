//! A sequence diagram built by an agent's edits: participants in a row, messages in time
//! order, repeated and to the sender itself, moved, and kept through a file.

use hiero_core::file::diagram_to_zip;
use hiero_core::model::DiagramTypeId;
use hiero_registry::edit::EditOp;
use hiero_registry::{new_diagram, ops, read_diagram_file};
use serde_json::{Value, json};

fn edits(value: Value) -> Vec<EditOp> {
    serde_json::from_value(value).unwrap()
}

#[test]
fn agents_add_order_and_keep_messages() {
    let t = ops(DiagramTypeId::Sequence);
    let mut diagram = new_diagram(DiagramTypeId::Sequence, "Orders", None);
    let ids: Vec<String> = diagram
        .content
        .elements
        .iter()
        .map(|e| e.id.0.clone())
        .collect();
    let (user, app) = (&ids[0], &ids[1]);
    let outcome = t
        .apply(
            &diagram,
            &edits(json!([
                {"op": "add_element", "ref": "db", "kind": "component", "label": "Orders DB",
                 "props": {"componentType": "database", "color": "yellow"}},
                {"op": "add_connection", "ref": "m1", "source_id": user, "target_id": app,
                 "props": {"label": "Place order"}},
                {"op": "add_connection", "ref": "m2", "source_id": app, "target_id": "db",
                 "props": {"label": "Save"}},
                {"op": "add_connection", "ref": "m3", "source_id": "db", "target_id": app},
                {"op": "add_connection", "ref": "m4", "source_id": app, "target_id": app,
                 "props": {"label": "Check stock"}},
                {"op": "add_connection", "ref": "m5", "source_id": user, "target_id": app,
                 "props": {"kind": "async", "label": "Track"}},
                {"op": "move_connection", "id": "m4", "index": 1},
                // Put the database first: x sets the order.
                {"op": "move_element", "id": "db", "x": 0, "y": 500}
            ])),
        )
        .unwrap();
    let order: Vec<&str> = outcome
        .content
        .connections
        .iter()
        .map(|c| c.id.0.as_str())
        .collect();
    let id = |r: &str| outcome.created[r].as_str();
    assert_eq!(order, [id("m1"), id("m4"), id("m2"), id("m3"), id("m5")]);
    // The message straight back after a call starts as a return.
    let m3 = &outcome.content.connections[3];
    assert_eq!(m3.props["kind"], "return");
    assert_eq!(outcome.messages[6], "Message is now 2 of 5.");
    // The database is first in the row, and every participant stays at the top.
    let db = id("db");
    let row: Vec<(&str, f64)> = {
        let mut row: Vec<(&str, f64)> = outcome
            .content
            .elements
            .iter()
            .map(|e| (e.id.0.as_str(), e.x))
            .collect();
        row.sort_by(|a, b| a.1.total_cmp(&b.1));
        row
    };
    assert_eq!(row[0].0, db);
    assert!(outcome.content.elements.iter().all(|e| e.y == 40.0));
    // An index past the end is refused.
    assert!(
        t.apply(
            &diagram,
            &edits(json!([
                {"op": "add_connection", "ref": "m", "source_id": user, "target_id": app},
                {"op": "move_connection", "id": "m", "index": 1}
            ]))
        )
        .is_err()
    );
    diagram.content = outcome.content;
    diagram.meta.created_at = hiero_core::model::now_rfc3339();
    diagram.meta.updated_at = diagram.meta.created_at.clone();
    t.validate(&diagram.content).unwrap();
    // Through a file, with the repeated messages and the one to itself, in their order.
    let back = read_diagram_file(&diagram_to_zip(&diagram)).unwrap();
    assert_eq!(back.content.connections, diagram.content.connections);
    let svg = t.to_svg(&diagram, None).unwrap();
    assert!(svg.contains("Check stock"));
    assert!(t.to_drawio(&diagram, None).unwrap().contains("Orders DB"));
}
