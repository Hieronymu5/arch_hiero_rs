//! The conceptual diagram's kinds, element and arrow properties, and colours.

use hiero_core::figures::component::FIGURE_SIZE;
use hiero_core::geometry::Size;
use hiero_core::look::DiagramColors;
use hiero_core::model::{ContainerLook, KindInfo, Resizable};
use serde::{Deserialize, Serialize};

/// The conceptual diagram type.
pub struct Conceptual;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConceptualKind {
    Actor,
    Domain,
    Application,
    Component,
    Process,
    Job,
    Entity,
    File,
    Section,
}

/// What a component is, which sets how it's drawn. Unset means a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentType {
    Database,
    Storage,
}

impl ComponentType {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Database => "Database",
            Self::Storage => "Storage",
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// What a conceptual element can have beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptualProps {
    /// Shown as a bot, for actors.
    #[serde(default, skip_serializing_if = "is_false")]
    pub agent: bool,
    /// A database or storage, for components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_type: Option<ComponentType>,
    /// Drawn as a cloud, for applications.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cloud_hosted: bool,
}

/// Whether what an arrow shows happens automatically (solid) or by hand (dashed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowMode {
    Automated,
    Manual,
}

/// What a conceptual arrow can have beyond its ends.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptualArrow {
    /// Unset means automated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ArrowMode>,
    /// Text shown beside the start of the arrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Whether a value also comes back, shown as a small return arrow.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bidirectional: bool,
    /// How the target is reached, such as API: shown in a box where the arrow meets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<String>,
    /// How what the arrow carries is changed on the way: shown on the middle of the line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformation: Option<String>,
    /// Set when the arrow was moved by hand so it runs straight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_offset: Option<f64>,
}

impl ConceptualArrow {
    pub fn is_manual(&self) -> bool {
        self.mode == Some(ArrowMode::Manual)
    }
}

/// The longest arrow text, connector, and transformation.
pub const ARROW_LABEL_MAX: usize = 100;
pub const CONNECTOR_MAX: usize = 20;
pub const TRANSFORMATION_MAX: usize = 40;
/// The longest application description.
pub const DESCRIPTION_MAX: usize = 500;

/// Which of the diagram's colours fills elements of a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRole {
    Applications,
    Data,
    Processes,
    Connectors,
}

impl FillRole {
    pub fn of(self, colors: &DiagramColors) -> &str {
        match self {
            Self::Applications => &colors.applications,
            Self::Data => &colors.data,
            Self::Processes => &colors.processes,
            Self::Connectors => &colors.connectors,
        }
    }
}

/// The drawing order for elements on their own: domains at the back, then applications, then
/// components, then everything else.
const DOMAINS: i32 = 0;
const APPLICATIONS: i32 = 1;
const COMPONENTS: i32 = 2;
const FIGURES: i32 = 3;

use ConceptualKind as K;

/// The elements offered in a conceptual diagram's palette.
pub static PALETTE: [KindInfo<ConceptualKind>; 9] = [
    KindInfo {
        connects_to: &[K::Application, K::Component, K::Domain, K::Job],
        layer: FIGURES,
        size: Some(Size::new(120.0, 80.0)),
        ..KindInfo::new(K::Actor, "Actor")
    },
    KindInfo {
        can_contain: &[K::Application],
        layer: DOMAINS,
        size: Some(Size::new(480.0, 180.0)),
        ..KindInfo::new(K::Domain, "Domain")
    },
    KindInfo {
        has_description: true,
        can_contain: &[K::Component, K::Process, K::Entity, K::File],
        container_look: ContainerLook::Boxed,
        header: Some(60.0),
        connects_to: &[K::Application, K::Component, K::Job],
        layer: APPLICATIONS,
        resizable: Resizable::Both,
        size: Some(Size::new(200.0, 100.0)),
        ..KindInfo::new(K::Application, "Application")
    },
    KindInfo {
        can_contain: &[K::Process, K::Entity, K::File],
        container_look: ContainerLook::Boxed,
        connects_to: &[K::Application, K::Component, K::Job],
        layer: COMPONENTS,
        ..KindInfo::new(K::Component, "Component")
    },
    KindInfo {
        layer: FIGURES,
        size: Some(Size::new(140.0, 40.0)),
        ..KindInfo::new(K::Process, "Process")
    },
    KindInfo {
        connects_to: &[K::Application, K::Component],
        layer: FIGURES,
        size: Some(Size::new(160.0, 40.0)),
        ..KindInfo::new(K::Job, "Job")
    },
    KindInfo {
        layer: FIGURES,
        size: Some(Size::new(140.0, 40.0)),
        ..KindInfo::new(K::Entity, "Entity")
    },
    // Data kept as a file, such as an export: placed as an entity is, but drawn as a page with
    // its name underneath, sized as a component drawn as a figure.
    KindInfo {
        layer: FIGURES,
        size: Some(FIGURE_SIZE),
        ..KindInfo::new(K::File, "File")
    },
    KindInfo {
        layer: FIGURES,
        resizable: Resizable::Width,
        size: Some(Size::new(480.0, 40.0)),
        ..KindInfo::new(K::Section, "Section")
    },
];

impl ConceptualKind {
    /// Which colour fills it. Unset means white.
    pub const fn fill(self) -> Option<FillRole> {
        match self {
            Self::Application | Self::Component => Some(FillRole::Applications),
            Self::Process => Some(FillRole::Processes),
            Self::Job => Some(FillRole::Connectors),
            Self::Entity | Self::File => Some(FillRole::Data),
            _ => None,
        }
    }

    pub const fn can_be_agent(self) -> bool {
        matches!(self, Self::Actor)
    }

    pub const fn has_component_type(self) -> bool {
        matches!(self, Self::Component)
    }

    pub const fn can_be_cloud_hosted(self) -> bool {
        matches!(self, Self::Application)
    }
}
