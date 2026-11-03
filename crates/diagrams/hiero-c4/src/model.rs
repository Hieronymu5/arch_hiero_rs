//! The C4 diagram type, its kinds of element, and the properties elements and arrows have.
//!
//! One type covers the C4 model's context, container, and component diagrams: which of them a
//! diagram is, its sub type, only names it. Every diagram offers the same five elements: a
//! person, a system, a container (whose type, such as a website or a database, sets its shape),
//! a component, and a boundary (around a system or a container).

use hiero_core::c4::{BOUNDARY_FOOTER, C4Shape};
use hiero_core::geometry::Size;
use hiero_core::model::{ContainerLook, KindInfo, Resizable};
use hiero_core::palette::{BLUE, FillColors, GREEN, ORANGE, RED};
use serde::{Deserialize, Serialize};

/// The C4 diagram type.
pub struct C4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum C4Kind {
    /// A user, actor, role, or persona.
    Person,
    /// A software system.
    System,
    /// An application or data store inside a system: its type sets its shape.
    Container,
    /// A part of a container.
    Component,
    /// A line round what a system or a container holds.
    Boundary,
}

impl C4Kind {
    /// Whether its type can be named, in place of the usual "Person" or "Software System",
    /// such as a "Customer" or an "External System".
    pub const fn has_custom_type(self) -> bool {
        matches!(self, Self::Person | Self::System)
    }

    /// Whether it names the technology it's built with.
    pub const fn has_technology(self) -> bool {
        matches!(self, Self::Container | Self::Component)
    }
}

/// What a container is, which sets its shape. Unset means a plain box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerType {
    /// A web application or other UI: a browser window.
    WebApplication,
    /// Files served as they are: a folder.
    StaticContent,
    /// A server-side application: a box with a prompt.
    Backend,
    /// A microservice, or another service that owns one capability: a hexagon.
    Service,
    /// A message queue, which one consumer takes each message from: a cylinder on its side.
    Queue,
    /// A topic, which every subscriber gets each message from: a cylinder on its side.
    Topic,
    /// A store of files, such as an S3 bucket: a bucket.
    Store,
    /// A database: a cylinder.
    Database,
}

impl ContainerType {
    /// Every choice, the plain box first, in the order offered.
    pub const CHOICES: [Option<Self>; 9] = [
        None,
        Some(Self::WebApplication),
        Some(Self::StaticContent),
        Some(Self::Backend),
        Some(Self::Service),
        Some(Self::Queue),
        Some(Self::Topic),
        Some(Self::Store),
        Some(Self::Database),
    ];

    pub const fn label(container_type: Option<Self>) -> &'static str {
        match container_type {
            None => "Default",
            Some(Self::WebApplication) => "Website",
            Some(Self::StaticContent) => "Static Content",
            Some(Self::Backend) => "Backend",
            Some(Self::Service) => "Service",
            Some(Self::Queue) => "Queue",
            Some(Self::Topic) => "Topic",
            Some(Self::Store) => "Store",
            Some(Self::Database) => "Database",
        }
    }

    pub const fn shape(container_type: Option<Self>) -> C4Shape {
        match container_type {
            None => C4Shape::Box,
            Some(Self::WebApplication) => C4Shape::Browser,
            Some(Self::StaticContent) => C4Shape::Folder,
            Some(Self::Backend) => C4Shape::Terminal,
            Some(Self::Service) => C4Shape::Hexagon,
            Some(Self::Queue | Self::Topic) => C4Shape::Pipe,
            Some(Self::Store) => C4Shape::Bucket,
            Some(Self::Database) => C4Shape::Cylinder,
        }
    }

    /// The type a container added from the palette entry `entry` starts as: the one the entry
    /// is named after, or none for the generic Container.
    pub fn of_entry(entry: &KindInfo<C4Kind>) -> Option<Self> {
        if entry.kind != C4Kind::Container {
            return None;
        }
        Self::CHOICES
            .into_iter()
            .flatten()
            .find(|t| Self::label(Some(*t)) == entry.label)
    }

    /// The size a container of this type takes until it's resized.
    pub const fn size(container_type: Option<Self>) -> Size {
        match container_type {
            Some(Self::Service) => SERVICE_SIZE,
            Some(Self::Queue | Self::Topic) => QUEUE_SIZE,
            _ => CONTAINER_SIZE,
        }
    }
}

/// What a boundary is round. Unset means a software system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoundaryType {
    Container,
}

impl BoundaryType {
    /// Every choice, a system first.
    pub const CHOICES: [Option<Self>; 2] = [None, Some(Self::Container)];

    pub const fn label(boundary_type: Option<Self>) -> &'static str {
        match boundary_type {
            None => "System",
            Some(Self::Container) => "Container",
        }
    }

    /// The C4 type shown in square brackets in its corner.
    pub const fn type_name(boundary_type: Option<Self>) -> &'static str {
        match boundary_type {
            None => "Software System",
            Some(Self::Container) => "Container",
        }
    }
}

