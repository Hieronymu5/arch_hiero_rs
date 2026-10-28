//! Physical diagrams: the infrastructure. Networks divided into zones, the users, servers,
//! databases, appliances, and storage inside them, and the traffic between them with its
//! protocol and port. See `docs/diagrams/physical.md`.

pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{Physical, PhysicalArrow, PhysicalColors, PhysicalKind, PhysicalProps};

/// The editor's instructions, shown above the canvas: the main ways to add, move, edit,
/// connect, select, and scroll, one per line.
pub const HELP: &str = concat!(
    "• Drag an element onto the canvas, or select it to add it. Zones divide the network they're dropped on, devices go inside zones, and firewalls can sit across a zone's edge.\n",
    "• Drag an element, or use the arrow keys, to move it. Resize networks and zones with their corner handles.\n",
    "• Press Enter to edit an element's properties, such as a server's type or instances, and Delete to remove it. Hold Shift, Ctrl, or Cmd as you click, or press Shift+Space on each, to select several and move or remove them together.\n",
    "• Drag from the round handle on the selected element, or use Add arrow in its properties, to draw an arrow. Select an arrow to set its protocol and port.\n",
    "• Cut, copy, and paste elements with Cmd or Ctrl and X, C, and V, with what they hold and the arrows between them. They paste into the selected element when it can hold them, or else beside where they were copied from.\n",
    "• Drag an empty part of the canvas to draw a box around elements and select them, holding Shift, Ctrl, or Cmd to add them to the selection.\n",
    "• Drag with the right mouse button to scroll, and use the Zoom slider in the canvas's corner, or Cmd or Ctrl and the scroll wheel, to see more."
);
