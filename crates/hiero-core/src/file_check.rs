//! The rules a diagram file must follow to be imported, and the message for the first one it
//! breaks. The prototype applies the same rules with the same messages, so both apps accept
//! the same files. The rules are written down in the prototype's `import-file-sync.md`; the
//! numbers in the comments below are its checks.
//!
//! Every optional value has one form: it's left out rather than written empty, `false`, or
//! as its default, so the same diagram never has two files (or two content hashes).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::color::is_hex_color;
use crate::engine::DiagramType;
use crate::model::{ColorScheme, DiagramTypeId, KindInfo};

type Object = Map<String, Value>;

/// The longest diagram name, element name, and diagram description.
const NAME_MAX: usize = 100;
const DIAGRAM_DESCRIPTION_MAX: usize = 1000;

/// What a field's value must be.
#[derive(Debug, Clone, Copy)]
pub enum FieldRule {
    /// `true`: `false` is the default.
    True,
    /// One of `values`. The default, if the field has one, isn't among them: it's left out.
    OneOf {
        values: &'static [&'static str],
        default: Option<&'static str>,
    },
    /// A whole number from 2 to `max`: one is the default.
    Count { max: u32 },
    /// A finite number.
    Number,
    /// Non-blank text of at most `max` characters, called `name` in messages.
    Text { name: &'static str, max: usize },
    /// Text that must be there and not blank, or else `blank` is the message.
    RequiredText {
        name: &'static str,
        max: usize,
        blank: &'static str,
    },
    /// A list of non-blank text: at most `max_items` of them, and at most `max` characters
    /// joined by ", ".
    List {
        name: &'static str,
        max: usize,
        max_items: Option<usize>,
    },
    /// A list of ids, each one `position` places (as a `noun`), listed once, and in the order of
    /// their positions, so each set of ids has one spelling.
    Ids {
        position: fn(&str) -> Option<usize>,
        noun: &'static str,
    },
}

/// A field of an element or arrow beyond the common ones.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub key: &'static str,
    pub rule: FieldRule,
}

/// How long an element's description can be.
#[derive(Debug, Clone, Copy)]
pub struct DescriptionRule {
    pub max: usize,
    /// The most lines, if limited.
    pub max_lines: Option<usize>,
}

/// The key a type's colours are kept under in a file: physical diagrams use their own.
pub fn colors_key(diagram_type: DiagramTypeId) -> &'static str {
    match diagram_type {
        DiagramTypeId::Physical => "physicalColors",
        _ => "colors",
    }
}

// Messages

fn missing(key: &str) -> String {
    format!("“{key}” is missing.")
}

fn must_be(key: &str, what: &str) -> String {
    format!("“{key}” must be {what}.")
}

fn blank(key: &str) -> String {
    format!("“{key}” must not be blank.")
}

fn one_of(key: &str, values: &[&str]) -> String {
    format!("“{key}” must be one of: {}.", values.join(", "))
}

fn left_out(key: &str) -> String {
    format!("“{key}” must be left out when it’s empty or the default.")
}

fn too_long(name: &str, max: usize) -> String {
    format!("{name} must be {max} characters or fewer.")
}

/// "A" or "An" before a kind's label, by its first letter.
fn article(label: &str) -> &'static str {
    if label.starts_with(['A', 'E', 'I', 'O']) {
        "An"
    } else {
        "A"
    }
}

/// A kind's label with its article, such as "an Actor" or "a User".
fn a(label: &str) -> String {
    format!("{} {label}", article(label).to_lowercase())
}

/// A kind's label with its article, capitalised, such as "An Actor".
fn capital_a(label: &str) -> String {
    format!("{} {label}", article(label))
}

fn chars(text: &str) -> usize {
    text.chars().count()
}

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// Whether a value is one that's left out rather than written: nothing, `false`, blank text,
/// or an empty list or object.
fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::String(s) => is_blank(s),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// The alphabetically first key of `object` that `allowed` doesn't accept.
fn first_unknown(object: &Object, allowed: impl Fn(&str) -> bool) -> Option<&str> {
    object
        .keys()
        .map(String::as_str)
        .filter(|k| !allowed(k))
        .min()
}

/// A required, non-blank text field.
fn required_text<'a>(object: &'a Object, key: &str) -> Result<&'a str, String> {
    match object.get(key) {
        None => Err(missing(key)),
        Some(Value::String(s)) if is_blank(s) => Err(blank(key)),
        Some(Value::String(s)) => Ok(s),
        Some(_) => Err(must_be(key, "text")),
    }
}