/// An element's colour other than blue, which unset means. The C4 model's usual choice is blue
/// for the software system in scope and what's in it, orange for the organisation's other
/// systems, and red for systems outside it; green is free for anything else to set apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Color {
    Green,
    Orange,
    Red,
}

impl Color {
    /// Every choice, blue first, in the order offered.
    pub const CHOICES: [Option<Self>; 4] =
        [None, Some(Self::Green), Some(Self::Orange), Some(Self::Red)];

    pub const fn label(color: Option<Self>) -> &'static str {
        match color {
            None => "Blue",
            Some(Self::Green) => "Green",
            Some(Self::Orange) => "Orange",
            Some(Self::Red) => "Red",
        }
    }

    /// The fill and border an element of `color` is drawn in.
    pub const fn colors(color: Option<Self>) -> FillColors {
        match color {
            None => BLUE,
            Some(Self::Green) => GREEN,
            Some(Self::Orange) => ORANGE,
            Some(Self::Red) => RED,
        }
    }
}

/// What a C4 element can have beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct C4Props {
    /// Unset means blue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// A container's type, which sets its shape: unset is a plain box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_type: Option<ContainerType>,
    /// What a boundary is round: unset is a software system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_type: Option<BoundaryType>,
    /// What a container or component is built with, such as "Java and Spring Boot".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
    /// A person's or system's type, shown in square brackets in place of the usual "Person"
    /// or "Software System".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
}

/// A relationship: what one element does with another, and how.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct C4Arrow {
    /// Such as "Makes API requests to", shown on the middle of the line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Such as "JSON/HTTPS", shown in square brackets under the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
}

/// The longest element description, technology, and relationship description and technology.
pub const DESCRIPTION_MAX: usize = 500;
pub const TECHNOLOGY_MAX: usize = 60;
/// The longest type in place of a person's or system's usual one.
pub const TYPE_NAME_MAX: usize = 40;
pub const ARROW_DESCRIPTION_MAX: usize = 150;
pub const ARROW_TECHNOLOGY_MAX: usize = 60;

/// A person's figure, name, type, and three lines of description.
pub const PERSON_SIZE: Size = Size::new(200.0, 160.0);
pub const SYSTEM_SIZE: Size = Size::new(240.0, 140.0);
pub const CONTAINER_SIZE: Size = Size::new(240.0, 160.0);
pub const COMPONENT_SIZE: Size = Size::new(240.0, 140.0);
/// A service's hexagon is narrower than the other containers' boxes.
pub const SERVICE_SIZE: Size = Size::new(200.0, 160.0);
/// A queue's or topic's cylinder lies on its side, so it's lower than the other containers.
pub const QUEUE_SIZE: Size = Size::new(240.0, 120.0);
pub const BOUNDARY_SIZE: Size = Size::new(600.0, 400.0);

/// The drawing order of elements not in a boundary: boundaries at the back.
const BOUNDARIES: i32 = 0;
const ELEMENTS: i32 = 1;

use C4Kind as K;

/// Everything but a boundary takes arrows.
const TARGETS: &[C4Kind] = &[K::Person, K::System, K::Container, K::Component];

const fn element(kind: C4Kind, label: &'static str, size: Size) -> KindInfo<C4Kind> {
    KindInfo {
        has_description: true,
        connects_to: TARGETS,
        layer: ELEMENTS,
        resizable: Resizable::Both,
        size: Some(size),
        ..KindInfo::new(kind, label)
    }
}

/// The elements offered in a C4 diagram's palette.
/// A container entry: the generic box, then one for each container type, which a new container
/// from it starts as (see `ContainerType::of_entry`). Its type can be changed afterwards.
const fn container(container_type: Option<ContainerType>) -> KindInfo<C4Kind> {
    element(
        K::Container,
        if container_type.is_none() {
            "Container"
        } else {
            ContainerType::label(container_type)
        },
        ContainerType::size(container_type),
    )
}

/// The elements offered in a C4 diagram's palette: Boundary above Container, Component under
/// it, then each kind of container.
pub static PALETTE: [KindInfo<C4Kind>; 13] = [
    element(K::Person, "Person", PERSON_SIZE),
    element(K::System, "System", SYSTEM_SIZE),
    KindInfo {
        has_description: true,
        // Anything, other boundaries too, such as a container's inside its system's.
        can_contain: &[
            K::Person,
            K::System,
            K::Container,
            K::Component,
            K::Boundary,
        ],
        container_look: ContainerLook::Region,
        header: Some(20.0),
        footer: Some(BOUNDARY_FOOTER),
        layer: BOUNDARIES,
        size: Some(BOUNDARY_SIZE),
        ..KindInfo::new(K::Boundary, "Boundary")
    },
    container(None),
    element(K::Component, "Component", COMPONENT_SIZE),
    container(Some(ContainerType::WebApplication)),
    container(Some(ContainerType::StaticContent)),
    container(Some(ContainerType::Backend)),
    container(Some(ContainerType::Service)),
    container(Some(ContainerType::Queue)),
    container(Some(ContainerType::Topic)),
    container(Some(ContainerType::Store)),
    container(Some(ContainerType::Database)),
];
