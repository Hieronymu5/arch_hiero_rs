//! The deployed component diagram's kinds and element properties. It has no arrows and no
//! custom colours.

use hiero_core::model::KindInfo;
use hiero_core::palette::{
    CORE_COMPONENT, ComponentColors, EXTERNAL_DEPENDENCY, INTERNAL_DEPENDENCY,
};
use serde::{Deserialize, Serialize};

/// The deployed component diagram type.
pub struct DeployedComponent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeployedKind {
    Layer,
    Component,
}

/// What a component depends on that isn't part of the system. Unset means it's a core
/// component of the system itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Dependency {
    Internal,
    External,
}

impl Dependency {
    pub const ALL: [Self; 2] = [Self::Internal, Self::External];

    pub const fn colors(self) -> ComponentColors {
        match self {
            Self::Internal => INTERNAL_DEPENDENCY,
            Self::External => EXTERNAL_DEPENDENCY,
        }
    }
}

/// A component's colours, from what it is.
pub fn component_colors(dependency: Option<Dependency>) -> ComponentColors {
    dependency.map_or(CORE_COMPONENT, Dependency::colors)
}

fn is_empty_list(list: &Option<Vec<String>>) -> bool {
    list.as_ref().is_none_or(Vec::is_empty)
}

/// What a layer or component can have beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployedProps {
    /// What sets a layer apart, such as "Internet facing" or "PCI DSS".
    #[serde(default, skip_serializing_if = "is_empty_list")]
    pub characteristics: Option<Vec<String>>,
    /// How a component is reached and secured, such as HTTPS, JWT, and XSRF.
    #[serde(default, skip_serializing_if = "is_empty_list")]
    pub protocols: Option<Vec<String>>,
    /// The ids of the technologies a component is built on.
    #[serde(default, skip_serializing_if = "is_empty_list")]
    pub tech_stack: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency: Option<Dependency>,
    /// How many instances run. Unset means one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_count: Option<u32>,
}

/// The most instances a component can have, bullets in a description, and protocols.
pub const MAX_INSTANCE_COUNT: u32 = 9999;
pub const MAX_DESCRIPTION_ROWS: usize = 20;
pub const MAX_PROTOCOLS: usize = 5;
pub const CHARACTERISTICS_MAX: usize = 200;
pub const PROTOCOLS_MAX: usize = 100;
pub const DESCRIPTION_MAX: usize = 2000;

/// The height of a layer's header, and the gap it keeps around its components.
pub const LAYER_HEADER: f64 = 50.0;
pub const LAYER_PADDING: f64 = 20.0;

/// How a deployed component diagram is drawn, whatever the app theme.
pub mod colors {
    pub const LAYER_FILL: &str = "#e6e6e6";
    pub const LAYER_BORDER: &str = "#b3b3b3";
    pub const BAR_FILL: &str = "#f5f5f5";
    pub const CHARACTERISTICS_BORDER: &str = "#666666";
    pub const PROTOCOL_BORDER: &str = "#1b1f24";
    pub const LOGO_FILL: &str = "#ffffff";
    pub const TEXT: &str = "#1b1f24";
}

/// The elements offered in a deployed component diagram's palette.
pub static PALETTE: [KindInfo<DeployedKind>; 2] = [
    KindInfo {
        can_contain: &[DeployedKind::Component],
        header: Some(LAYER_HEADER),
        padding: Some(LAYER_PADDING),
        ..KindInfo::new(DeployedKind::Layer, "Layer")
    },
    KindInfo {
        has_description: true,
        ..KindInfo::new(DeployedKind::Component, "Component")
    },
];

/// A list as stored: its items trimmed, without blank ones, and unset when that leaves none.
pub fn cleaned_list(items: Option<Vec<String>>) -> Option<Vec<String>> {
    let items: Vec<String> = items?
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    (!items.is_empty()).then_some(items)
}

/// A comma-separated list, such as "HTTPS, JWT", as its items, or unset when it's empty.
pub fn list_from(text: &str) -> Option<Vec<String>> {
    let items: Vec<String> = text
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    (!items.is_empty()).then_some(items)
}
