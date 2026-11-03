//! C4 model diagrams (<https://c4model.com>): one type for its context, container, and
//! component diagrams, whose sub type names the level. Every diagram offers a person, a system,
//! a container (whose type, such as a website, a service, a queue, or a database, sets its
//! shape), a component, and a boundary. See `docs/diagrams/c4.md`.

pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{BoundaryType, C4, C4Arrow, C4Kind, C4Props, Color, ContainerType};

/// The editor's instructions, shown above the canvas: the main ways to add, move, edit,
/// connect, select, and scroll, one per line.
pub const HELP: &str = concat!(
    "• Drag an element onto the canvas, or select it to add it. Choose a container's shape with its Container type, and a boundary's with its Boundary type, in its properties; drop elements into a boundary. Choose the diagram's level, context, container, or component, in Edit Details.\n",
    "• Drag an element, or use the arrow keys, to move it. Resize elements and boundaries with their handles.\n",
    "• Press Enter to edit an element's properties, such as its colour, and Delete to remove it. Hold Shift, Ctrl, or Cmd as you click, or press Shift+Space on each, to select several and move or remove them together.\n",
    "• Drag from the round handle on the selected element, or use Add arrow in its properties, to draw a relationship. Select a relationship to give it a description and a technology.\n",
    "• Cut, copy, and paste elements with Cmd or Ctrl and X, C, and V, with what they hold and the relationships between them.\n",
    "• Drag an empty part of the canvas to draw a box around elements and select them, holding Shift, Ctrl, or Cmd to add them to the selection.\n",
    "• Drag with the right mouse button, or scroll with a trackpad or with Shift and the scroll wheel, to move around. Use the scroll wheel, a pinch on a trackpad, or the Zoom slider in the canvas's corner to zoom."
);