/// A name: required, not blank, and at most 100 characters.
fn name(object: &Object, key: &str) -> Result<(), String> {
    match object.get(key) {
        None => Err(missing(key)),
        Some(Value::String(s)) if is_blank(s) => Err("Enter a name.".into()),
        Some(Value::String(s)) if chars(s) > NAME_MAX => Err(too_long("Name", NAME_MAX)),
        Some(Value::String(_)) => Ok(()),
        Some(_) => Err(must_be(key, "text")),
    }
}

fn required_number(object: &Object, key: &str) -> Result<f64, String> {
    match object.get(key) {
        None => Err(missing(key)),
        Some(value) => value.as_f64().ok_or_else(|| must_be(key, "a number")),
    }
}

fn required_list<'a>(object: &'a Object, key: &str) -> Result<&'a [Value], String> {
    match object.get(key) {
        None => Err(missing(key)),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(must_be(key, "a list")),
    }
}

/// An optional size: a number greater than 0.
fn size(object: &Object, key: &str) -> Result<(), String> {
    match object.get(key) {
        None => Ok(()),
        Some(value) if is_empty(value) => Err(left_out(key)),
        Some(value) => match value.as_f64() {
            None => Err(must_be(key, "a number")),
            Some(n) if n <= 0.0 => Err(must_be(key, "greater than 0")),
            Some(_) => Ok(()),
        },
    }
}

/// An optional description: non-blank, and within `max` characters and `max_lines` lines.
fn description(object: &Object, max: usize, max_lines: Option<usize>) -> Result<(), String> {
    let Some(value) = object.get("description") else {
        return Ok(());
    };
    if is_empty(value) {
        return Err(left_out("description"));
    }
    let text = value
        .as_str()
        .ok_or_else(|| must_be("description", "text"))?;
    if chars(text) > max {
        return Err(too_long("Description", max));
    }
    if let Some(lines) = max_lines
        && text.split('\n').count() > lines
    {
        return Err(format!("Use {lines} lines or fewer."));
    }
    Ok(())
}

/// A list of non-blank text.
fn text_list<'a>(key: &str, value: &'a Value) -> Result<Vec<&'a str>, String> {
    let items = value.as_array().ok_or_else(|| must_be(key, "a list"))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|s| !is_blank(s))
                .ok_or_else(|| format!("“{key}” must be a list of text."))
        })
        .collect()
}

/// Checks one field of an element or arrow, if it's there or required.
fn field(object: &Object, field: &Field) -> Result<(), String> {
    let key = field.key;
    let Some(value) = object.get(key) else {
        return match field.rule {
            FieldRule::RequiredText { blank, .. } => Err(blank.into()),
            _ => Ok(()),
        };
    };
    if let FieldRule::RequiredText { name, max, blank } = field.rule {
        return match value {
            Value::String(s) if is_blank(s) => Err(blank.into()),
            Value::String(s) if chars(s) > max => Err(too_long(name, max)),
            Value::String(_) => Ok(()),
            Value::Null => Err(blank.into()),
            _ => Err(must_be(key, "text")),
        };
    }
    if is_empty(value) {
        return Err(left_out(key));
    }
    match field.rule {
        FieldRule::True => {
            if value != &Value::Bool(true) {
                return Err(must_be(key, "true"));
            }
        }
        FieldRule::OneOf { values, default } => {
            if default.is_some() && value.as_str() == default {
                return Err(left_out(key));
            }
            if !value.as_str().is_some_and(|s| values.contains(&s)) {
                return Err(one_of(key, values));
            }
        }
        FieldRule::Count { max } => {
            let n = value.as_f64().ok_or_else(|| must_be(key, "a number"))?;
            if (n - 1.0).abs() < f64::EPSILON {
                return Err(left_out(key));
            }
            if n.fract() != 0.0 || n < 1.0 || n > f64::from(max) {
                return Err(format!("Enter a whole number from 1 to {max}."));
            }
        }
        FieldRule::Number => {
            if !value.is_number() {
                return Err(must_be(key, "a number"));
            }
        }
        FieldRule::Text { name, max } => {
            let text = value.as_str().ok_or_else(|| must_be(key, "text"))?;
            if chars(text) > max {
                return Err(too_long(name, max));
            }
        }
        FieldRule::RequiredText { .. } => unreachable!("checked above"),
        FieldRule::List {
            name,
            max,
            max_items,
        } => {
            let items = text_list(key, value)?;
            if let Some(most) = max_items
                && items.len() > most
            {
                return Err(format!("Enter up to {most}, separated by commas."));
            }
            if chars(&items.join(", ")) > max {
                return Err(too_long(name, max));
            }
        }
        FieldRule::Ids { position, noun } => {
            let mut seen = BTreeSet::new();
            let mut previous: Option<(&str, usize)> = None;
            for id in text_list(key, value)? {
                let Some(at) = position(id) else {
                    return Err(format!("“{id}” isn’t a known {noun}."));
                };
                if !seen.insert(id) {
                    return Err(format!("“{id}” is listed twice."));
                }
                if let Some((before, before_at)) = previous
                    && at < before_at
                {
                    return Err(format!("“{id}” must come before “{before}”."));
                }
                previous = Some((id, at));
            }
        }
    }
    Ok(())
}

