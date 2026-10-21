//! Builds a sample diagram of each type through the agent edit API and writes it as SVG and
//! draw.io files, for looking at the drawing outside the app.
//!
//! Usage: `cargo run -p hiero-registry --example render_samples -- <output folder>`

use hiero_core::model::DiagramTypeId;
use hiero_registry::{EditOp, new_diagram, ops};
use serde_json::json;

fn edits(value: serde_json::Value) -> Vec<EditOp> {
    serde_json::from_value(value).expect("valid edits")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("an output folder")?);
    std::fs::create_dir_all(&out)?;

    let samples = [
        (
            DiagramTypeId::DeployedComponent,
            json!([
                {"op": "add_element", "ref": "web", "kind": "layer", "label": "Web", "props": {"characteristics": ["Internet facing", "PCI DSS"]}},
                {"op": "add_element", "kind": "component", "label": "Customer Portal", "parent_id": "web",
                 "description": "Serves pages\nHandles sign-in\nCaches sessions\nSends events\nRenders reports",
                 "props": {"protocols": ["HTTPS", "JWT", "XSRF"], "techStack": ["aws", "docker", "java"], "instanceCount": 3}},
                {"op": "add_element", "kind": "component", "label": "Payments", "parent_id": "web",
                 "description": "Takes card payments", "props": {"dependency": "external", "protocols": ["HTTPS"]}},
                {"op": "add_element", "ref": "data", "kind": "layer", "label": "Data"},
                {"op": "add_element", "kind": "component", "label": "Orders DB", "parent_id": "data",
                 "description": "Orders\nCustomers", "props": {"dependency": "internal", "techStack": ["postgresql"]}}
            ]),
        ),
        (
            DiagramTypeId::Conceptual,
            json!([
                {"op": "add_element", "ref": "d", "kind": "domain", "label": "Sales", "x": 40, "y": 40},
                {"op": "add_element", "ref": "a", "kind": "application", "label": "CRM", "parent_id": "d", "description": "Customer records", "props": {"cloudHosted": true}},
                {"op": "add_element", "ref": "b", "kind": "application", "label": "Billing", "parent_id": "d"},
                {"op": "add_element", "ref": "u", "kind": "actor", "label": "Customer", "x": 700, "y": 60},
                {"op": "add_element", "ref": "bot", "kind": "actor", "label": "Assistant", "x": 700, "y": 220, "props": {"agent": true}},
                {"op": "add_element", "ref": "db", "kind": "component", "label": "Ledger", "x": 360, "y": 360, "props": {"componentType": "database"}},
                {"op": "add_connection", "source_id": "a", "target_id": "b", "props": {"label": "Invoices", "connector": "API", "transformation": "Map fields", "bidirectional": true}},
                {"op": "add_connection", "source_id": "u", "target_id": "a", "props": {"mode": "manual"}},
                {"op": "add_connection", "source_id": "b", "target_id": "db"}
            ]),
        ),
    ];
    for (diagram_type, ops_json) in samples {
        let mut diagram = new_diagram(
            diagram_type,
            &format!("Sample {}", diagram_type.label()),
            Some("A sample".into()),
        );
        let t = ops(diagram_type);
        let outcome = t.apply(&diagram, &edits(ops_json))?;
        diagram.content = outcome.content;
        let base = out.join(diagram_type.as_str());
        std::fs::write(base.with_extension("svg"), t.to_svg(&diagram)?)?;
        std::fs::write(base.with_extension("drawio"), t.to_drawio(&diagram)?)?;
        println!("{}", base.display());
    }
    Ok(())
}
