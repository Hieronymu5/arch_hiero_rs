//! The MCP tools and resources: diagrams read and edited through each diagram type's own
//! rules, versions, and import and export.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hiero_core::file::{diagram_from_zip, diagram_to_json, diagram_to_zip};
use hiero_core::model::{DiagramId, DiagramTypeId, validate_description, validate_name};
use hiero_registry::{EditOp, ops, types};
use hiero_store::{Store, StoreError};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, ListResourcesResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

/// What the server tells agents about using it.
pub const INSTRUCTIONS: &str = "Arch Hiero keeps architecture diagrams (conceptual, physical, and deployed component) in a local database that the Arch Hiero app also uses; changes you make show up in the app live and can be undone there. Before editing, call list_diagram_types to learn each type's kinds of element, what they can contain and connect to, and their properties, and read the arch-hiero://types/{type} resource for the full rules. Every edit goes through the type's own rules (for example, zones fill networks and layers stack), so positions you give may be adjusted. Batch related changes into one edit_diagram call; a batch is all or nothing and names the edit that failed and why. Before large changes, call save_version with a description such as \"Before agent edits\" so the user can go back. Your edits leave the diagram a Draft until a version is saved, which makes it a Release. delete_diagram, revert_to_version, and delete_version are destructive.";

/// The type specs, as resources.
const SPECS: [(DiagramTypeId, &str); 3] = [
    (
        DiagramTypeId::Conceptual,
        include_str!("../../../docs/diagrams/conceptual.md"),
    ),
    (
        DiagramTypeId::Physical,
        include_str!("../../../docs/diagrams/physical.md"),
    ),
    (
        DiagramTypeId::DeployedComponent,
        include_str!("../../../docs/diagrams/deployed-component.md"),
    ),
];

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DiagramParams {
    /// The diagram's id, from list_diagrams.
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDiagramParams {
    /// The diagram's id, from list_diagrams.
    pub id: String,
    /// A saved version to read instead of the current diagram.
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    /// The diagram's name, at most 100 characters.
    pub name: String,
    /// "conceptual", "physical", or "deployed-component". It can't be changed later.
    #[serde(rename = "type")]
    pub diagram_type: String,
    /// What the diagram shows, at most 1000 characters.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DetailsParams {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// An empty string clears it.
    #[serde(default)]
    pub description: Option<String>,
    /// Colours to set, by key from list_diagram_types, as "#rrggbb". Only colours that differ
    /// from the defaults are kept. An empty object resets them all.
    #[serde(default)]
    pub colors: Option<BTreeMap<String, String>>,
    /// The revision from get_diagram. If the diagram changed since, the update is refused.
    #[serde(default)]
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditParams {
    pub id: String,
    /// Edits applied in order, all or nothing. Each has an "op": add_element {ref?, kind, label?,
    /// description?, parent_id?, x?, y?, props?}, update_element {id, label?, description?,
    /// parent_id? ("" takes it out of its container), props? (merged; null removes a key)},
    /// move_element {id, x, y}, resize_element {id, width, height}, remove_element {id},
    /// add_connection {ref?, source_id, target_id, props?}, update_connection {id, props},
    /// remove_connection {id}. Any id can be a ref given to an element added earlier in the
    /// same batch.
    pub ops: Vec<Value>,
    /// The revision from get_diagram. If the diagram changed since, the edit is refused.
    #[serde(default)]
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveVersionParams {
    pub id: String,
    /// An optional short description, at most 200 characters.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RevertParams {
    pub id: String,
    pub version: i64,
    /// The new version's description. Defaults to "Reverted to vN".
    #[serde(default)]
    pub description: Option<String>,
    /// Revert even though the diagram has changes not saved as a version, losing them.
    #[serde(default)]
    pub discard_unsaved: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VersionParams {
    pub id: String,
    pub version: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExportParams {
    pub id: String,
    /// "zip" (the diagram file), "svg", or "drawio".
    pub format: String,
    /// A saved version to export instead of the current diagram.
    #[serde(default)]
    pub version: Option<i64>,
    /// Where to write the file. Required for zip; optional for svg and drawio, which are also
    /// returned as text. Its extension must match the format.
    #[serde(default)]
    pub path: Option<String>,
    /// Replace a file that's already there.
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportParams {
    /// A .zip diagram file, as exported by Arch Hiero or its prototype.
    pub path: String,
}

/// The MCP server.
#[derive(Clone)]
pub struct HieroServer {
    store: Arc<Mutex<Store>>,
    tool_router: ToolRouter<Self>,
}

fn text(value: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(value.into())])
}

fn json_result(value: &Value) -> CallToolResult {
    text(serde_json::to_string_pretty(value).unwrap_or_default())
}

/// A failure the agent should read and act on, rather than a protocol error.
fn failure(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

fn store_failure(e: &StoreError) -> CallToolResult {
    match e {
        StoreError::Conflict => failure(
            "The diagram changed since you read it (perhaps in the app). Call get_diagram again and redo your change.",
        ),
        other => failure(other.to_string()),
    }
}

type ToolResult = Result<CallToolResult, ErrorData>;

impl HieroServer {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            tool_router: Self::tool_router(),
        }
    }

    fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        let mut store = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut store)
    }

    /// A diagram as agents see it: the file's JSON, its revision and status, and where each
    /// element is laid out.
    fn describe(&self, id: &DiagramId) -> Result<Value, CallToolResult> {
        self.with_store(|store| {
            let record = store
                .get(id)
                .map_err(|e| store_failure(&e))?
                .ok_or_else(|| failure(format!("There's no diagram “{id}”.")))?;
            let status = store.status(id).map_err(|e| store_failure(&e))?;
            let layout = ops(record.diagram.meta.diagram_type)
                .layout(&record.diagram)
                .map(|items| {
                    items
                        .into_iter()
                        .map(|(id, r)| json!({"id": id, "x": r.x, "y": r.y, "width": r.width, "height": r.height}))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(json!({
                "revision": record.revision,
                "status": status.label(),
                "saved": status.saved,
                "latestVersion": status.latest,
                "diagram": diagram_to_json(&record.diagram),
                "layout": layout,
            }))
        })
    }

    fn write_file(
        path: &str,
        extension: &str,
        overwrite: bool,
        bytes: &[u8],
    ) -> Result<PathBuf, String> {
        let path = PathBuf::from(path);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if !ext.eq_ignore_ascii_case(extension) {
            return Err(format!("The path must end in .{extension}."));
        }
        if path.exists() && !overwrite {
            return Err(format!(
                "{} already exists. Pass overwrite: true to replace it.",
                path.display()
            ));
        }
        std::fs::write(&path, bytes)
            .map_err(|e| format!("{} couldn't be written: {e}", path.display()))?;
        Ok(path)
    }
}

#[tool_router]
impl HieroServer {
    #[tool(
        description = "Every diagram type, with its kinds of element (what each can contain and connect to), what its elements' and arrows' props can be, and its colours.",
        annotations(read_only_hint = true)
    )]
    pub async fn list_diagram_types(&self) -> ToolResult {
        let list: Vec<Value> = types()
            .iter()
            .map(|t| {
                json!({
                    "type": t.id().as_str(),
                    "label": t.label(),
                    "hasArrows": t.id().has_arrows(),
                    "kinds": t.palette(),
                    "props": t.props_doc(),
                    "colors": t.color_fields().iter().map(|(k, l)| json!({"key": k, "label": l})).collect::<Vec<_>>(),
                    "spec": format!("arch-hiero://types/{}", t.id().as_str()),
                })
            })
            .collect();
        Ok(json_result(&Value::Array(list)))
    }

    #[tool(
        description = "Every diagram: id, name, type, when it was last updated, its version tag (Release when it matches its latest version, Draft when it has changed since), and its revision.",
        annotations(read_only_hint = true)
    )]
    pub async fn list_diagrams(&self) -> ToolResult {
        match self.with_store(|s| s.list()) {
            Ok(list) => Ok(json_result(&Value::Array(
                list.into_iter()
                    .map(|s| {
                        json!({
                            "id": s.meta.id, "name": s.meta.name, "type": s.meta.diagram_type.as_str(),
                            "updatedAt": s.meta.updated_at, "status": s.status.label(),
                            "saved": s.status.saved, "latestVersion": s.status.latest, "revision": s.revision,
                        })
                    })
                    .collect(),
            ))),
            Err(e) => Ok(store_failure(&e)),
        }
    }

    #[tool(
        description = "A diagram: its JSON (elements with their props, and arrows), its revision (pass it back to edits), its Release/Draft version tag, and each element's laid-out position and size. With `version`, a saved version instead.",
        annotations(read_only_hint = true)
    )]
    pub async fn get_diagram(&self, Parameters(p): Parameters<GetDiagramParams>) -> ToolResult {
        let id = DiagramId(p.id);
        if let Some(version) = p.version {
            return Ok(match self.with_store(|s| s.get_version(&id, version)) {
                Ok(Some(d)) => {
                    json_result(&json!({"version": version, "diagram": diagram_to_json(&d)}))
                }
                Ok(None) => failure(format!("There's no v{version} of “{id}”.")),
                Err(e) => store_failure(&e),
            });
        }
        Ok(match self.describe(&id) {
            Ok(value) => json_result(&value),
            Err(result) => result,
        })
    }

    #[tool(
        description = "Creates a diagram and records it as version 1. A physical diagram starts with an End User zone and an Intranet network of three zones."
    )]
    pub async fn create_diagram(&self, Parameters(p): Parameters<CreateParams>) -> ToolResult {
        let Some(diagram_type) = DiagramTypeId::parse(&p.diagram_type) else {
            return Ok(failure(
                "The type must be \"conceptual\", \"physical\", or \"deployed-component\".",
            ));
        };
        if let Err(m) = validate_name(&p.name) {
            return Ok(failure(m));
        }
        if let Err(m) = validate_description(p.description.as_deref().unwrap_or_default()) {
            return Ok(failure(m));
        }
        let diagram = hiero_registry::new_diagram(diagram_type, &p.name, p.description);
        match self.with_store(|s| s.create(&diagram)) {
            Ok(record) => Ok(match self.describe(&record.diagram.meta.id) {
                Ok(v) => json_result(&v),
                Err(r) => r,
            }),
            Err(e) => Ok(store_failure(&e)),
        }
    }

    #[tool(
        description = "Changes a diagram's name, description, or colours. Its type can't be changed."
    )]
    pub async fn update_diagram_details(
        &self,
        Parameters(p): Parameters<DetailsParams>,
    ) -> ToolResult {
        let id = DiagramId(p.id);
        let result = self.with_store(|store| {
            let mut record = store.get(&id)?.ok_or(StoreError::NotFound)?;
            let d = &mut record.diagram;
            if let Some(name) = p.name {
                validate_name(&name).map_err(|m| StoreError::Corrupt(m.into()))?;
                d.meta.name = name.trim().to_owned();
            }
            if let Some(description) = p.description {
                validate_description(&description).map_err(|m| StoreError::Corrupt(m.into()))?;
                d.meta.description = Some(description.trim().to_owned()).filter(|s| !s.is_empty());
            }
            if let Some(colors) = p.colors {
                d.colors = ops(d.meta.diagram_type).normalize_colors(&colors);
            }
            store.save(
                &record.diagram,
                p.expected_revision.or(Some(record.revision)),
            )
        });
        Ok(match result {
            Ok(record) => match self.describe(&record.diagram.meta.id) {
                Ok(v) => json_result(&v),
                Err(r) => r,
            },
            Err(StoreError::Corrupt(m)) => failure(m),
            Err(e) => store_failure(&e),
        })
    }

    #[tool(
        description = "Deletes a diagram and all its versions. This can't be undone.",
        annotations(destructive_hint = true)
    )]
    pub async fn delete_diagram(&self, Parameters(p): Parameters<DiagramParams>) -> ToolResult {
        let id = DiagramId(p.id);
        Ok(match self.with_store(|s| s.delete(&id)) {
            Ok(()) => text(format!("Deleted “{id}”.")),
            Err(e) => store_failure(&e),
        })
    }

    #[tool(
        description = "Applies a batch of edits to a diagram with its type's rules, all or nothing. Returns what each edit did, the ids given to refs, and the diagram afterwards. See the ops field for the edits."
    )]
    pub async fn edit_diagram(&self, Parameters(p): Parameters<EditParams>) -> ToolResult {
        let id = DiagramId(p.id);
        let mut edits: Vec<EditOp> = Vec::new();
        for (i, op) in p.ops.into_iter().enumerate() {
            match serde_json::from_value(op) {
                Ok(e) => edits.push(e),
                Err(e) => {
                    return Ok(failure(format!(
                        "Edit {} isn't one this server knows: {e}",
                        i + 1
                    )));
                }
            }
        }
        let outcome = self.with_store(|store| -> Result<Result<Value, String>, StoreError> {
            let record = store.get(&id)?.ok_or(StoreError::NotFound)?;
            if p.expected_revision.is_some_and(|r| r != record.revision) {
                return Err(StoreError::Conflict);
            }
            let outcome = match ops(record.diagram.meta.diagram_type).apply(&record.diagram, &edits)
            {
                Ok(o) => o,
                Err(e) => return Ok(Err(e.to_string())),
            };
            let mut next = record.diagram.clone();
            next.content = outcome.content;
            let saved = store.save(&next, Some(record.revision))?;
            Ok(Ok(json!({
                "revision": saved.revision,
                "messages": outcome.messages,
                "created": outcome.created,
            })))
        });
        Ok(match outcome {
            Ok(Ok(mut summary)) => {
                if let Ok(described) = self.describe(&id) {
                    summary["diagram"] = described["diagram"].clone();
                    summary["layout"] = described["layout"].clone();
                    summary["status"] = described["status"].clone();
                }
                json_result(&summary)
            }
            Ok(Err(message)) => failure(message),
            Err(e) => store_failure(&e),
        })
    }

    #[tool(
        description = "Records the diagram as a new numbered version, with an optional short description. Does nothing if it already matches its latest version."
    )]
    pub async fn save_version(&self, Parameters(p): Parameters<SaveVersionParams>) -> ToolResult {
        if let Err(m) = hiero_core::history::validate_version_description(
            p.description.as_deref().unwrap_or_default(),
        ) {
            return Ok(failure(m));
        }
        let id = DiagramId(p.id);
        Ok(
            match self.with_store(|s| s.save_version(&id, p.description.as_deref())) {
                Ok(Some(v)) => text(format!("Saved as v{v}.")),
                Ok(None) => text("Already saved: the diagram matches its latest version."),
                Err(e) => store_failure(&e),
            },
        )
    }

    #[tool(
        description = "A diagram's saved versions, newest first: number, when it was saved, its description, and whether it matches the current diagram.",
        annotations(read_only_hint = true)
    )]
    pub async fn list_versions(&self, Parameters(p): Parameters<DiagramParams>) -> ToolResult {
        let id = DiagramId(p.id);
        Ok(match self.with_store(|s| s.list_versions(&id)) {
            Ok(list) => json_result(&Value::Array(
                list.into_iter()
                    .map(|v| json!({"version": v.version, "savedAt": v.saved_at, "description": v.description, "matchesCurrent": v.matches_current}))
                    .collect(),
            )),
            Err(e) => store_failure(&e),
        })
    }

    #[tool(
        description = "Reverts a diagram to a saved version by recording that version's content as a new version; later versions are kept. Refused when the diagram has changes not saved as a version, unless discard_unsaved is true (call save_version first instead).",
        annotations(destructive_hint = true)
    )]
    pub async fn revert_to_version(&self, Parameters(p): Parameters<RevertParams>) -> ToolResult {
        let id = DiagramId(p.id);
        let result = self.with_store(|store| {
            let status = store.status(&id)?;
            if !status.saved && !p.discard_unsaved {
                return Ok(Err(
                    "The diagram has changes that aren't saved as a version. Call save_version first, or pass discard_unsaved: true.".to_owned(),
                ));
            }
            store.revert_to_version(&id, p.version, p.description.as_deref()).map(Ok)
        });
        Ok(match result {
            Ok(Ok((_, v))) => text(format!("Reverted to v{}, recorded as v{v}.", p.version)),
            Ok(Err(m)) => failure(m),
            Err(e) => store_failure(&e),
        })
    }

    #[tool(
        description = "Deletes one saved version. Version numbers aren't reused. This can't be undone.",
        annotations(destructive_hint = true)
    )]
    pub async fn delete_version(&self, Parameters(p): Parameters<VersionParams>) -> ToolResult {
        let id = DiagramId(p.id);
        Ok(
            match self.with_store(|s| s.delete_version(&id, p.version)) {
                Ok(()) => text(format!("Deleted v{}.", p.version)),
                Err(e) => store_failure(&e),
            },
        )
    }

    #[tool(
        description = "Exports a diagram, or a saved version, as zip (the diagram file, to `path`), svg, or drawio (returned as text, and written to `path` if given). Won't replace a file unless overwrite is true.",
        annotations(read_only_hint = true)
    )]
    pub async fn export_diagram(&self, Parameters(p): Parameters<ExportParams>) -> ToolResult {
        let id = DiagramId(p.id);
        let diagram = self.with_store(|s| match p.version {
            Some(v) => s.get_version(&id, v),
            None => s.get(&id).map(|r| r.map(|r| r.diagram)),
        });
        let diagram = match diagram {
            Ok(Some(d)) => d,
            Ok(None) => {
                return Ok(failure(format!(
                    "There's no diagram “{id}” (or that version)."
                )));
            }
            Err(e) => return Ok(store_failure(&e)),
        };
        let t = ops(diagram.meta.diagram_type);
        let (bytes, extension, as_text) = match p.format.as_str() {
            "zip" => (diagram_to_zip(&diagram), "zip", None),
            "svg" => match t.to_svg(&diagram) {
                Ok(s) => (s.clone().into_bytes(), "svg", Some(s)),
                Err(e) => return Ok(failure(e.0)),
            },
            "drawio" => match t.to_drawio(&diagram) {
                Ok(s) => (s.clone().into_bytes(), "drawio", Some(s)),
                Err(e) => return Ok(failure(e.0)),
            },
            _ => {
                return Ok(failure(
                    "The format must be \"zip\", \"svg\", or \"drawio\".",
                ));
            }
        };
        let written = match &p.path {
            Some(path) => match Self::write_file(path, extension, p.overwrite, &bytes) {
                Ok(path) => Some(path),
                Err(m) => return Ok(failure(m)),
            },
            None if extension == "zip" => return Ok(failure("A path is needed to export a zip.")),
            None => None,
        };
        let mut blocks = Vec::new();
        if let Some(path) = written {
            blocks.push(ContentBlock::text(format!("Wrote {}.", path.display())));
        }
        if let Some(s) = as_text {
            blocks.push(ContentBlock::text(s));
        }
        Ok(CallToolResult::success(blocks))
    }

    #[tool(
        description = "Imports a .zip diagram file: it replaces the diagram with the same id (which must be the same type), or is added as a new diagram."
    )]
    pub async fn import_diagram(&self, Parameters(p): Parameters<ImportParams>) -> ToolResult {
        let path = PathBuf::from(&p.path);
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        {
            return Ok(failure("The path must be a .zip file."));
        }
        let diagram = match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| diagram_from_zip(&b).map_err(|e| e.0))
        {
            Ok(d) => d,
            Err(m) => return Ok(failure(m)),
        };
        if ops(diagram.meta.diagram_type)
            .validate(&diagram.content)
            .is_err()
        {
            return Ok(failure("The diagram in the file is incomplete or damaged."));
        }
        Ok(match self.with_store(|s| s.import(&diagram)) {
            Ok(i) => text(format!(
                "{} “{}” ({}).",
                if i.replaced { "Updated" } else { "Imported" },
                i.record.diagram.meta.name,
                i.record.diagram.meta.id
            )),
            Err(e) => store_failure(&e),
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for HieroServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut resources: Vec<Resource> = SPECS
            .iter()
            .map(|(t, _)| {
                Resource::new(
                    format!("arch-hiero://types/{}", t.as_str()),
                    format!("{} diagram spec", t.label()),
                )
                .with_mime_type("text/markdown")
            })
            .collect();
        let list = self.with_store(|s| s.list()).unwrap_or_default();
        for s in list {
            let id = s.meta.id.as_str();
            resources.push(
                Resource::new(format!("arch-hiero://diagrams/{id}"), s.meta.name.clone())
                    .with_description(format!("{} diagram, as JSON", s.meta.diagram_type.label()))
                    .with_mime_type("application/json"),
            );
            resources.push(
                Resource::new(
                    format!("arch-hiero://diagrams/{id}.svg"),
                    format!("{} (SVG)", s.meta.name),
                )
                .with_mime_type("image/svg+xml"),
            );
        }
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let uri = request.uri.clone();
        let not_found = || ErrorData::invalid_params(format!("There's no resource {uri}."), None);
        if let Some(t) = uri.strip_prefix("arch-hiero://types/") {
            let spec = SPECS
                .iter()
                .find(|(id, _)| id.as_str() == t)
                .ok_or_else(not_found)?;
            return Ok(
                ReadResourceResult::new(vec![ResourceContents::text(spec.1, uri.clone())]).into(),
            );
        }
        let rest = uri
            .strip_prefix("arch-hiero://diagrams/")
            .ok_or_else(not_found)?;
        let parts: Vec<&str> = rest.split('/').collect();
        let body = match parts.as_slice() {
            [id] if id.to_ascii_lowercase().ends_with(".svg") => {
                let id = DiagramId(id.trim_end_matches(".svg").to_owned());
                let diagram = self
                    .with_store(|s| s.get(&id))
                    .ok()
                    .flatten()
                    .ok_or_else(not_found)?;
                ops(diagram.diagram.meta.diagram_type)
                    .to_svg(&diagram.diagram)
                    .map_err(|e| ErrorData::internal_error(e.0, None))?
            }
            [id] => {
                let value = self
                    .describe(&DiagramId((*id).to_owned()))
                    .map_err(|_| not_found())?;
                serde_json::to_string_pretty(&value).unwrap_or_default()
            }
            [id, "versions", n] => {
                let version: i64 = n.parse().map_err(|_| not_found())?;
                let d = self
                    .with_store(|s| s.get_version(&DiagramId((*id).to_owned()), version))
                    .ok()
                    .flatten()
                    .ok_or_else(not_found)?;
                serde_json::to_string_pretty(&diagram_to_json(&d)).unwrap_or_default()
            }
            _ => return Err(not_found()),
        };
        Ok(ReadResourceResult::new(vec![ResourceContents::text(body, uri.clone())]).into())
    }
}
