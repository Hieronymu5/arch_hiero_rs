//! Builds a sample diagram of each type through the agent edit API and writes it as SVG and
//! draw.io files, for looking at the drawing outside the app.
//!
//! Usage: `cargo run -p hiero-registry --example render_samples -- <output folder>`

use hiero_core::export::drawio::Stamp;
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
                {"op": "add_element", "ref": "d", "kind": "domain", "label": "Sales", "x": 320, "y": 230},
                {"op": "add_element", "ref": "a", "kind": "application", "label": "CRM", "parent_id": "d", "description": "Customer records", "props": {"cloudHosted": true}},
                {"op": "add_element", "ref": "b", "kind": "application", "label": "Billing", "parent_id": "d"},
                {"op": "add_element", "ref": "u", "kind": "actor", "label": "Customer", "x": 700, "y": 60},
                {"op": "add_element", "ref": "bot", "kind": "actor", "label": "Assistant", "x": 700, "y": 220, "props": {"agent": true}},
                {"op": "add_element", "ref": "db", "kind": "component", "label": "Ledger", "x": 80, "y": 420, "props": {"componentType": "database"}},
                {"op": "add_connection", "source_id": "a", "target_id": "b", "props": {"label": "Invoices", "connector": "API", "transformation": "Map fields", "bidirectional": true}},
                {"op": "add_connection", "source_id": "u", "target_id": "a", "props": {"mode": "manual"}},
                {"op": "add_connection", "source_id": "b", "target_id": "db"}
            ]),
        ),
        (
            // Below the Start and End a new flowchart has, every other shape and arrow.
            DiagramTypeId::Flowchart,
            json!([
                {"op": "add_element", "ref": "io", "kind": "input-output", "label": "Receive order", "x": 80, "y": 240},
                {"op": "add_element", "ref": "d", "kind": "decision", "label": "In stock?", "x": 320, "y": 220},
                {"op": "add_element", "ref": "db", "kind": "data-store", "label": "Inventory", "x": 80, "y": 420},
                {"op": "add_element", "ref": "m", "kind": "manual", "label": "Pick and pack", "x": 580, "y": 240},
                {"op": "add_element", "ref": "w", "kind": "delay", "label": "Wait for courier", "x": 560, "y": 420},
                {"op": "add_element", "ref": "sub", "kind": "subprocess", "label": "Reorder stock", "x": 320, "y": 600},
                {"op": "add_element", "ref": "doc", "kind": "document", "label": "Invoice", "x": 820, "y": 420},
                {"op": "add_element", "ref": "c", "kind": "connector", "label": "A", "x": 620, "y": 620},
                {"op": "add_element", "ref": "n", "kind": "note", "label": "The courier comes at 4 pm", "x": 820, "y": 240},
                {"op": "add_element", "ref": "p", "kind": "process", "label": "Notify customer", "x": 320, "y": 780},
                {"op": "add_connection", "source_id": "io", "target_id": "d"},
                {"op": "add_connection", "source_id": "d", "target_id": "m"},
                {"op": "add_connection", "source_id": "d", "target_id": "sub"},
                {"op": "add_connection", "source_id": "db", "target_id": "d"},
                {"op": "add_connection", "source_id": "m", "target_id": "w"},
                {"op": "add_connection", "source_id": "w", "target_id": "c"},
                {"op": "add_connection", "source_id": "w", "target_id": "doc"},
                {"op": "add_connection", "source_id": "sub", "target_id": "p"},
                {"op": "add_connection", "source_id": "n", "target_id": "m"}
            ]),
        ),
        (
            // A pool of swimlanes, below the Start and End: steps handed from lane to lane.
            DiagramTypeId::Flowchart,
            json!([
                {"op": "add_element", "ref": "pool", "kind": "pool", "label": "Order fulfilment", "x": 40, "y": 200},
                {"op": "add_element", "ref": "l3", "kind": "lane", "label": "Warehouse", "parent_id": "pool"},
                {"op": "add_element", "ref": "a", "kind": "input-output", "label": "Submit order", "x": 100, "y": 280},
                {"op": "add_element", "ref": "b", "kind": "process", "label": "Validate order", "x": 360, "y": 500},
                {"op": "add_element", "ref": "c", "kind": "decision", "label": "Valid?", "x": 620, "y": 480},
                {"op": "add_element", "ref": "d", "kind": "process", "label": "Pick and pack", "x": 880, "y": 680},
                {"op": "add_connection", "source_id": "a", "target_id": "b"},
                {"op": "add_connection", "source_id": "b", "target_id": "c"},
                {"op": "add_connection", "source_id": "c", "target_id": "d"}
            ]),
        ),
        (
            // The same, with the lanes turned to run vertically.
            DiagramTypeId::Flowchart,
            json!([
                {"op": "add_element", "ref": "pool", "kind": "pool", "label": "Order fulfilment", "x": 40, "y": 200},
                {"op": "add_element", "ref": "l3", "kind": "lane", "label": "Warehouse", "parent_id": "pool"},
                {"op": "add_element", "ref": "a", "kind": "input-output", "label": "Submit order", "x": 100, "y": 280},
                {"op": "add_element", "ref": "b", "kind": "process", "label": "Validate order", "x": 360, "y": 500},
                {"op": "add_element", "ref": "c", "kind": "decision", "label": "Valid?", "x": 620, "y": 480},
                {"op": "add_element", "ref": "d", "kind": "process", "label": "Pick and pack", "x": 880, "y": 680},
                {"op": "add_connection", "source_id": "a", "target_id": "b"},
                {"op": "add_connection", "source_id": "b", "target_id": "c"},
                {"op": "add_connection", "source_id": "c", "target_id": "d"},
                {"op": "update_element", "id": "pool", "props": {"orientation": "vertical"}}
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
        let mut base = out.join(diagram_type.as_str());
        while base.with_extension("svg").exists() {
            base = base.with_file_name(format!(
                "{}-2",
                base.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
        std::fs::write(
            base.with_extension("svg"),
            t.to_svg(&diagram, Some(&Stamp::today("Version 1")))?,
        )?;
        std::fs::write(
            base.with_extension("drawio"),
            t.to_drawio(&diagram, Some(&Stamp::today("Version 1")))?,
        )?;
        println!("{}", base.display());
    }
    Ok(())
}
