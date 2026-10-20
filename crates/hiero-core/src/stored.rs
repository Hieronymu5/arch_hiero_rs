//! Diagrams in a form that doesn't know about diagram types: the common columns of each element
//! and arrow, plus a JSON object of type-specific properties. This is what the store keeps and
//! what files and versions carry. Typed content converts to and from it through serde.

use std::collections::BTreeMap;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::model::{Connection, ConnectionId, DiagramMeta, Element, ElementId};

/// Keys that are columns of their own rather than properties.
const ELEMENT_KEYS: [&str; 9] = [
    "id",
    "kind",
    "label",
    "description",
    "parentId",
    "x",
    "y",
    "width",
    "height",
];
const CONNECTION_KEYS: [&str; 3] = ["id", "sourceId", "targetId"];

/// An element as stored: its common fields, and everything else in `props`.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredElement {
    pub id: ElementId,
    pub kind: String,
    pub label: String,
    pub description: Option<String>,
    pub parent_id: Option<ElementId>,
    pub x: f64,
    pub y: f64,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub props: Map<String, Value>,
}

/// An arrow as stored: its ends, and everything else in `props`.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredConnection {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub props: Map<String, Value>,
}

/// A diagram's elements and arrows, in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoredContent {
    pub elements: Vec<StoredElement>,
    pub connections: Vec<StoredConnection>,
}

/// A whole diagram as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredDiagram {
    pub meta: DiagramMeta,
    /// Colours that differ from the type's defaults.
    pub colors: BTreeMap<String, String>,
    pub content: StoredContent,
}

/// Why JSON couldn't be read as a diagram's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError(pub String);

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ContentError {}

