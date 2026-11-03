//! C4 model diagrams (<https://c4model.com>): system context, container, and component
//! diagrams. The three types share their shapes, properties, and rules, so they live in one
//! crate, each with its own kinds. See `docs/diagrams/c4-*.md`.

pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{
    C4Arrow, C4Component, C4Container, C4Context, C4Props, C4Type, ComponentKind, ContainerKind,
    ContextKind, Role, Scope,
};
