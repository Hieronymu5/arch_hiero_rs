//! A diagram as a file to export and import again, on this machine or another: a zip holding
//! one JSON file with every field of the diagram. The same format as the prototype's, so its
//! files import unchanged. A port of `utils/diagram-file.ts`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::model::{DiagramId, DiagramMeta, DiagramTypeId};
use crate::stored::{StoredConnection, StoredContent, StoredDiagram, StoredElement};

/// What the JSON file says it is, so a file from something else isn't taken for a diagram.
pub const DIAGRAM_FILE_FORMAT: &str = "arch-hiero-diagram";
/// The version of the file's layout, raised if it changes in a way older readers can't follow.
pub const DIAGRAM_FILE_VERSION: u64 = 1;
/// The JSON file's name in the zip.
pub const DIAGRAM_FILE_ENTRY: &str = "diagram.json";

/// A file that can't be imported as a diagram, with a message saying why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramFileError(pub String);

impl std::fmt::Display for DiagramFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DiagramFileError {}

fn err(message: &str) -> DiagramFileError {
    DiagramFileError(message.to_owned())
}

/// The key a type's colours are kept under in the file: physical diagrams use their own.
fn colors_key(diagram_type: DiagramTypeId) -> &'static str {
    match diagram_type {
        DiagramTypeId::Physical => "physicalColors",
        _ => "colors",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileDiagram<'a> {
    id: &'a DiagramId,
    name: &'a str,
    #[serde(rename = "type")]
    diagram_type: DiagramTypeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    colors: Option<&'a BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    physical_colors: Option<&'a BTreeMap<String, String>>,
    elements: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connections: Option<Vec<Value>>,
    created_at: &'a str,
    updated_at: &'a str,
}

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'static str,
    version: u64,
    diagram: FileDiagram<'a>,
}

/// The diagram as the JSON object a file's `diagram` key holds.
pub fn diagram_to_json(diagram: &StoredDiagram) -> Value {
    serde_json::to_value(file_diagram(diagram)).expect("diagrams serialise")
}

fn file_diagram(diagram: &StoredDiagram) -> FileDiagram<'_> {
    let meta = &diagram.meta;
    let colors = (!diagram.colors.is_empty()).then_some(&diagram.colors);
    let physical = meta.diagram_type == DiagramTypeId::Physical;
    FileDiagram {
        id: &meta.id,
        name: &meta.name,
        diagram_type: meta.diagram_type,
        description: meta.description.as_deref(),
        colors: if physical { None } else { colors },
        physical_colors: if physical { colors } else { None },
        elements: diagram
            .content
            .elements
            .iter()
            .map(StoredElement::to_json)
            .collect(),
        // A type without arrows keeps having no list of them.
        connections: meta.diagram_type.has_arrows().then(|| {
            diagram
                .content
                .connections
                .iter()
                .map(StoredConnection::to_json)
                .collect()
        }),
        created_at: &meta.created_at,
        updated_at: &meta.updated_at,
    }
}

/// The diagram file's JSON text: `{format, version, diagram}`, indented by two spaces.
pub fn diagram_file_json(diagram: &StoredDiagram) -> String {
    let file = FileOut {
        format: DIAGRAM_FILE_FORMAT,
        version: DIAGRAM_FILE_VERSION,
        diagram: file_diagram(diagram),
    };
    serde_json::to_string_pretty(&file).expect("diagram files serialise")
}

/// `diagram` as a zipped JSON file.
pub fn diagram_to_zip(diagram: &StoredDiagram) -> Vec<u8> {
    let json = diagram_file_json(diagram);
    let mut out = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let mut options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        if let Some(time) = local_zip_time() {
            options = options.last_modified_time(time);
        }
        zip.start_file(DIAGRAM_FILE_ENTRY, options)
            .expect("zip entries start");
        zip.write_all(json.as_bytes()).expect("zip entries write");
        zip.finish().expect("zips finish");
    }
    out.into_inner()
}

