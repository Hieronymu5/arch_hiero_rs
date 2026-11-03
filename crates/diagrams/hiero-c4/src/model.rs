//! The C4 diagram types, their kinds of element, and the properties elements and arrows have.
//!
//! The three types share one set of roles (a person, a software system, a website, and so on),
//! each type offering the ones its level of detail uses. Each type has its own kind enum, so
//! a diagram of one type can't hold another's kinds.

use hiero_core::c4::{BOUNDARY_FOOTER, C4Shape};
use hiero_core::engine::DiagramType;
use hiero_core::geometry::Size;
use hiero_core::model::{ContainerLook, KindInfo, NoColors, Resizable};
use hiero_core::palette::{BLUE, FillColors, ORANGE, RED};
use serde::{Deserialize, Serialize};

/// A C4 system context diagram: a software system, the people who use it, and the other
/// systems it works with.
pub struct C4Context;
/// A C4 container diagram: the applications and data stores inside a software system.
pub struct C4Container;
/// A C4 component diagram: the components inside a container.
pub struct C4Component;

/// What an element is, whichever C4 diagram it's on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Person,
    SoftwareSystem,
    SystemBoundary,
    ContainerBoundary,
    WebApplication,
    StaticContent,
    Backend,
    Store,
    Database,
    Service,
    Queue,
    Topic,
    Component,
}

impl Role {
    /// The shape it's drawn as. A person is drawn as the actor figure instead.
    pub const fn shape(self) -> Option<C4Shape> {
        Some(match self {
            Self::Person => return None,
            Self::SoftwareSystem => C4Shape::Box,
            Self::SystemBoundary | Self::ContainerBoundary => C4Shape::Boundary,
            Self::WebApplication => C4Shape::Browser,
            Self::StaticContent => C4Shape::Folder,
            Self::Backend => C4Shape::Terminal,
            Self::Store => C4Shape::Bucket,
            Self::Database => C4Shape::Cylinder,
            Self::Service => C4Shape::Hexagon,
            Self::Queue | Self::Topic => C4Shape::Pipe,
            Self::Component => C4Shape::Component,
        })
    }

    /// The C4 type shown in square brackets under its name, with any technology.
    pub const fn type_name(self) -> Option<&'static str> {
        match self {
            Self::Person => None,
            Self::SoftwareSystem | Self::SystemBoundary => Some("Software System"),
            Self::Component => Some("Component"),
            Self::ContainerBoundary
            | Self::WebApplication
            | Self::StaticContent
            | Self::Backend
            | Self::Store
            | Self::Database
            | Self::Service
            | Self::Queue
            | Self::Topic => Some("Container"),
        }
    }

    /// Whether it's in scope, internal, or external, which sets its colour.
    pub const fn has_scope(self) -> bool {
        matches!(self, Self::SoftwareSystem)
    }

    /// Whether it names the technology it's built with.
    pub const fn has_technology(self) -> bool {
        matches!(
            self,
            Self::WebApplication
                | Self::StaticContent
                | Self::Backend
                | Self::Store
                | Self::Database
                | Self::Service
                | Self::Queue
                | Self::Topic
                | Self::Component
        )
    }

    pub const fn is_boundary(self) -> bool {
        matches!(self, Self::SystemBoundary | Self::ContainerBoundary)
    }
}

/// Where a software system stands relative to the one the diagram is about. Unset means it's
/// the system in scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// Another of the organisation's own systems.
    Internal,
    /// A system outside the organisation.
    External,
}

impl Scope {
    pub const fn label(scope: Option<Self>) -> &'static str {
        match scope {
            None => "In scope",
            Some(Self::Internal) => "Internal",
            Some(Self::External) => "External",
        }
    }

    /// The colours a software system is drawn in: blue in scope, orange internal, and red
    /// external.
    pub const fn colors(scope: Option<Self>) -> FillColors {
        match scope {
            None => BLUE,
            Some(Self::Internal) => ORANGE,
            Some(Self::External) => RED,
        }
    }
}

/// What a C4 element can have beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct C4Props {
    /// A software system's place: unset in scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    /// What a container or component is built with, such as "Java and Spring Boot".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
}

/// A relationship: what one element does with another, and how.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct C4Arrow {
    /// Such as "Makes API requests to", shown on the middle of the line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Such as "JSON/HTTPS", shown in square brackets under the description. Not on system
    /// context diagrams, which leave technology out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
}

