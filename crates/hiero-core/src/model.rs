//! The diagram model: ids, elements, connections, element kinds, and colour schemes. Each
//! diagram type supplies its own kind enum `K`, element properties `P`, arrow properties `A`,
//! and colour scheme; this module holds what they share.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::geometry::Size;

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// A new random (v4 UUID) id.
            pub fn new_v4() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

id_type!(
    /// A diagram's id. Prototype files use UUIDs, but any non-empty string is accepted.
    DiagramId
);
id_type!(
    /// An element's id, unique within its diagram.
    ElementId
);
id_type!(
    /// An arrow's id, unique within its diagram.
    ConnectionId
);

/// The kinds of diagram that can be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagramTypeId {
    Conceptual,
    DeployedComponent,
    Physical,
}

impl DiagramTypeId {
    /// Every diagram type, in menu order.
    pub const ALL: [Self; 3] = [Self::Conceptual, Self::DeployedComponent, Self::Physical];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conceptual => "conceptual",
            Self::DeployedComponent => "deployed-component",
            Self::Physical => "physical",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Conceptual => "Conceptual",
            Self::DeployedComponent => "Deployed Component",
            Self::Physical => "Physical",
        }
    }

    /// Whether diagrams of this type have arrows. Deployed component diagrams don't.
    pub const fn has_arrows(self) -> bool {
        !matches!(self, Self::DeployedComponent)
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == text)
    }
}

impl fmt::Display for DiagramTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serialises whole-number coordinates without a decimal point, as the prototype's JSON has
/// them, so files and agents see `40` rather than `40.0`.
pub mod num {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &f64, s: S) -> Result<S::Ok, S::Error> {
        if value.fract() == 0.0 && value.abs() < 9e15 {
            s.serialize_i64(*value as i64)
        } else {
            s.serialize_f64(*value)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        f64::deserialize(d)
    }

    pub mod opt {
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(value: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
            match value {
                Some(v) => super::serialize(v, s),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
            Option::<f64>::deserialize(d)
        }
    }
}

/// An element placed on a diagram. Positions are canvas pixels from the top-left corner.
/// `K` is the diagram type's kinds and `P` its per-element properties, flattened into the
/// same JSON object as the common fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Element<K, P> {
    pub id: ElementId,
    pub kind: K,
    /// The element's name, shown in bold. Starts as the kind's label.
    pub label: String,
    /// Shown under the name for kinds with a description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Id of the container element this element sits in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ElementId>,
    #[serde(flatten)]
    pub props: P,
    /// Size set by resizing. Unset means the kind's natural size.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "num::opt")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "num::opt")]
    pub height: Option<f64>,
    #[serde(with = "num")]
    pub x: f64,
    #[serde(with = "num")]
    pub y: f64,
}

impl<K, P> Element<K, P> {
    /// The element with `parent_id` set, or cleared.
    pub fn with_parent(mut self, parent_id: Option<ElementId>) -> Self {
        self.parent_id = parent_id;
        self
    }
}

/// An arrow drawn from one element to another, with the diagram type's arrow properties `A`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection<A> {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    #[serde(flatten)]
    pub props: A,
}

/// How a container is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerLook {
    /// A filled region with its name as a header, such as a domain.
    Region,
    /// Looks like any other box until it holds elements, when its text moves to the top.
    Boxed,
}

/// Whether elements of a kind can be resized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resizable {
    No,
    Both,
    /// Across only, from handles on the left and right ends, such as a section.
    Width,
}

/// A kind of element that can be placed on a diagram: what every diagram type's editor
/// understands. Each type keeps its own extra settings in its own tables.
#[derive(Debug, Clone, Copy)]
pub struct KindInfo<K: 'static> {
    pub kind: K,
    pub label: &'static str,
    /// Whether elements of this kind show a description under their name.
    pub has_description: bool,
    /// Size in pixels. Unset means `DEFAULT_ELEMENT_SIZE`.
    pub size: Option<Size>,
    /// Kinds of element this kind can hold. Containers grow to fit what they hold.
    pub can_contain: &'static [K],
    pub container_look: ContainerLook,
    /// Height of a container's header. Unset means `CONTAINER_HEADER`.
    pub header: Option<f64>,
    /// Space a container keeps below what it holds, on top of its padding.
    pub footer: Option<f64>,
    /// Gap a container keeps around what it holds. Unset means `CONTAINER_PADDING`.
    pub padding: Option<f64>,
    /// Whether it can be resized. Containers always can.
    pub resizable: Resizable,
    /// Drawing order for elements not in a container: lower layers behind higher ones.
    pub layer: i32,
    /// Kinds of element this kind can draw arrows to.
    pub connects_to: &'static [K],
}

impl<K: Copy> KindInfo<K> {
    /// A kind with the usual settings: a plain, unresizable box of the default size.
    pub const fn new(kind: K, label: &'static str) -> Self {
        Self {
            kind,
            label,
            has_description: false,
            size: None,
            can_contain: &[],
            container_look: ContainerLook::Region,
            header: None,
            footer: None,
            padding: None,
            resizable: Resizable::No,
            layer: 0,
            connects_to: &[],
        }
    }

    pub fn size(&self) -> Size {
        self.size.unwrap_or(crate::layout::DEFAULT_ELEMENT_SIZE)
    }

    pub fn header(&self) -> f64 {
        self.header.unwrap_or(crate::layout::CONTAINER_HEADER)
    }

    pub fn padding(&self) -> f64 {
        self.padding.unwrap_or(crate::layout::CONTAINER_PADDING)
    }

    pub fn footer(&self) -> f64 {
        self.footer.unwrap_or(0.0)
    }
}

