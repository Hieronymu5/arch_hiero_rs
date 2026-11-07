//! Conceptual - High Level diagrams: a landscape at a glance, with applications and actors
//! (people) in groups, groups in other groups (or empty), and wide arrows between them where
//! they're needed. Each element has its own colour and font size, and each application and
//! group a solid or dashed border. See `docs/diagrams/high-level-conceptual.md`.

pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{BlockArrow, Color, HighLevel, HighLevelKind, HighLevelProps};

/// The editor's instructions, shown above the canvas: the main ways to add, move, edit,
/// connect, select, and scroll, one per line.
pub const HELP: &str = concat!(
    "• Drag an application, a group, or actors (people, such as stakeholders) onto the canvas, or select one to add it. Drop them into a group to put them in it; a group grows to hold what's in it.\n",
    "• Drag an element, or use the arrow keys, to move it. Resize them with their handles.\n",
    "• Press Enter to edit an element's properties: its name, an application's or actors' description, colour (blue, green, red, yellow, or grey), font size, and an application's or group's solid or dashed border. A group's name sits at its top, or in its middle while it's empty.\n",
    "• Drag from the round handle on the selected element, or use Add arrow in its properties, to draw a wide arrow to another. Select an arrow to make it point both ways.\n",
    "• Hold Shift, Ctrl, or Cmd as you click, or drag an empty part of the canvas to draw a box around elements, to select several and move or remove them together. Cut, copy, and paste with Cmd or Ctrl and X, C, and V.\n",
    "• Drag with the right or middle mouse button, or scroll (with Shift to scroll sideways), to move around. Scroll with Cmd or Ctrl held, pinch on a trackpad, press Cmd or Ctrl and + or -, or use the Zoom slider in the canvas's corner to zoom."
);

/// A new diagram measured by estimate, with its ids counted from `id1`.
#[cfg(test)]
pub(crate) fn test_engine() -> hiero_core::engine::Engine<HighLevel> {
    use hiero_core::engine::Engine;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;
    static ESTIMATE: EstimateText = EstimateText;
    let mut n = 0;
    Engine::<HighLevel>::new(Vec::new(), Vec::new(), NoColors {})
        .with_measure(&ESTIMATE)
        .with_ids(move || {
            n += 1;
            format!("id{n}")
        })
}
