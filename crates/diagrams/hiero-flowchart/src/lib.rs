//! Flowcharts: a process mapped out step by step, from its start, through its steps and
//! decisions, to where it ends, in the ISO 5807 shapes. See `docs/diagrams/flowchart.md`.

pub mod lanes;
pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{Flowchart, FlowchartArrow, FlowchartKind, FlowchartProps, Orientation};

/// The editor's instructions, shown above the canvas: the main ways to add, move, edit,
/// connect, select, and scroll, one per line.
pub const HELP: &str = concat!(
    "• Drag a step onto the canvas, or select it to add it. A new flowchart starts with a Start and an End: put the steps between them. Add diagram on a subprocess maps it out in a flowchart of its own.\n",
    "• For swimlanes, add a Pool: it starts with two lanes, and Lane adds another. Name each lane after who does its steps, and drop the steps in their lanes. Drag a lane, or use the arrow keys, to reorder it; choose which way the lanes run in the pool's properties.\n",
    "• Drag an element, or use the arrow keys, to move it. Resize it with its handles.\n",
    "• Press Enter to edit an element's name, and Delete to remove it. Hold Shift, Ctrl, or Cmd as you click, or press Shift+Space on each, to select several and move or remove them together.\n",
    "• Drag from the round handle on the selected element, or use Add arrow in its properties, to draw the flow to the next step. A decision's first two arrows are labelled Yes and No; select an arrow to change its label. Arrows to a data store or document are dashed, and a note's dotted.\n",
    "• Cut, copy, and paste elements with Cmd or Ctrl and X, C, and V, with the arrows between them.\n",
    "• Drag an empty part of the canvas to draw a box around elements and select them, holding Shift, Ctrl, or Cmd to add them to the selection.\n",
    "• Drag with the right or middle mouse button, or scroll (with Shift to scroll sideways), to move around. Scroll with Cmd or Ctrl held, pinch on a trackpad, press Cmd or Ctrl and + or -, or use the Zoom slider in the canvas's corner to zoom."
);
