//! Edits a diagram from another process, as an agent through the MCP server would: adds an
//! actor called "Agent" to the diagram named in the second argument.
//!
//! Usage: `cargo run -p hiero-mcp --example agent_edit -- <db> <diagram name>`

use hiero_registry::{EditOp, ops};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let db = args.next().ok_or("a database path")?;
    let name = args.next().ok_or("a diagram name")?;
    let mut store = hiero_store::Store::open(std::path::Path::new(&db))?;
    let summary = store
        .list()?
        .into_iter()
        .find(|s| s.meta.name == name)
        .ok_or("no such diagram")?;
    let record = store.get(&summary.meta.id)?.ok_or("gone")?;
    let edit: EditOp = serde_json::from_value(serde_json::json!({
        "op": "add_element", "kind": "actor", "label": "Agent", "props": {"agent": true}
    }))?;
    let outcome = ops(record.diagram.meta.diagram_type).apply(&record.diagram, &[edit])?;
    let mut next = record.diagram.clone();
    next.content = outcome.content;
    store.save(&next, Some(record.revision))?;
    println!("{}", outcome.messages.join("\n"));
    Ok(())
}