/// Checks a parsed diagram file up to its diagram's type (checks 4 to 9), returning the
/// diagram object and its type.
pub fn check_file(file: &Value) -> Result<(&Object, DiagramTypeId), String> {
    const NOT_OURS: &str = "The file isn’t an ArchHiero diagram.";
    let file = file
        .as_object()
        .filter(|f| {
            f.get("format").and_then(Value::as_str) == Some(crate::file::DIAGRAM_FILE_FORMAT)
        })
        .ok_or(NOT_OURS)?;
    let version = file
        .get("version")
        .and_then(Value::as_f64)
        .filter(|v| v.fract() == 0.0 && *v >= 1.0)
        .ok_or(NOT_OURS)?;
    if version > crate::file::DIAGRAM_FILE_VERSION as f64 {
        return Err(
            "The diagram was saved by a newer version of ArchHiero. Update it and try again."
                .into(),
        );
    }
    if let Some(key) = first_unknown(file, |k| matches!(k, "format" | "version" | "diagram")) {
        return Err(format!("The file has a field “{key}” that isn’t known."));
    }
    let diagram = file
        .get("diagram")
        .and_then(Value::as_object)
        .ok_or(NOT_OURS)?;
    let at = |message: String| format!("The diagram: {message}");
    required_text(diagram, "id").map_err(at)?;
    name(diagram, "name").map_err(at)?;
    let types: Vec<&str> = DiagramTypeId::ALL.iter().map(|t| t.as_str()).collect();
    let diagram_type = match diagram.get("type") {
        None => return Err(at(missing("type"))),
        Some(value) => value
            .as_str()
            .and_then(DiagramTypeId::parse)
            .ok_or_else(|| at(one_of("type", &types)))?,
    };
    Ok((diagram, diagram_type))
}

/// A kind as written in files, such as "load-balancer".
fn kind_name<K: Serialize>(kind: &K) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Where a message points for an element: its name, or else its id.
fn element_prefix(object: Option<&Object>) -> String {
    let text = |key: &str| {
        object
            .and_then(|o| o.get(key))
            .and_then(Value::as_str)
            .filter(|s| !is_blank(s))
    };
    match (text("label"), text("id")) {
        (Some(label), _) => format!("“{label}”"),
        (None, Some(id)) => format!("The element {id}"),
        (None, None) => "An element".into(),
    }
}

/// An element that passed its own checks.
struct Checked<'a, K: 'static> {
    object: &'a Object,
    id: &'a str,
    label: &'a str,
    info: &'static KindInfo<K>,
    prefix: String,
}

