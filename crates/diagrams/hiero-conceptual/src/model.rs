//! The conceptual diagram's kinds, element and arrow properties, and colours.

use hiero_core::figures::component::FIGURE_SIZE;
use hiero_core::geometry::Size;
use hiero_core::look::DiagramColors;
pub use hiero_core::model::ArrowPayload;
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

/// Where a domain's name sits. Unset means centred along its top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamePosition {
    /// Centred along its bottom, under the applications it holds.
    Bottom,
}

/// The usual size of an element's name, in pixels.
pub const NAME_FONT_SIZE: u32 = 16;
/// The usual size of a domain's name, in pixels: larger, as a heading for what it holds.
pub const DOMAIN_FONT_SIZE: u32 = 20;

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
    /// Shown as a group, such as a team, for actors: three people, or bots, one in front of
    /// two. Unset means one.
    #[serde(default, skip_serializing_if = "is_false")]
    pub group: bool,
    /// A database or storage, for components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_type: Option<ComponentType>,
    /// Drawn as a cloud, for applications.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cloud_hosted: bool,
    /// The size of its name, in pixels, for a domain or a box such as an application. Unset
    /// means the kind's: 20 for a domain, 16 for the others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u32>,
    /// Where a domain's name sits. Unset means centred along its top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_position: Option<NamePosition>,
}

impl ConceptualProps {
    /// The size of its name, as an element of `kind`.
    pub fn font_size_of(&self, kind: ConceptualKind) -> u32 {
        self.font_size.unwrap_or(kind.default_font_size())
    }

    /// Whether its name sits along its bottom, as a domain's can.
    pub fn name_at_bottom(&self) -> bool {
        self.name_position == Some(NamePosition::Bottom)
    }
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
    /// What the arrow carries, shown as an icon beside its start, on the other side of the line
    /// from its text. Unset means none.
    #[serde(default, skip_serializing_if = "ArrowPayload::is_none")]
    pub payload: ArrowPayload,
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
/// The longest description of an application or a domain.
pub const DESCRIPTION_MAX: usize = 500;

/// Which of the diagram's colours fills elements of a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRole {
    Applications,
    Components,
    Data,
    Processes,
    Connectors,
}