/// The longest element description, technology, and relationship description and technology.
pub const DESCRIPTION_MAX: usize = 500;
pub const TECHNOLOGY_MAX: usize = 60;
pub const ARROW_DESCRIPTION_MAX: usize = 150;
pub const ARROW_TECHNOLOGY_MAX: usize = 60;

/// What the three C4 types share: elements with the same properties and arrows, and kinds that
/// each play a role.
pub trait C4Type: DiagramType<Props = C4Props, Arrow = C4Arrow, Colors = NoColors> {
    /// What an element of `kind` is.
    fn role(kind: Self::Kind) -> Role;
    /// Whether relationships show a technology: not on system context diagrams.
    const ARROW_TECHNOLOGY: bool;
}

/// The size of a person, a software system, a container, and a component.
pub const PERSON_SIZE: Size = Size::new(120.0, 80.0);
pub const SYSTEM_SIZE: Size = Size::new(240.0, 140.0);
pub const CONTAINER_SIZE: Size = Size::new(240.0, 160.0);
pub const COMPONENT_SIZE: Size = Size::new(240.0, 140.0);
/// A service's hexagon is narrower than the other containers' boxes.
pub const SERVICE_SIZE: Size = Size::new(200.0, 160.0);
/// A queue's or topic's cylinder lies on its side, so it's lower than the other containers.
pub const QUEUE_SIZE: Size = Size::new(240.0, 120.0);

/// The drawing order of elements not in a boundary: system boundaries at the back, then
/// container boundaries, then everything else.
const SYSTEM_BOUNDARIES: i32 = 0;
const CONTAINER_BOUNDARIES: i32 = 1;
const ELEMENTS: i32 = 2;

const fn element<K: Copy>(kind: K, label: &'static str, connects_to: &'static [K]) -> KindInfo<K> {
    KindInfo {
        has_description: true,
        connects_to,
        layer: ELEMENTS,
        resizable: Resizable::Both,
        size: Some(CONTAINER_SIZE),
        ..KindInfo::new(kind, label)
    }
}

const fn person<K: Copy>(kind: K, connects_to: &'static [K]) -> KindInfo<K> {
    KindInfo {
        connects_to,
        layer: ELEMENTS,
        size: Some(PERSON_SIZE),
        ..KindInfo::new(kind, "Person")
    }
}

const fn boundary<K: Copy>(
    kind: K,
    label: &'static str,
    can_contain: &'static [K],
    layer: i32,
    size: Size,
) -> KindInfo<K> {
    KindInfo {
        has_description: true,
        can_contain,
        container_look: ContainerLook::Region,
        header: Some(20.0),
        footer: Some(BOUNDARY_FOOTER),
        layer,
        size: Some(size),
        ..KindInfo::new(kind, label)
    }
}

// System context

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextKind {
    Person,
    SoftwareSystem,
}

const CONTEXT_TARGETS: &[ContextKind] = &[ContextKind::Person, ContextKind::SoftwareSystem];

/// The elements offered in a system context diagram's palette.
pub static CONTEXT_PALETTE: [KindInfo<ContextKind>; 2] = [
    person(ContextKind::Person, CONTEXT_TARGETS),
    KindInfo {
        size: Some(SYSTEM_SIZE),
        ..element(
            ContextKind::SoftwareSystem,
            "Software System",
            CONTEXT_TARGETS,
        )
    },
];

impl C4Type for C4Context {
    fn role(kind: ContextKind) -> Role {
        match kind {
            ContextKind::Person => Role::Person,
            ContextKind::SoftwareSystem => Role::SoftwareSystem,
        }
    }
    const ARROW_TECHNOLOGY: bool = false;
}

// Container

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerKind {
    Person,
    SoftwareSystem,
    SystemBoundary,
    WebApplication,
    StaticContent,
    Backend,
    Store,
    Database,
    /// A microservice, or another service that owns one capability: a hexagon.
    Service,
    /// A message queue, which one consumer takes each message from: a cylinder on its side.
    Queue,
    /// A topic, which every subscriber gets each message from: a cylinder on its side.
    Topic,
}

const CONTAINER_TARGETS: &[ContainerKind] = {
    use ContainerKind as K;
    &[
        K::Person,
        K::SoftwareSystem,
        K::WebApplication,
        K::StaticContent,
        K::Backend,
        K::Store,
        K::Database,
        K::Service,
        K::Queue,
        K::Topic,
    ]
};

