//! Edits agents make through the MCP server, applied with each diagram type's own rules, so an
//! agent can't produce a layout the editor wouldn't.

use std::collections::HashMap;

use hiero_core::engine::{Corner, DiagramType, Engine};
use hiero_core::geometry::Point;
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::stored::{StoredContent, StoredDiagram};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::engine_for;

/// One edit. Ids can name elements added earlier in the same batch by their `ref`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    /// Adds an element. With `parent_id` it goes in that container; with `x` and `y` its
    /// top-left corner goes there (inside whatever container is there); with neither it goes
    /// at a free spot, as a palette click does.
    AddElement {
        #[serde(default, rename = "ref")]
        reference: Option<String>,
        kind: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        parent_id: Option<String>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        props: Option<Map<String, Value>>,
    },
    /// Changes an element's name, description, properties (merged), or container. A
    /// `parent_id` of "" takes it out of its container.
    UpdateElement {
        id: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        parent_id: Option<String>,
        #[serde(default)]
        props: Option<Map<String, Value>>,
    },
    /// Moves an element's top-left corner to `x`, `y`, as dragging it there would.
    MoveElement {
        id: String,
        x: f64,
        y: f64,
    },
    /// Resizes a resizable element, keeping its top-left corner.
    ResizeElement {
        id: String,
        width: f64,
        height: f64,
    },
    RemoveElement {
        id: String,
    },
    /// Adds an arrow, with its type's usual properties, then any given.
    AddConnection {
        #[serde(default, rename = "ref")]
        reference: Option<String>,
        source_id: String,
        target_id: String,
        #[serde(default)]
        props: Option<Map<String, Value>>,
    },
    /// Changes an arrow's properties (merged).
    UpdateConnection {
        id: String,
        props: Map<String, Value>,
    },
    RemoveConnection {
        id: String,
    },
}

/// Why a batch of edits was rejected: which edit, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditError {
    pub index: usize,
    pub message: String,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Edit {} was rejected: {}", self.index + 1, self.message)
    }
}

impl std::error::Error for EditError {}

/// What a batch of edits did.
#[derive(Debug, Clone, PartialEq)]
pub struct EditOutcome {
    pub content: StoredContent,
    /// The ids given to elements and arrows added with a `ref`.
    pub created: HashMap<String, String>,
    /// What each edit did, in words.
    pub messages: Vec<String>,
}