fn local_zip_time() -> Option<zip::DateTime> {
    let now = jiff::Zoned::now();
    zip::DateTime::from_date_and_time(
        u16::try_from(now.year()).ok()?.max(1980),
        u8::try_from(now.month()).ok()?,
        u8::try_from(now.day()).ok()?,
        u8::try_from(now.hour()).ok()?,
        u8::try_from(now.minute()).ok()?,
        u8::try_from(now.second()).ok()?,
    )
    .ok()
}

/// The diagram in a zip made by `diagram_to_zip`, or by the prototype.
pub fn diagram_from_zip(bytes: &[u8]) -> Result<StoredDiagram, DiagramFileError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| err("The file isn’t a zip file."))?;
    let names: Vec<String> = archive
        .file_names()
        .map(|name| name.map(std::borrow::Cow::into_owned))
        .collect::<Result<_, _>>()
        .map_err(|_| err("The zip file is damaged."))?;
    // Its own entry, or any one JSON file, in case it was zipped again by hand.
    let name = names
        .iter()
        .find(|n| n.as_str() == DIAGRAM_FILE_ENTRY)
        .or_else(|| names.iter().find(|n| n.to_lowercase().ends_with(".json")))
        .ok_or_else(|| err("The zip file has no diagram in it."))?
        .clone();
    let mut entry = archive.by_name(&name).map_err(|error| match error {
        zip::result::ZipError::UnsupportedArchive(message) if message.contains("assword") => {
            err("The zip file is encrypted.")
        }
        zip::result::ZipError::UnsupportedArchive(_) => {
            err("The zip file uses a compression method that isn’t supported.")
        }
        _ => err("The zip file is damaged."),
    })?;
    if entry.encrypted() {
        return Err(err("The zip file is encrypted."));
    }
    let mut data = Vec::new();
    entry
        .read_to_end(&mut data)
        .map_err(|_| err("The zip file is damaged."))?;
    let parsed: Value = serde_json::from_slice(&data)
        .map_err(|_| err("The diagram in the zip file isn’t valid JSON."))?;
    diagram_from_file_json(&parsed)
}

/// The diagram in a parsed file, checked to be one this app can open.
pub fn diagram_from_file_json(file: &Value) -> Result<StoredDiagram, DiagramFileError> {
    let object = file
        .as_object()
        .filter(|o| o.get("format").and_then(Value::as_str) == Some(DIAGRAM_FILE_FORMAT))
        .ok_or_else(|| err("The file isn’t an ArchHiero diagram."))?;
    match object.get("version").and_then(Value::as_f64) {
        Some(version) if version <= DIAGRAM_FILE_VERSION as f64 => {}
        _ => {
            return Err(err(
                "The diagram was saved by a newer version of ArchHiero. Update it and try again.",
            ));
        }
    }
    let diagram = object
        .get("diagram")
        .ok_or_else(|| err("The diagram in the file is incomplete or damaged."))?;
    diagram_from_json(diagram)
}

/// A diagram from the JSON object a file's `diagram` key holds, as a version snapshot is.
pub fn diagram_from_json(diagram: &Value) -> Result<StoredDiagram, DiagramFileError> {
    let damaged = || err("The diagram in the file is incomplete or damaged.");
    let d = diagram.as_object().ok_or_else(damaged)?;
    let id = d
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(damaged)?;
    let name = d.get("name").and_then(Value::as_str).ok_or_else(damaged)?;
    let diagram_type = d
        .get("type")
        .and_then(Value::as_str)
        .and_then(DiagramTypeId::parse)
        .ok_or_else(damaged)?;
    let elements = match d.get("elements") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(StoredElement::from_json)
            .collect::<Result<_, _>>()
            .map_err(|_| damaged())?,
        Some(_) => return Err(damaged()),
    };
    let connections = match d.get("connections") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(StoredConnection::from_json)
            .collect::<Result<_, _>>()
            .map_err(|_| damaged())?,
        Some(_) => return Err(damaged()),
    };
    let colors = d
        .get(colors_key(diagram_type))
        .and_then(Value::as_object)
        .map(colors_from)
        .unwrap_or_default();
    let text = |key: &str| d.get(key).and_then(Value::as_str).map(str::to_owned);
    let now = crate::model::now_rfc3339();
    Ok(StoredDiagram {
        meta: DiagramMeta {
            id: DiagramId(id.to_owned()),
            diagram_type,
            name: name.to_owned(),
            description: text("description").filter(|s| !s.is_empty()),
            created_at: text("createdAt").unwrap_or_else(|| now.clone()),
            updated_at: text("updatedAt").unwrap_or(now),
        },
        colors,
        content: StoredContent {
            elements,
            connections,
        },
    })
}