impl FillRole {
    pub fn of(self, colors: &DiagramColors) -> String {
        match self {
            Self::Applications => colors.applications.clone(),
            Self::Components => colors.components.clone(),
            Self::Data => colors.data.clone(),
            Self::Processes => colors.processes.clone(),
            Self::Connectors => colors.connectors.clone(),
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

/// The smallest a process, a job, or an entity can be resized to: as short as it starts, so
/// making one wider doesn't also make it taller.
const SMALL_BOX_MIN_SIZE: Size = Size::new(120.0, 40.0);

/// A component, which a palette entry can start as a database or storage.
const COMPONENT: KindInfo<ConceptualKind> = KindInfo {
    // Other components only as databases or storage (see `Conceptual::hold_problem`), such as
    // the database a service keeps its data in.
    can_contain: &[K::Process, K::Entity, K::File, K::Component],
    container_look: ContainerLook::Boxed,
    // Actors too, such as a person a system sends a report to, and domains, for a system that
    // acts on a whole domain.
    connects_to: &[
        K::Actor,
        K::Application,
        K::Component,
        K::Domain,
        K::Job,
        K::Process,
        K::Entity,
        K::File,
    ],
    layer: COMPONENTS,
    ..KindInfo::new(K::Component, "Component")
};

/// The palette entries for a database and storage: components that start as that type, at
/// the size of their figure. Their type can be changed afterwards.
pub const DATABASE_ENTRY: &str = "Database";
pub const STORAGE_ENTRY: &str = "Storage";

/// The elements offered in a conceptual diagram's palette: the kinds, then sections, files,
/// databases, and storage, the last two being components that start as that type.
pub static PALETTE: [KindInfo<ConceptualKind>; 11] = [
    KindInfo {
        // Other actors too, such as a person who hands work to an assistant.
        connects_to: &[
            K::Actor,
            K::Application,
            K::Component,
            K::Domain,
            K::Job,
            K::Process,
            K::Entity,
            K::File,
        ],
        layer: FIGURES,
        size: Some(Size::new(120.0, 80.0)),
        ..KindInfo::new(K::Actor, "Actor")
    },
    KindInfo {
        can_contain: &[K::Application],
        has_description: true,
        layer: DOMAINS,
        size: Some(Size::new(480.0, 180.0)),
        ..KindInfo::new(K::Domain, "Domain")
    },
    KindInfo {
        has_description: true,
        can_contain: &[K::Component, K::Process, K::Entity, K::File],
        container_look: ContainerLook::Boxed,
        // The name and a line of description: the usual 40 without a description.
        header: Some(60.0),
        // Actors too, such as a person a system sends a report to, and domains, for a system
        // that acts on a whole domain.
        connects_to: &[
            K::Actor,
            K::Application,
            K::Component,
            K::Domain,
            K::Job,
            K::Process,
            K::Entity,
            K::File,
        ],
        layer: APPLICATIONS,
        resizable: Resizable::Both,
        size: Some(Size::new(200.0, 100.0)),
        ..KindInfo::new(K::Application, "Application")
    },
    COMPONENT,
    KindInfo {
        layer: FIGURES,
        // Actors too, such as a person a system sends a report to, and domains, for a system
        // that acts on a whole domain.
        connects_to: &[
            K::Actor,
            K::Application,
            K::Component,
            K::Domain,
            K::Job,
            K::Process,
            K::Entity,
            K::File,
        ],
        resizable: Resizable::Both,
        min_size: Some(SMALL_BOX_MIN_SIZE),
        size: Some(Size::new(140.0, 40.0)),
        ..KindInfo::new(K::Process, "Process")
    },
    KindInfo {
        connects_to: &[
            K::Actor,
            K::Application,
            K::Component,
            K::Domain,
            K::Process,
            K::Entity,
            K::File,
        ],
        layer: FIGURES,
        resizable: Resizable::Both,
        min_size: Some(SMALL_BOX_MIN_SIZE),
        size: Some(Size::new(160.0, 40.0)),
        ..KindInfo::new(K::Job, "Job")
    },
    KindInfo {
        layer: FIGURES,
        resizable: Resizable::Both,
        min_size: Some(SMALL_BOX_MIN_SIZE),
        size: Some(Size::new(140.0, 40.0)),
        ..KindInfo::new(K::Entity, "Entity")
    },
    // Placed above a part of the diagram, and stretched across it, to name that part.
    KindInfo {
        layer: FIGURES,
        resizable: Resizable::Width,
        size: Some(Size::new(480.0, 40.0)),
        ..KindInfo::new(K::Section, "Section")
    },
    // Data kept as a file, such as an export: placed as an entity is, but drawn as a page with
    // its name underneath, sized as a component drawn as a figure.
    KindInfo {
        layer: FIGURES,
        size: Some(FIGURE_SIZE),
        ..KindInfo::new(K::File, "File")
    },
    // Components drawn as a figure, which, as a file, can't be resized.
    KindInfo {
        palette_label: Some(DATABASE_ENTRY),
        size: Some(FIGURE_SIZE),
        ..COMPONENT
    },
    KindInfo {
        palette_label: Some(STORAGE_ENTRY),
        size: Some(FIGURE_SIZE),
        ..COMPONENT
    },
];

impl ConceptualKind {
    /// Which colour fills it. Unset means white.
    pub const fn fill(self) -> Option<FillRole> {
        match self {
            Self::Application => Some(FillRole::Applications),
            Self::Component => Some(FillRole::Components),
            Self::Process => Some(FillRole::Processes),
            Self::Job => Some(FillRole::Connectors),
            Self::Entity | Self::File => Some(FillRole::Data),
            _ => None,
        }
    }

    pub const fn can_be_agent(self) -> bool {
        matches!(self, Self::Actor)
    }

    pub const fn can_be_group(self) -> bool {
        matches!(self, Self::Actor)
    }

    pub const fn has_component_type(self) -> bool {
        matches!(self, Self::Component)
    }

    pub const fn can_be_cloud_hosted(self) -> bool {
        matches!(self, Self::Application)
    }

    /// Whether its name can sit along its bottom instead of its top, as a domain's.
    pub const fn has_name_position(self) -> bool {
        matches!(self, Self::Domain)
    }

    /// Whether its name can be given a larger or smaller font. A component drawn as a figure
    /// keeps its name's usual size.
    pub const fn has_font_size(self) -> bool {
        matches!(
            self,
            Self::Domain
                | Self::Application
                | Self::Component
                | Self::Process
                | Self::Job
                | Self::Entity
        )
    }

    /// The size of its name unless it has its own.
    pub const fn default_font_size(self) -> u32 {
        match self {
            Self::Domain => DOMAIN_FONT_SIZE,
            _ => NAME_FONT_SIZE,
        }
    }
}
