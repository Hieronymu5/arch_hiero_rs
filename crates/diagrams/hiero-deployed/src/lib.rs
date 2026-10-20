//! Deployed component diagrams: how a system is deployed. Horizontal layers, each holding its
//! components side by side, with how each is reached and secured, a bulleted description, its
//! tech stack, and its instance count. See `docs/diagrams/deployed-component.md`.

pub mod layout;
pub mod model;
pub mod rules;
pub mod scene;
pub mod tech_stack;
mod tech_stack_data;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{DeployedComponent, DeployedKind, DeployedProps, Dependency};

/// The editor's instructions, shown above the canvas.
pub const HELP: &str = "Drag a layer or component onto the grid, or select it to add it: a component goes after the selected one, or at the end of the selected layer. Layers stack from top to bottom, and the components in a layer sit side by side. Move a layer up or down by dragging it, or focus it and use the up and down arrow keys. Move a component by dragging it to where it should go, or focus it and use the left and right arrow keys to swap it with the one beside it, and the up and down arrow keys to move it to the layer above or below. Press Enter to edit its properties and Delete to remove it; removing a layer removes its components too. Select more than one element by holding Shift, Ctrl, or Cmd as you click each, or by focusing each and pressing Shift+Space, to move them together by dragging, or to remove them together with Delete. Scroll the canvas by dragging an empty part of it, or focus it and use the arrow keys. Zoom with the Zoom slider in its bottom-right corner, or with Cmd or Ctrl and the scroll wheel.";