/// Checks a diagram of type `T` from its description on (checks 10 to 32), once
/// `check_file` has accepted the rest.
pub fn check_diagram<T: DiagramType>(diagram: &Object) -> Result<(), String> {
    let at = |message: String| format!("The diagram: {message}");

    // 10. The description.
    if let Some(value) = diagram.get("description") {
        if is_empty(value) {
            return Err(at(left_out("description")));
        }
        let text = value
            .as_str()
            .ok_or_else(|| at(must_be("description", "text")))?;
        if chars(text) > DIAGRAM_DESCRIPTION_MAX {
            return Err(at(too_long("Description", DIAGRAM_DESCRIPTION_MAX)));
        }
    }

    // 11. The colours, for types that have them.
    let colors_key = colors_key(T::ID);
    let has_colors = !T::Colors::FIELDS.is_empty();
    if has_colors && let Some(value) = diagram.get(colors_key) {
        check_colors::<T>(colors_key, value).map_err(at)?;
    }

    // 12. The dates.
    for key in ["createdAt", "updatedAt"] {
        let text = match diagram.get(key) {
            None => return Err(at(missing(key))),
            Some(value) => value.as_str().ok_or_else(|| at(must_be(key, "text")))?,
        };
        if text.parse::<jiff::Timestamp>().is_err() {
            return Err(at(must_be(key, "a date and time")));
        }
    }

    // 13 and 14. The lists of elements and arrows.
    let elements = required_list(diagram, "elements").map_err(at)?;
    let connections = if T::ID.has_arrows() {
        required_list(diagram, "connections").map_err(at)?
    } else {
        if diagram.contains_key("connections") {
            return Err(at("“connections” isn’t a field a diagram has.".into()));
        }
        &[]
    };

    // 15. Anything else.
    let allowed = |k: &str| {
        matches!(
            k,
            "id" | "name"
                | "type"
                | "description"
                | "createdAt"
                | "updatedAt"
                | "elements"
                | "connections"
        ) || (has_colors && k == colors_key)
    };
    if let Some(key) = first_unknown(diagram, allowed) {
        return Err(at(format!("“{key}” isn’t a field a diagram has.")));
    }

    // 16 to 23. Each element on its own.
    let kinds: Vec<(String, &'static KindInfo<T::Kind>)> = T::palette()
        .iter()
        .map(|info| (kind_name(&info.kind), info))
        .collect();
    let checked = elements
        .iter()
        .map(|value| check_element::<T>(value, &kinds))
        .collect::<Result<Vec<_>, _>>()?;

    // 24. Unique ids.
    let mut ids = BTreeSet::new();
    for element in &checked {
        if !ids.insert(element.id) {
            return Err(format!("The id {} is used twice.", element.id));
        }
    }

    // 25. Containers.
    let by_id: BTreeMap<&str, &Checked<T::Kind>> = checked.iter().map(|e| (e.id, e)).collect();
    for element in &checked {
        check_parent(element, &by_id)?;
    }

    // 26 to 32. Each arrow.
    let mut pairs = BTreeSet::new();
    for value in connections {
        check_arrow::<T>(value, &by_id, &mut ids, &mut pairs)?;
    }
    Ok(())
}

fn check_colors<T: DiagramType>(key: &str, value: &Value) -> Result<(), String> {
    if is_empty(value) {
        return Err(left_out(key));
    }
    let colors = value.as_object().ok_or_else(|| must_be(key, "an object"))?;
    let defaults = T::Colors::default();
    // In alphabetical order, so the first problem doesn't depend on how the file is written.
    let colors: BTreeMap<&String, &Value> = colors.iter().collect();
    for (field, color) in colors {
        if !T::Colors::FIELDS.iter().any(|(k, _)| k == field) {
            return Err(format!("The colour “{field}” isn’t one this type has."));
        }
        let text = color
            .as_str()
            .filter(|c| is_hex_color(c) && c.to_lowercase() == *c);
        let Some(text) = text else {
            return Err(format!("The colour “{field}” must be written as #rrggbb."));
        };
        if defaults.get(field).as_deref() == Some(text) {
            return Err(format!(
                "The colour “{field}” must be left out when it’s the default."
            ));
        }
    }
    Ok(())
}

fn check_element<'a, T: DiagramType>(
    value: &'a Value,
    kinds: &[(String, &'static KindInfo<T::Kind>)],
) -> Result<Checked<'a, T::Kind>, String> {
    let prefix = element_prefix(value.as_object());
    let at = |message: String| format!("{prefix}: {message}");
    // 16.
    let object = value
        .as_object()
        .ok_or_else(|| "An element must be an object.".to_owned())?;
    // 17 to 19.
    let id = required_text(object, "id").map_err(at)?;
    let kind_names: Vec<&str> = kinds.iter().map(|(name, _)| name.as_str()).collect();
    let info = match object.get("kind") {
        None => return Err(at(missing("kind"))),
        Some(kind) => kinds
            .iter()
            .find(|(name, _)| kind.as_str() == Some(name.as_str()))
            .map(|(_, info)| *info)
            .ok_or_else(|| at(one_of("kind", &kind_names)))?,
    };
    name(object, "label").map_err(at)?;
    let label = object["label"].as_str().unwrap_or_default();
    // 20 and 21.
    required_number(object, "x").map_err(at)?;
    required_number(object, "y").map_err(at)?;
    size(object, "width").map_err(at)?;
    size(object, "height").map_err(at)?;
    if let Some(parent) = object.get("parentId") {
        if is_empty(parent) {
            return Err(at(left_out("parentId")));
        }
        if !parent.is_string() {
            return Err(at(must_be("parentId", "text")));
        }
    }
    // 22.
    let fields = T::file_fields(info.kind);
    let allowed = |k: &str| {
        matches!(
            k,
            "id" | "kind" | "label" | "x" | "y" | "width" | "height" | "parentId"
        ) || (info.has_description && k == "description")
            || fields.iter().any(|f| f.key == k)
    };
    if let Some(key) = first_unknown(object, allowed) {
        return Err(at(format!("“{key}” isn’t a field {} has.", a(info.label))));
    }
    // 23.
    if info.has_description {
        description(object, T::DESCRIPTION.max, T::DESCRIPTION.max_lines).map_err(at)?;
    }
    for f in fields {
        field(object, f).map_err(at)?;
    }
    Ok(Checked {
        object,
        id,
        label,
        info,
        prefix,
    })
}

fn check_parent<K: Copy + PartialEq>(
    element: &Checked<K>,
    by_id: &BTreeMap<&str, &Checked<K>>,
) -> Result<(), String> {
    let Some(parent_id) = element.object.get("parentId").and_then(Value::as_str) else {
        return Ok(());
    };
    let at = |message: String| format!("{}: {message}", element.prefix);
    let parent = by_id
        .get(parent_id)
        .ok_or_else(|| at(format!("The parent {parent_id} isn’t in the diagram.")))?;
    // Follow the parents up: reaching this element again means it's inside itself.
    let mut seen = BTreeSet::from([element.id]);
    let mut up = Some(parent_id);
    while let Some(id) = up {
        if !seen.insert(id) {
            return Err(at("It can’t be inside itself.".into()));
        }
        up = by_id
            .get(id)
            .and_then(|e| e.object.get("parentId"))
            .and_then(Value::as_str);
    }
    if !parent.info.can_contain.contains(&element.info.kind) {
        return Err(at(format!(
            "{} can’t hold {}.",
            capital_a(parent.info.label),
            a(element.info.label)
        )));
    }
    Ok(())
}

fn check_arrow<'a, T: DiagramType>(
    value: &'a Value,
    by_id: &BTreeMap<&str, &Checked<T::Kind>>,
    ids: &mut BTreeSet<&'a str>,
    pairs: &mut BTreeSet<(&'a str, &'a str)>,
) -> Result<(), String> {
    let object = value.as_object();
    let text = |key: &str| {
        object
            .and_then(|o| o.get(key))
            .and_then(Value::as_str)
            .filter(|s| !is_blank(s))
    };
    let ends = text("sourceId")
        .and_then(|s| by_id.get(s))
        .zip(text("targetId").and_then(|t| by_id.get(t)));
    let prefix = match (ends, text("id")) {
        (Some((source, target)), _) => {
            format!("The arrow from “{}” to “{}”", source.label, target.label)
        }
        (None, Some(id)) => format!("The arrow {id}"),
        (None, None) => "An arrow".into(),
    };
    let at = |message: String| format!("{prefix}: {message}");
    // 26.
    let object = object.ok_or_else(|| "An arrow must be an object.".to_owned())?;
    let id = required_text(object, "id").map_err(at)?;
    if !ids.insert(id) {
        return Err(format!("The id {id} is used twice."));
    }
    let source_id = required_text(object, "sourceId").map_err(at)?;
    let target_id = required_text(object, "targetId").map_err(at)?;
    // 27.
    for end in [source_id, target_id] {
        if !by_id.contains_key(end) {
            return Err(at(format!("There’s no element with id {end}.")));
        }
    }
    let (source, target) = (by_id[source_id], by_id[target_id]);
    // 28 to 30.
    if source_id == target_id {
        return Err(at(
            "An arrow can’t point to the element it starts from.".into()
        ));
    }
    if !source.info.connects_to.contains(&target.info.kind) {
        return Err(at(format!(
            "{} can’t have an arrow to {}.",
            capital_a(source.info.label),
            a(target.info.label)
        )));
    }
    if !pairs.insert((source_id, target_id)) {
        return Err(at(format!(
            "There’s already an arrow from “{}” to “{}”.",
            source.label, target.label
        )));
    }
    // 31.
    let allowed = |k: &str| {
        matches!(k, "id" | "sourceId" | "targetId") || T::ARROW_FIELDS.iter().any(|f| f.key == k)
    };
    if let Some(key) = first_unknown(object, allowed) {
        return Err(at(format!("“{key}” isn’t a field an arrow has.")));
    }
    // 32.
    for f in T::ARROW_FIELDS {
        field(object, f).map_err(at)?;
    }
    Ok(())
}
