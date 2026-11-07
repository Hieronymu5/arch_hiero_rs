//! Sequence diagrams: the messages participants send each other over time, each participant
//! with a lifeline running down from it and the messages going across between them, the first
//! at the top. See `docs/diagrams/sequence.md`.

pub mod model;
pub mod rules;
pub mod scene;
pub mod timeline;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{
    ComponentType, MessageKind, ParticipantColor, Sequence, SequenceKind, SequenceMessage,
    SequenceProps,
};

/// The editor's instructions, shown above the canvas: the main ways to add, order, edit, and
/// connect, one per line.
pub const HELP: &str = concat!(
    "• Drag an actor, application, or component onto the canvas, or select it to add it at the end of the row. Participants stand in a row along the top, each with a lifeline under it.\n",
    "• Drag a participant left or right, or use the arrow keys or Move left and Move right, to change its place in the row.\n",
    "• Drag from the round handle on the selected participant to another's lifeline, or to its own, to send a message. A message straight back answers a call as a return; select a call and use Add return to add its answer.\n",
    "• Drag a message up or down, or select it and use the up and down arrow keys, to change its place in time. Select a message to change its text and whether it's a call, asynchronous, or a return.\n",
    "• Select a participant to choose its colour, blue, green, red, or yellow, which its lifeline takes too.\n",
    "• Press Enter to edit a participant's name, and Delete to remove it, with its messages, or the selected message.\n",
    "• Drag with the right or middle mouse button, or scroll (with Shift to scroll sideways), to move around. Scroll with Cmd or Ctrl held, pinch on a trackpad, press Cmd or Ctrl and + or -, or use the Zoom slider in the canvas's corner to zoom."
);

/// A new diagram measured by estimate, with its ids counted from `id1`: its user, then its
/// application.
#[cfg(test)]
pub(crate) fn test_engine() -> hiero_core::engine::Engine<Sequence> {
    use hiero_core::engine::{DiagramType, Engine};
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;
    static ESTIMATE: EstimateText = EstimateText;
    let mut n = 0;
    let mut eng = Engine::<Sequence>::new(Vec::new(), Vec::new(), NoColors {})
        .with_measure(&ESTIMATE)
        .with_ids(move || {
            n += 1;
            format!("id{n}")
        });
    let starting = Sequence::starting_elements(&mut eng);
    eng.set_elements(starting);
    eng
}