fn text(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

fn json_number(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() < 9e15 {
        Value::from(value as i64)
    } else {
        Value::from(value)
    }
}

impl StoredElement {
    /// Reads an element from its flat JSON object, as in a diagram file.
    pub fn from_json(value: &Value) -> Result<Self, ContentError> {
        let bad = || ContentError("An element is incomplete or damaged.".into());
        let object = value.as_object().ok_or_else(bad)?;
        let get = |key: &str| object.get(key);
        let props = object
            .iter()
            .filter(|(key, _)| !ELEMENT_KEYS.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(Self {
            id: ElementId(get("id").and_then(text).ok_or_else(bad)?),
            kind: get("kind").and_then(text).ok_or_else(bad)?,
            label: get("label")
                .and_then(Value::as_str)
                .ok_or_else(bad)?
                .to_owned(),
            description: get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            parent_id: get("parentId").and_then(text).map(ElementId),
            x: number(get("x")).ok_or_else(bad)?,
            y: number(get("y")).ok_or_else(bad)?,
            width: number(get("width")),
            height: number(get("height")),
            props,
        })
    }

    /// The flat JSON object, as in a diagram file.
    pub fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("id".into(), Value::from(self.id.0.clone()));
        object.insert("kind".into(), Value::from(self.kind.clone()));
        object.insert("label".into(), Value::from(self.label.clone()));
        if let Some(description) = &self.description {
            object.insert("description".into(), Value::from(description.clone()));
        }
        if let Some(parent) = &self.parent_id {
            object.insert("parentId".into(), Value::from(parent.0.clone()));
        }
        for (key, value) in &self.props {
            object.insert(key.clone(), value.clone());
        }
        if let Some(width) = self.width {
            object.insert("width".into(), json_number(width));
        }
        if let Some(height) = self.height {
            object.insert("height".into(), json_number(height));
        }
        object.insert("x".into(), json_number(self.x));
        object.insert("y".into(), json_number(self.y));
        Value::Object(object)
    }

    /// The element as a diagram type's typed element.
    pub fn to_typed<K: DeserializeOwned, P: DeserializeOwned>(
        &self,
    ) -> Result<Element<K, P>, ContentError> {
        serde_json::from_value(self.to_json()).map_err(|error| {
            ContentError(format!(
                "The element “{}” ({}) can't be read: {error}.",
                self.label, self.kind
            ))
        })
    }

    pub fn from_typed<K: Serialize, P: Serialize>(element: &Element<K, P>) -> Self {
        let value = serde_json::to_value(element).expect("elements serialise");
        Self::from_json(&value).expect("serialised elements read back")
    }
}

impl StoredConnection {
    pub fn from_json(value: &Value) -> Result<Self, ContentError> {
        let bad = || ContentError("An arrow is incomplete or damaged.".into());
        let object = value.as_object().ok_or_else(bad)?;
        let get = |key: &str| object.get(key);
        let props = object
            .iter()
            .filter(|(key, _)| !CONNECTION_KEYS.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(Self {
            id: ConnectionId(get("id").and_then(text).ok_or_else(bad)?),
            source_id: ElementId(get("sourceId").and_then(text).ok_or_else(bad)?),
            target_id: ElementId(get("targetId").and_then(text).ok_or_else(bad)?),
            props,
        })
    }

    pub fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("id".into(), Value::from(self.id.0.clone()));
        object.insert("sourceId".into(), Value::from(self.source_id.0.clone()));
        object.insert("targetId".into(), Value::from(self.target_id.0.clone()));
        for (key, value) in &self.props {
            object.insert(key.clone(), value.clone());
        }
        Value::Object(object)
    }

    pub fn to_typed<A: DeserializeOwned>(&self) -> Result<Connection<A>, ContentError> {
        serde_json::from_value(self.to_json())
            .map_err(|error| ContentError(format!("An arrow can't be read: {error}.")))
    }

    pub fn from_typed<A: Serialize>(connection: &Connection<A>) -> Self {
        let value = serde_json::to_value(connection).expect("arrows serialise");
        Self::from_json(&value).expect("serialised arrows read back")
    }
}

impl StoredContent {
    /// The content as typed elements and arrows.
    #[expect(clippy::type_complexity, reason = "a pair of typed lists")]
    pub fn to_typed<K, P, A>(
        &self,
    ) -> Result<(Vec<Element<K, P>>, Vec<Connection<A>>), ContentError>
    where
        K: DeserializeOwned,
        P: DeserializeOwned,
        A: DeserializeOwned,
    {
        let elements = self
            .elements
            .iter()
            .map(StoredElement::to_typed)
            .collect::<Result<_, _>>()?;
        let connections = self
            .connections
            .iter()
            .map(StoredConnection::to_typed)
            .collect::<Result<_, _>>()?;
        Ok((elements, connections))
    }

    pub fn from_typed<K: Serialize, P: Serialize, A: Serialize>(
        elements: &[Element<K, P>],
        connections: &[Connection<A>],
    ) -> Self {
        Self {
            elements: elements.iter().map(StoredElement::from_typed).collect(),
            connections: connections
                .iter()
                .map(StoredConnection::from_typed)
                .collect(),
        }
    }
}

/// `value` as JSON with object keys sorted and numbers in their shortest form, so equal
/// content always gives the same text, whatever order maps were built in.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => out.push_str(&crate::geometry::fmt_num(f)),
            _ => out.push_str(&n.to_string()),
        },
        Value::String(s) => out.push_str(&serde_json::to_string(s).expect("strings serialise")),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("keys serialise"));
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn elements_split_common_fields_from_props() {
        let json = json!({
            "id": "e1", "kind": "server", "label": "Web", "parentId": "z1",
            "serverType": "container", "instanceCount": 3, "x": 40, "y": 60
        });
        let element = StoredElement::from_json(&json).unwrap();
        assert_eq!(element.kind, "server");
        assert_eq!(element.parent_id, Some(ElementId::from("z1")));
        assert_eq!(element.props.len(), 2);
        assert_eq!(element.to_json(), json);
    }

    #[test]
    fn incomplete_elements_are_rejected() {
        assert!(
            StoredElement::from_json(&json!({"id": "", "kind": "a", "label": "", "x": 0, "y": 0}))
                .is_err()
        );
        assert!(
            StoredElement::from_json(&json!({"id": "a", "kind": "a", "label": "", "x": 0}))
                .is_err()
        );
        assert!(StoredConnection::from_json(&json!({"id": "c", "sourceId": "a"})).is_err());
    }

    #[test]
    fn canonical_json_sorts_keys() {
        let a = json!({"b": 1, "a": [1.5, {"d": true, "c": null}]});
        assert_eq!(
            canonical_json(&a),
            r#"{"a":[1.5,{"c":null,"d":true}],"b":1}"#
        );
        assert_eq!(canonical_json(&json!(40.0)), "40");
    }
}