fn merge_props<P: Serialize + serde::de::DeserializeOwned>(
    props: &P,
    changes: &Map<String, Value>,
) -> Result<P, String> {
    let mut value = serde_json::to_value(props).map_err(|e| e.to_string())?;
    if let Some(map) = value.as_object_mut() {
        for (k, v) in changes {
            if v.is_null() {
                map.remove(k);
            } else {
                map.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::from_value(value).map_err(|e| format!("the properties aren't valid: {e}"))
}

struct Batch<'a, T: DiagramType> {
    eng: Engine<T>,
    refs: &'a mut HashMap<String, String>,
}

impl<T: DiagramType> Batch<'_, T> {
    fn element_id(&self, text: &str) -> Result<ElementId, String> {
        let id = ElementId(
            self.refs
                .get(text)
                .cloned()
                .unwrap_or_else(|| text.to_owned()),
        );
        if self.eng.element(&id).is_none() {
            return Err(format!("there's no element “{text}”"));
        }
        Ok(id)
    }

    fn connection_id(&self, text: &str) -> Result<ConnectionId, String> {
        let id = ConnectionId(
            self.refs
                .get(text)
                .cloned()
                .unwrap_or_else(|| text.to_owned()),
        );
        if self.eng.connection(&id).is_none() {
            return Err(format!("there's no arrow “{text}”"));
        }
        Ok(id)
    }

    fn kind(text: &str) -> Result<T::Kind, String> {
        serde_json::from_value(Value::String(text.to_owned())).map_err(|_| {
            let kinds: Vec<String> = T::palette()
                .iter()
                .filter_map(|k| {
                    serde_json::to_value(k.kind)
                        .ok()?
                        .as_str()
                        .map(str::to_owned)
                })
                .collect();
            format!(
                "“{text}” isn't a kind of element here; use one of {}",
                kinds.join(", ")
            )
        })
    }

    fn apply(&mut self, op: &EditOp) -> Result<String, String> {
        match op {
            EditOp::AddElement {
                reference,
                kind,
                label,
                description,
                parent_id,
                x,
                y,
                props,
            } => {
                let kind = Self::kind(kind)?;
                let parent = parent_id
                    .as_deref()
                    .map(|p| self.element_id(p))
                    .transpose()?;
                if let Some(parent) = &parent {
                    let holder = self.eng.element(parent).map(|e| e.kind);
                    let can_hold = holder.is_some_and(|h| T::info(h).can_contain.contains(&kind));
                    if !can_hold {
                        return Err(format!(
                            "a {} can't go inside “{}”",
                            T::info(kind).label.to_lowercase(),
                            self.eng.label_of(parent).unwrap_or_default()
                        ));
                    }
                }
                let before: Vec<ElementId> =
                    self.eng.elements().iter().map(|e| e.id.clone()).collect();
                match (parent, x.zip(*y)) {
                    (Some(parent), None) => {
                        self.eng.select(&parent);
                        self.eng.add_at_free_position(kind);
                    }
                    (_, Some((x, y))) => {
                        let info = T::info(kind);
                        let size = T::new_size_of(&self.eng, info);
                        self.eng.palette_drop(
                            kind,
                            Point::new(x + size.width / 2.0, y + size.height / 2.0),
                        );
                    }
                    (None, None) => {
                        self.eng.clear_selection();
                        self.eng.add_at_free_position(kind);
                    }
                }
                let id = self
                    .eng
                    .selected_id()
                    .cloned()
                    .filter(|id| !before.contains(id))
                    .ok_or("the element couldn't be added")?;
                if let Some(r) = reference {
                    self.refs.insert(r.clone(), id.0.clone());
                }
                if label.is_some() || description.is_some() || props.is_some() {
                    self.update(
                        &id,
                        label.clone(),
                        description.clone(),
                        None,
                        props.as_ref(),
                    )?;
                }
                Ok(format!(
                    "Added {} “{}” ({id}).",
                    T::info(kind).label.to_lowercase(),
                    self.eng.label_of(&id).unwrap_or_default()
                ))
            }
            EditOp::UpdateElement {
                id,
                label,
                description,
                parent_id,
                props,
            } => {
                let id = self.element_id(id)?;
                let parent = match parent_id.as_deref() {
                    None => None,
                    Some("") => Some(None),
                    Some(p) => Some(Some(self.element_id(p)?)),
                };
                self.update(
                    &id,
                    label.clone(),
                    description.clone(),
                    parent,
                    props.as_ref(),
                )?;
                Ok(format!(
                    "Updated “{}”.",
                    self.eng.label_of(&id).unwrap_or_default()
                ))
            }
            EditOp::MoveElement { id, x, y } => {
                let id = self.element_id(id)?;
                let laid = self
                    .eng
                    .laid(&id)
                    .cloned()
                    .ok_or("the element isn't laid out")?;
                let placement = self.eng.move_placement(&laid, Point::new(*x, *y));
                self.eng.commit_move(&id, placement);
                Ok(self.eng.announcement.clone())
            }
            EditOp::ResizeElement { id, width, height } => {
                let id = self.element_id(id)?;
                let laid = self
                    .eng
                    .laid(&id)
                    .cloned()
                    .ok_or("the element isn't laid out")?;
                if !laid.is_resizable {
                    return Err(format!(
                        "a {} can't be resized",
                        laid.info.label.to_lowercase()
                    ));
                }
                let delta = Point::new(width - laid.rect.width, height - laid.rect.height);
                let rect = self.eng.resize_bounds(&laid, Corner::BottomRight, delta);
                self.eng.commit_resize(&id, rect);
                Ok(format!("Resized “{}”.", laid.el.label))
            }
            EditOp::RemoveElement { id } => {
                let id = self.element_id(id)?;
                T::remove(&mut self.eng, &id);
                Ok(self.eng.announcement.clone())
            }
            EditOp::AddConnection {
                reference,
                source_id,
                target_id,
                props,
            } => {
                if !T::ID.has_arrows() {
                    return Err(format!("{} diagrams have no arrows", T::ID.label()));
                }
                let source = self.element_id(source_id)?;
                let target = self.element_id(target_id)?;
                if !self.eng.can_connect(&source, &target) {
                    return Err(format!(
                        "an arrow can't go from “{}” to “{}”",
                        self.eng.label_of(&source).unwrap_or_default(),
                        self.eng.label_of(&target).unwrap_or_default()
                    ));
                }
                let id = self
                    .eng
                    .add_connection(&source, &target)
                    .ok_or("there's already an arrow between them")?;
                if let Some(r) = reference {
                    self.refs.insert(r.clone(), id.0.clone());
                }
                if let Some(props) = props {
                    self.update_connection(&id, props)?;
                }
                // Described after the props are applied, so it names the arrow's own protocol.
                let message = self
                    .eng
                    .connection(&id)
                    .map_or_else(String::new, |c| T::arrow_added(&self.eng, c));
                Ok(format!("{message} ({id})"))
            }
            EditOp::UpdateConnection { id, props } => {
                let id = self.connection_id(id)?;
                self.update_connection(&id, props)?;
                Ok("Arrow updated.".into())
            }
            EditOp::RemoveConnection { id } => {
                let id = self.connection_id(id)?;
                self.eng.remove_connection(&id);
                Ok(self.eng.announcement.clone())
            }
        }
    }

    fn update(
        &mut self,
        id: &ElementId,
        label: Option<String>,
        description: Option<String>,
        parent: Option<Option<ElementId>>,
        props: Option<&Map<String, Value>>,
    ) -> Result<(), String> {
        let current = self
            .eng
            .element(id)
            .cloned()
            .ok_or("there's no such element")?;
        let mut with = current.clone();
        if let Some(label) = label {
            with.label = label;
        }
        if let Some(description) = description {
            if !T::info(current.kind).has_description {
                return Err(format!(
                    "a {} has no description",
                    T::info(current.kind).label.to_lowercase()
                ));
            }
            with.description = Some(description);
        }
        if let Some(props) = props {
            with.props = merge_props(&current.props, props)?;
        }
        T::validate_element(&with)?;
        if let Some(Some(parent)) = &parent {
            let holder = self.eng.element(parent).map(|e| e.kind);
            if !holder.is_some_and(|h| T::info(h).can_contain.contains(&current.kind)) {
                return Err(format!(
                    "a {} can't go inside “{}”",
                    T::info(current.kind).label.to_lowercase(),
                    self.eng.label_of(parent).unwrap_or_default()
                ));
            }
        }
        self.eng.select(id);
        let parent = parent.unwrap_or_else(|| current.parent_id.clone());
        T::apply_element(&mut self.eng, id, with, parent);
        Ok(())
    }

    fn update_connection(
        &mut self,
        id: &ConnectionId,
        props: &Map<String, Value>,
    ) -> Result<(), String> {
        let current = self
            .eng
            .connection(id)
            .cloned()
            .ok_or("there's no such arrow")?;
        let merged = merge_props(&current.props, props)?;
        T::validate_arrow(&merged)?;
        self.eng.set_connection_props(id, merged);
        Ok(())
    }
}

/// Applies `ops` to `diagram` in order. If any fails, nothing is applied.
pub fn apply<T: DiagramType>(
    diagram: &StoredDiagram,
    ops: &[EditOp],
) -> Result<EditOutcome, EditError> {
    let eng = engine_for::<T>(&diagram.content, &diagram.colors).map_err(|e| EditError {
        index: 0,
        message: e.0,
    })?;
    let mut refs = HashMap::new();
    let mut batch = Batch {
        eng,
        refs: &mut refs,
    };
    let mut messages = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        let message = batch
            .apply(op)
            .map_err(|message| EditError { index, message })?;
        messages.push(message);
    }
    let content = StoredContent::from_typed(batch.eng.elements(), batch.eng.connections());
    Ok(EditOutcome {
        content,
        created: refs,
        messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{new_diagram, ops};
    use hiero_core::model::DiagramTypeId;
    use serde_json::json;

    fn parse(value: Value) -> Vec<EditOp> {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn agents_build_a_physical_diagram_with_the_rules() {
        let diagram = new_diagram(DiagramTypeId::Physical, "Infra", None);
        let zone = diagram.content.elements[2].id.0.clone();
        let edits = parse(json!([
            {"op": "add_element", "ref": "web", "kind": "server", "label": "Web", "parent_id": zone,
             "props": {"serverType": "container", "instanceCount": 3}},
            {"op": "add_element", "ref": "db", "kind": "database", "label": "Orders", "parent_id": zone,
             "props": {"databaseType": "postgresql"}},
            {"op": "add_connection", "source_id": "web", "target_id": "db"}
        ]));
        let outcome = ops(DiagramTypeId::Physical)
            .apply(&diagram, &edits)
            .unwrap();
        assert_eq!(outcome.content.elements.len(), 7);
        let arrow = &outcome.content.connections[0];
        assert_eq!(arrow.props["protocol"], json!("TCP"));
        assert_eq!(arrow.props["port"], json!("5432"));
        let web = outcome
            .content
            .elements
            .iter()
            .find(|e| e.label == "Web")
            .unwrap();
        assert_eq!(
            web.parent_id.as_ref().map(|p| p.0.as_str()),
            Some(zone.as_str())
        );
        assert_eq!(web.props["instanceCount"], json!(3));
    }

    #[test]
    fn added_arrows_are_described_with_their_given_protocol() {
        let diagram = new_diagram(DiagramTypeId::Physical, "Infra", None);
        let zone = diagram.content.elements[2].id.0.clone();
        let edits = parse(json!([
            {"op": "add_element", "ref": "proxy", "kind": "server", "label": "Proxy", "parent_id": zone},
            {"op": "add_element", "ref": "api", "kind": "server", "label": "API", "parent_id": zone},
            {"op": "add_connection", "source_id": "proxy", "target_id": "api",
             "props": {"protocol": "HTTP", "port": "8080"}}
        ]));
        let outcome = ops(DiagramTypeId::Physical)
            .apply(&diagram, &edits)
            .unwrap();
        let arrow = &outcome.content.connections[0];
        assert!(
            outcome.messages[2].starts_with("Arrow from Proxy to API added, over HTTP:8080. ("),
            "{}",
            outcome.messages[2]
        );
        assert_eq!(arrow.props["port"], json!("8080"));
    }

    #[test]
    fn invalid_edits_reject_the_whole_batch_with_a_reason() {
        let diagram = new_diagram(DiagramTypeId::Physical, "Infra", None);
        let zone = diagram.content.elements[2].id.0.clone();
        let edits = parse(json!([
            {"op": "add_element", "kind": "server", "parent_id": zone},
            {"op": "add_element", "kind": "firewall", "parent_id": zone}
        ]));
        let error = ops(DiagramTypeId::Physical)
            .apply(&diagram, &edits)
            .unwrap_err();
        assert_eq!(error.index, 1);
        assert_eq!(error.message, "a firewall can't go inside “Perimeter”");
        let bad_kind = parse(json!([{"op": "add_element", "kind": "spaceship"}]));
        let error = ops(DiagramTypeId::Physical)
            .apply(&diagram, &bad_kind)
            .unwrap_err();
        assert!(
            error
                .message
                .starts_with("“spaceship” isn't a kind of element here")
        );
    }

    #[test]
    fn conceptual_arrows_respect_connects_to() {
        let diagram = new_diagram(DiagramTypeId::Conceptual, "C", None);
        let edits = parse(json!([
            {"op": "add_element", "ref": "a", "kind": "actor", "label": "Customer"},
            {"op": "add_element", "ref": "p", "kind": "process", "label": "Billing"},
            {"op": "add_connection", "source_id": "a", "target_id": "p"},
            {"op": "add_connection", "source_id": "p", "target_id": "a"}
        ]));
        let error = ops(DiagramTypeId::Conceptual)
            .apply(&diagram, &edits)
            .unwrap_err();
        assert_eq!(
            error.message,
            "an arrow can't go from “Billing” to “Customer”"
        );
    }

    #[test]
    fn deployed_components_land_in_layers() {
        let diagram = new_diagram(DiagramTypeId::DeployedComponent, "D", None);
        let edits = parse(json!([
            {"op": "add_element", "ref": "web", "kind": "layer", "label": "Web"},
            {"op": "add_element", "kind": "component", "label": "Portal", "parent_id": "web",
             "description": "Serves pages\nCaches", "props": {"protocols": ["HTTPS"], "techStack": ["aws"]}},
            {"op": "add_connection", "source_id": "web", "target_id": "web"}
        ]));
        let error = ops(DiagramTypeId::DeployedComponent)
            .apply(&diagram, &edits)
            .unwrap_err();
        assert_eq!(error.message, "Deployed Component diagrams have no arrows");
        let outcome = ops(DiagramTypeId::DeployedComponent)
            .apply(&diagram, &edits[..2])
            .unwrap();
        let portal = outcome
            .content
            .elements
            .iter()
            .find(|e| e.label == "Portal")
            .unwrap();
        assert_eq!(
            portal.parent_id.as_ref().map(|p| p.0.clone()),
            Some(outcome.created["web"].clone())
        );
    }

    #[test]
    fn unknown_technologies_are_refused() {
        let diagram = new_diagram(DiagramTypeId::DeployedComponent, "D", None);
        let edits = parse(json!([
            {"op": "add_element", "ref": "web", "kind": "layer", "label": "Web"},
            {"op": "add_element", "kind": "component", "label": "Portal", "parent_id": "web",
             "props": {"techStack": ["springboot", "spring-boot"]}}
        ]));
        let error = ops(DiagramTypeId::DeployedComponent)
            .apply(&diagram, &edits)
            .unwrap_err();
        assert_eq!(error.index, 1);
        assert_eq!(error.message, "“spring-boot” isn't a known technology.");
        let tech = &ops(DiagramTypeId::DeployedComponent).prop_values()["techStack"];
        assert!(
            tech.iter()
                .any(|t| t.id == "springboot" && t.group == "Frameworks")
        );
        assert!(ops(DiagramTypeId::Conceptual).prop_values().is_empty());
    }
}
