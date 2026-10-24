//! Conceptual diagrams: the business view of a system landscape. Actors, domains, the
//! applications in them, the components and data inside those, and the arrows between them.
//! See `docs/diagrams/conceptual.md`.

pub mod arrows;
pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{Conceptual, ConceptualArrow, ConceptualKind, ConceptualProps};

/// The editor's instructions, shown above the canvas: the main ways to add, move, edit,
/// connect, and scroll, one per line.
pub const HELP: &str = concat!(
    "• Drag an element onto the canvas, or select it to add it. Drop applications into domains, and components, processes, entities, and files into applications.\n",
    "• Drag an element, or use the arrow keys, to move it. Resize domains, applications, and sections with their handles.\n",
    "• Press Enter to edit an element's properties and Delete to remove it. Hold Shift, Ctrl, or Cmd as you click, or press Shift+Space on each, to select several and move or remove them together.\n",
    "• Drag from the round handle on the selected element, or use Add arrow in its properties, to draw an arrow. Select an arrow to give it text, a connector, or a transformation.\n",
    "• Drag an empty part of the canvas to scroll, and use the Zoom slider in its corner, or Cmd or Ctrl and the scroll wheel, to see more."
);
