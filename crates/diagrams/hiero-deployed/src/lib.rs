//! Deployed component diagrams: how a system is deployed. Horizontal layers, each holding its
//! components side by side, with how each is reached and secured, a bulleted description, its
//! tech stack, and its instance count. See `docs/diagrams/deployed-component.md`.

pub mod drawio;
pub mod layout;
pub mod model;
pub mod rules;
pub mod scene;
pub mod tech_stack;
mod tech_stack_data;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{Dependency, DeployedComponent, DeployedKind, DeployedProps};

/// The editor's instructions, shown above the canvas: the main ways to add, move, edit,
/// connect, and scroll, one per line.
pub const HELP: &str = concat!(
    "• Drag a layer or component onto the canvas, or select it to add it after the selected component.\n",
    "• Drag a layer, or use the up and down arrow keys, to move it up or down.\n",
    "• Drag a component to move it, or use the arrow keys: left and right swap it with its neighbour, up and down move it to another layer.\n",
    "• Press Enter to edit an element's properties and Delete to remove it. Hold Shift, Ctrl, or Cmd as you click, or press Shift+Space on each, to select several and move or remove them together.\n",
    "• Drag an empty part of the canvas to scroll, and use the Zoom slider in its corner, or Cmd or Ctrl and the scroll wheel, to see more."
);