/// A diagram's colours: a struct of `#rrggbb` strings with defaults, edited generically
/// through `FIELDS` and stored as the values that differ from the defaults.
pub trait ColorScheme:
    Clone + Default + PartialEq + fmt::Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
    /// Every colour's key (as serialised) and label, in the order offered.
    const FIELDS: &'static [(&'static str, &'static str)];

    /// The defaults with `overrides` applied. Unknown keys and malformed colours are ignored.
    fn from_overrides(overrides: &BTreeMap<String, String>) -> Self {
        let mut value = serde_json::to_value(Self::default()).unwrap_or_default();
        if let Some(map) = value.as_object_mut() {
            for (key, color) in overrides {
                if map.contains_key(key) && crate::color::is_hex_color(color) {
                    map.insert(key.clone(), serde_json::Value::String(color.to_lowercase()));
                }
            }
        }
        serde_json::from_value(value).unwrap_or_default()
    }

    /// The colours that differ from the defaults.
    fn overrides(&self) -> BTreeMap<String, String> {
        let defaults = Self::default();
        Self::FIELDS
            .iter()
            .filter_map(|(key, _)| {
                let mine = self.get(key)?;
                (Some(mine.clone()) != defaults.get(key)).then(|| ((*key).to_owned(), mine))
            })
            .collect()
    }

    fn get(&self, key: &str) -> Option<String> {
        serde_json::to_value(self)
            .ok()?
            .get(key)?
            .as_str()
            .map(str::to_owned)
    }

    fn set(&mut self, key: &str, color: &str) {
        let mut overrides = self.overrides();
        overrides.insert(key.to_owned(), color.to_owned());
        *self = Self::from_overrides(&overrides);
    }
}

/// The colour scheme of a diagram type without custom colours.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NoColors {}

impl ColorScheme for NoColors {
    const FIELDS: &'static [(&'static str, &'static str)] = &[];
}

/// The properties of an element or arrow with nothing beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NoProps {}

/// A diagram's name, type, description, and dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramMeta {
    pub id: DiagramId,
    #[serde(rename = "type")]
    pub diagram_type: DiagramTypeId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// RFC 3339 UTC.
    pub created_at: String,
    pub updated_at: String,
}

/// The current time as RFC 3339 UTC with milliseconds, as the prototype's `toISOString`.
pub fn now_rfc3339() -> String {
    jiff::Timestamp::now()
        .round(jiff::TimestampRound::new().smallest(jiff::Unit::Millisecond))
        .map_or_else(|_| jiff::Timestamp::now().to_string(), |t| t.to_string())
}

/// The longest diagram name, and description.
pub const NAME_MAX: usize = 100;
pub const DESCRIPTION_MAX: usize = 1000;

/// Checks a diagram's name, returning the message to show when it isn't valid.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() {
        Err("Enter a name.")
    } else if name.chars().count() > NAME_MAX {
        Err("Name must be 100 characters or fewer.")
    } else {
        Ok(())
    }
}

/// Checks a diagram's description, returning the message to show when it isn't valid.
pub fn validate_description(description: &str) -> Result<(), &'static str> {
    if description.chars().count() > DESCRIPTION_MAX {
        Err("Description must be 1000 characters or fewer.")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Kind {
        Box,
        BigBox,
    }

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Props {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        agent: bool,
    }

    #[test]
    fn elements_serialise_flat_like_the_prototype() {
        let element: Element<Kind, Props> = Element {
            id: "e1".into(),
            kind: Kind::BigBox,
            label: "A".into(),
            description: None,
            parent_id: Some("p".into()),
            props: Props { agent: true },
            width: None,
            height: Some(120.0),
            x: 40.0,
            y: 60.5,
        };
        let json = serde_json::to_value(&element).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "e1", "kind": "big-box", "label": "A", "parentId": "p",
                "agent": true, "height": 120, "x": 40, "y": 60.5
            })
        );
        let back: Element<Kind, Props> = serde_json::from_value(json).unwrap();
        assert_eq!(back, element);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = serde_json::json!({
            "id": "e1", "kind": "box", "label": "A", "x": 0, "y": 0, "zoneColor": "red"
        });
        let element: Element<Kind, Props> = serde_json::from_value(json).unwrap();
        assert_eq!(element.props, Props::default());
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TwoColors {
        fill: String,
        line: String,
    }

    impl Default for TwoColors {
        fn default() -> Self {
            Self {
                fill: "#ffffff".into(),
                line: "#000000".into(),
            }
        }
    }

    impl ColorScheme for TwoColors {
        const FIELDS: &'static [(&'static str, &'static str)] =
            &[("fill", "Fill"), ("line", "Line")];
    }

    #[test]
    fn color_overrides_round_trip() {
        let mut overrides = BTreeMap::new();
        overrides.insert("line".to_owned(), "#FF0000".to_owned());
        overrides.insert("bogus".to_owned(), "#00ff00".to_owned());
        overrides.insert("fill".to_owned(), "red".to_owned());
        let colors = TwoColors::from_overrides(&overrides);
        assert_eq!(colors.line, "#ff0000");
        assert_eq!(colors.fill, "#ffffff");
        let expected: BTreeMap<String, String> = [("line".to_owned(), "#ff0000".to_owned())].into();
        assert_eq!(colors.overrides(), expected);
    }

    #[test]
    fn type_ids_parse() {
        assert_eq!(
            DiagramTypeId::parse("deployed-component"),
            Some(DiagramTypeId::DeployedComponent)
        );
        assert_eq!(DiagramTypeId::parse("nope"), None);
        assert!(!DiagramTypeId::DeployedComponent.has_arrows());
    }
}