/// The elements offered in a container diagram's palette.
pub static CONTAINER_PALETTE: [KindInfo<ContainerKind>; 11] = {
    use ContainerKind as K;
    [
        person(K::Person, CONTAINER_TARGETS),
        KindInfo {
            size: Some(SYSTEM_SIZE),
            ..element(K::SoftwareSystem, "Software System", CONTAINER_TARGETS)
        },
        boundary(
            K::SystemBoundary,
            "System",
            &[
                K::WebApplication,
                K::StaticContent,
                K::Backend,
                K::Store,
                K::Database,
                K::Service,
                K::Queue,
                K::Topic,
            ],
            SYSTEM_BOUNDARIES,
            Size::new(600.0, 400.0),
        ),
        element(K::WebApplication, "Website", CONTAINER_TARGETS),
        element(K::StaticContent, "Static Content", CONTAINER_TARGETS),
        element(K::Backend, "Backend", CONTAINER_TARGETS),
        KindInfo {
            size: Some(SERVICE_SIZE),
            ..element(K::Service, "Service", CONTAINER_TARGETS)
        },
        KindInfo {
            size: Some(QUEUE_SIZE),
            ..element(K::Queue, "Queue", CONTAINER_TARGETS)
        },
        KindInfo {
            size: Some(QUEUE_SIZE),
            ..element(K::Topic, "Topic", CONTAINER_TARGETS)
        },
        element(K::Store, "Store", CONTAINER_TARGETS),
        element(K::Database, "Database", CONTAINER_TARGETS),
    ]
};

impl C4Type for C4Container {
    fn role(kind: ContainerKind) -> Role {
        match kind {
            ContainerKind::Person => Role::Person,
            ContainerKind::SoftwareSystem => Role::SoftwareSystem,
            ContainerKind::SystemBoundary => Role::SystemBoundary,
            ContainerKind::WebApplication => Role::WebApplication,
            ContainerKind::StaticContent => Role::StaticContent,
            ContainerKind::Backend => Role::Backend,
            ContainerKind::Store => Role::Store,
            ContainerKind::Database => Role::Database,
            ContainerKind::Service => Role::Service,
            ContainerKind::Queue => Role::Queue,
            ContainerKind::Topic => Role::Topic,
        }
    }
    const ARROW_TECHNOLOGY: bool = true;
}

// Component

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    Person,
    SoftwareSystem,
    SystemBoundary,
    ContainerBoundary,
    WebApplication,
    Component,
    Database,
    Store,
}

const COMPONENT_TARGETS: &[ComponentKind] = {
    use ComponentKind as K;
    &[
        K::Person,
        K::SoftwareSystem,
        K::WebApplication,
        K::Component,
        K::Database,
        K::Store,
    ]
};

/// The elements offered in a component diagram's palette.
pub static COMPONENT_PALETTE: [KindInfo<ComponentKind>; 8] = {
    use ComponentKind as K;
    [
        person(K::Person, COMPONENT_TARGETS),
        KindInfo {
            size: Some(SYSTEM_SIZE),
            ..element(K::SoftwareSystem, "Software System", COMPONENT_TARGETS)
        },
        boundary(
            K::SystemBoundary,
            "System",
            &[
                K::ContainerBoundary,
                K::WebApplication,
                K::Database,
                K::Store,
            ],
            SYSTEM_BOUNDARIES,
            Size::new(800.0, 560.0),
        ),
        boundary(
            K::ContainerBoundary,
            "Container",
            &[K::Component, K::Database, K::Store],
            CONTAINER_BOUNDARIES,
            Size::new(600.0, 400.0),
        ),
        element(K::WebApplication, "UI Container", COMPONENT_TARGETS),
        KindInfo {
            size: Some(COMPONENT_SIZE),
            ..element(K::Component, "Component", COMPONENT_TARGETS)
        },
        element(K::Database, "Database", COMPONENT_TARGETS),
        element(K::Store, "Store", COMPONENT_TARGETS),
    ]
};

impl C4Type for C4Component {
    fn role(kind: ComponentKind) -> Role {
        match kind {
            ComponentKind::Person => Role::Person,
            ComponentKind::SoftwareSystem => Role::SoftwareSystem,
            ComponentKind::SystemBoundary => Role::SystemBoundary,
            ComponentKind::ContainerBoundary => Role::ContainerBoundary,
            ComponentKind::WebApplication => Role::WebApplication,
            ComponentKind::Component => Role::Component,
            ComponentKind::Database => Role::Database,
            ComponentKind::Store => Role::Store,
        }
    }
    const ARROW_TECHNOLOGY: bool = true;
}
