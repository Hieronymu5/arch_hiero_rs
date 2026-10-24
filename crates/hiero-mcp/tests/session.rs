//! Runs the real `arch-hiero-mcp` binary as a child process and drives it as an agent would.

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ReadResourceRequestParams};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::{Value, json};

fn text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &'static str,
    args: Value,
) -> (bool, String) {
    let result = client
        .call_tool(
            CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .expect("the tool call completes");
    (result.is_error != Some(true), text(&result))
}

#[tokio::test]
async fn an_agent_builds_versions_and_exports_a_physical_diagram() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("agent.sqlite3");
    let transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_arch-hiero-mcp")).configure(|cmd| {
            cmd.arg("--db").arg(&db);
        }),
    )
    .unwrap();
    let client = ().serve(transport).await.expect("the server starts");

    // The tools, with the destructive ones marked.
    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    for expected in [
        "list_diagram_types",
        "edit_diagram",
        "save_version",
        "revert_to_version",
        "export_diagram",
    ] {
        assert!(
            names.contains(&expected.to_owned()),
            "{expected} is offered"
        );
    }
    let delete = tools.iter().find(|t| t.name == "delete_diagram").unwrap();
    assert_eq!(
        delete.annotations.as_ref().and_then(|a| a.destructive_hint),
        Some(true)
    );

    // The types list the technologies a deployed component can be built on.
    let (ok, types) = call(&client, "list_diagram_types", json!({})).await;
    assert!(ok && types.contains(r#""id": "springboot""#));

    // The seeds are there, and the type specs are resources.
    let (ok, list) = call(&client, "list_diagrams", json!({})).await;
    assert!(ok && list.contains("Enterprise Overview"));
    let spec = client
        .read_resource(ReadResourceRequestParams::new(
            "arch-hiero://types/physical",
        ))
        .await
        .unwrap();
    assert!(format!("{spec:?}").contains("Zones fill their network"));

    // Create a physical diagram and find its Perimeter zone.
    let (ok, created) = call(
        &client,
        "create_diagram",
        json!({"name": "Agent infra", "type": "physical"}),
    )
    .await;
    assert!(ok, "{created}");
    let created: Value = serde_json::from_str(&created).unwrap();
    let id = created["diagram"]["id"].as_str().unwrap().to_owned();
    let perimeter = created["diagram"]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["label"] == "Perimeter")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Save a version, then edit with the type's rules.
    let (ok, saved) = call(
        &client,
        "save_version",
        json!({"id": id, "description": "Before agent edits"}),
    )
    .await;
    assert!(ok && saved.contains("Already saved"), "{saved}");
    let (ok, edited) = call(
        &client,
        "edit_diagram",
        json!({"id": id, "ops": [
            {"op": "add_element", "ref": "web", "kind": "server", "label": "Web", "parent_id": perimeter},
            {"op": "add_element", "ref": "db", "kind": "database", "label": "Orders", "parent_id": perimeter,
             "props": {"databaseType": "postgresql"}},
            {"op": "add_connection", "source_id": "web", "target_id": "db"}
        ]}),
    )
    .await;
    assert!(ok, "{edited}");
    let edited: Value = serde_json::from_str(&edited).unwrap();
    assert_eq!(edited["status"], "Draft · v1");
    assert_eq!(edited["diagram"]["connections"][0]["port"], "5432");

    // A rule violation rejects the whole batch with a reason.
    let (ok, refused) = call(
        &client,
        "edit_diagram",
        json!({"id": id, "ops": [{"op": "add_element", "kind": "firewall", "parent_id": perimeter}]}),
    )
    .await;
    assert!(
        !ok && refused.contains("a firewall can't go inside “Perimeter”"),
        "{refused}"
    );

    // A stale revision is refused.
    let (ok, stale) = call(
        &client,
        "edit_diagram",
        json!({"id": id, "ops": [], "expected_revision": 0}),
    )
    .await;
    assert!(
        !ok && stale.contains("changed since you read it"),
        "{stale}"
    );

    // Export SVG as text, and a zip to a file.
    let (ok, svg) = call(
        &client,
        "export_diagram",
        json!({"id": id, "format": "svg"}),
    )
    .await;
    assert!(ok && svg.contains("<svg") && svg.contains(">PSQL</text>"));
    let zip = dir.path().join("infra.zip");
    let (ok, wrote) = call(
        &client,
        "export_diagram",
        json!({"id": id, "format": "zip", "path": zip}),
    )
    .await;
    assert!(ok && zip.exists(), "{wrote}");
    let (ok, again) = call(
        &client,
        "export_diagram",
        json!({"id": id, "format": "zip", "path": zip}),
    )
    .await;
    assert!(!ok && again.contains("overwrite"), "{again}");

    // Reverting with unsaved changes needs discard_unsaved; then it records a new version.
    let (ok, refused) = call(
        &client,
        "revert_to_version",
        json!({"id": id, "version": 1}),
    )
    .await;
    assert!(!ok && refused.contains("save_version first"), "{refused}");
    let (ok, reverted) = call(
        &client,
        "revert_to_version",
        json!({"id": id, "version": 1, "discard_unsaved": true}),
    )
    .await;
    assert!(ok && reverted.contains("recorded as v2"), "{reverted}");
    let (_, versions) = call(&client, "list_versions", json!({"id": id})).await;
    let versions: Value = serde_json::from_str(&versions).unwrap();
    assert_eq!(versions[0]["description"], "Reverted to v1");

    // Importing the exported file puts the server and database back.
    let (ok, imported) = call(&client, "import_diagram", json!({"path": zip})).await;
    assert!(ok && imported.starts_with("Updated"), "{imported}");
    let (_, after) = call(&client, "get_diagram", json!({"id": id})).await;
    assert!(after.contains("\"Orders\""));

    client.cancel().await.unwrap();
}