fn colors_from(map: &Map<String, Value>) -> BTreeMap<String, String> {
    map.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

/// A file name for `name`: characters not allowed in file names on common systems become
/// dashes, and an empty name becomes "Diagram".
pub fn file_name(name: &str, extension: &str) -> String {
    let mut cleaned = String::new();
    let mut in_bad = false;
    for c in name.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            if !in_bad {
                cleaned.push('-');
            }
            in_bad = true;
        } else {
            cleaned.push(c);
            in_bad = false;
        }
    }
    let trimmed = cleaned.trim();
    let base = if trimmed.is_empty() {
        "Diagram"
    } else {
        trimmed
    };
    format!("{base}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> StoredDiagram {
        let file = json!({
            "format": "arch-hiero-diagram",
            "version": 1,
            "diagram": {
                "id": "d1", "name": "Infra", "type": "physical",
                "physicalColors": {"servers": "#00ff00"},
                "elements": [
                    {"id": "n", "kind": "network", "label": "Intranet", "x": 40, "y": 300},
                    {"id": "s", "kind": "server", "label": "Web", "parentId": "n",
                     "serverType": "container", "x": 60, "y": 360}
                ],
                "connections": [{"id": "c", "sourceId": "s", "targetId": "n",
                                 "protocol": "HTTPS", "port": "443"}],
                "createdAt": "2026-10-01T09:00:00.000Z", "createdBy": "hiero",
                "updatedAt": "2026-10-01T09:00:00.000Z", "updatedBy": "hiero"
            }
        });
        diagram_from_file_json(&file).unwrap()
    }

    #[test]
    fn reads_prototype_files() {
        let d = sample();
        assert_eq!(d.meta.diagram_type, DiagramTypeId::Physical);
        assert_eq!(d.colors.get("servers").map(String::as_str), Some("#00ff00"));
        assert_eq!(d.content.elements.len(), 2);
        assert_eq!(d.content.connections[0].props["port"], json!("443"));
    }

    #[test]
    fn zips_round_trip() {
        let d = sample();
        let back = diagram_from_zip(&diagram_to_zip(&d)).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(
            diagram_from_zip(b"not a zip").unwrap_err().0,
            "The file isn’t a zip file."
        );
        let wrong = json!({"format": "something-else", "version": 1, "diagram": {}});
        assert_eq!(
            diagram_from_file_json(&wrong).unwrap_err().0,
            "The file isn’t an ArchHiero diagram."
        );
        let newer = json!({"format": "arch-hiero-diagram", "version": 2, "diagram": {}});
        assert!(
            diagram_from_file_json(&newer)
                .unwrap_err()
                .0
                .contains("newer version")
        );
        let damaged = json!({"format": "arch-hiero-diagram", "version": 1,
                             "diagram": {"id": "d", "name": "x", "type": "conceptual",
                                         "elements": [{"id": "e"}]}});
        assert_eq!(
            diagram_from_file_json(&damaged).unwrap_err().0,
            "The diagram in the file is incomplete or damaged."
        );
    }

    #[test]
    fn deployed_component_files_have_no_connections_key() {
        let mut d = sample();
        d.meta.diagram_type = DiagramTypeId::DeployedComponent;
        let json: Value = serde_json::from_str(&diagram_file_json(&d)).unwrap();
        assert!(json["diagram"].get("connections").is_none());
        assert!(json["diagram"].get("colors").is_some());
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("A/B: C?", "zip"), "A-B- C-.zip");
        assert_eq!(file_name("  ", "svg"), "Diagram.svg");
    }
}
