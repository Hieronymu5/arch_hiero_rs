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

/// The editor's instructions, shown above the canvas.
pub const HELP: &str = "Drag an element onto the grid, or select it to add it. Move a placed element by dragging it, or focus it and use the arrow keys (hold Shift for bigger steps). Press Enter to edit its properties and Delete to remove it. Select more than one element by holding Shift, Ctrl, or Cmd as you click each, or by focusing each and pressing Shift+Space, to move them together by dragging or with the arrow keys, or to remove them together with Delete. Applications dropped onto a domain go inside it, components dropped onto an application go inside it, and processes and entities dropped onto an application or component go inside that. An application or component holding elements shows its name and description at its top. Resize a domain or application with the handles on its top-left and bottom-right corners, by dragging or with the arrow keys. To draw an arrow between applications, components, and jobs, or from an actor to an application, component, job, or domain, drag from the round handle on the selected element, or use Add arrow in its properties. Drag an arrow sideways, or select it and use the arrow keys, to run it straight between the shapes. Give an arrow a connector, such as API, in its properties to show it in a box where the arrow meets its target, or a transformation to show it in a box on the middle of the line. Name a part of the diagram with a section placed above it, and resize it across with the handles on its ends. Scroll the canvas by dragging an empty part of it, or focus it and use the arrow keys. Zoom with the Zoom slider in its bottom-right corner, or with Cmd or Ctrl and the scroll wheel.";
