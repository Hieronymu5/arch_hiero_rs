//! The flowchart type, its kinds of element, and what its arrows have.
//!
//! A flowchart maps out a process: where it starts, its steps and decisions, and where it
//! ends. Each kind is drawn in its ISO 5807 shape and a fixed colour. A pool divides it into
//! swimlanes, one for each role, and the steps sit in the lane of whoever does them.

use hiero_core::flowchart::FlowShape;
use hiero_core::geometry::Size;
use hiero_core::model::{KindInfo, Resizable};
use hiero_core::palette::{BLUE, FillColors, GREEN, RED};
use serde::{Deserialize, Serialize};

/// The flowchart diagram type.
pub struct Flowchart;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowchartKind {
    /// Where the flow begins: nothing leads into it.
    Start,
    /// Where a flow ends: nothing leads out of it.
    End,
    /// A step someone or something does.
    Process,
    /// A question with two or more ways out, each labelled.
    Decision,
    /// Data coming in or going out, such as "Receive order".
    InputOutput,
    /// A step mapped out in a flowchart of its own, which it opens.
    Subprocess,
    /// A form, report, or letter made or used.
    Document,
    /// A database or file the flow reads or writes.
    DataStore,
    /// A step done by hand.
    Manual,
    /// Waiting for time to pass or for something to happen.
    Delay,
    /// One end of a jump to another place in the flow, named like its other end.
    Connector,
    /// A comment on an element, linked to it by a dotted line.
    Note,
    /// The frame round a process's swimlanes, named after the process.
    Pool,
    /// A band of a pool for one role, team, or system, holding the steps it does.
    Lane,
}

impl FlowchartKind {
    /// The shape it's drawn as. A pool and a lane are regions instead.
    pub const fn shape(self) -> Option<FlowShape> {
        Some(match self {
            Self::Start | Self::End => FlowShape::Terminal,
            Self::Process => FlowShape::Process,
            Self::Decision => FlowShape::Decision,
            Self::InputOutput => FlowShape::InputOutput,
            Self::Subprocess => FlowShape::Subprocess,
            Self::Document => FlowShape::Document,
            Self::DataStore => FlowShape::DataStore,
            Self::Manual => FlowShape::Manual,
            Self::Delay => FlowShape::Delay,
            Self::Connector => FlowShape::Connector,
            Self::Note => FlowShape::Note,
            Self::Pool | Self::Lane => return None,
        })
    }

    /// Whether it's a swimlane's pool or lane rather than a step.
    pub const fn is_swimlane(self) -> bool {
        matches!(self, Self::Pool | Self::Lane)
    }

    /// The fill and border it's drawn in.
    pub const fn colors(self) -> FillColors {
        match self {
            Self::Start => GREEN,
            Self::End => RED,
            Self::Decision => DECISION,
            Self::InputOutput => INPUT_OUTPUT,
            Self::Document | Self::DataStore => DATA,
            Self::Connector => CONNECTOR,
            Self::Note => NOTE,
            Self::Pool => POOL,
            Self::Lane => LANE,
            Self::Process | Self::Subprocess | Self::Manual | Self::Delay => BLUE,
        }
    }

    /// Whether arrows to and from it carry data rather than the flow: a data store's and a
    /// document's, drawn dashed.
    pub const fn is_data(self) -> bool {
        matches!(self, Self::DataStore | Self::Document)
    }
}

/// A decision's amber.
pub const DECISION: FillColors = FillColors {
    fill: "#fff2cc",
    border: "#9a7210",
};
/// Input and output's purple.
pub const INPUT_OUTPUT: FillColors = FillColors {
    fill: "#e1d5e7",
    border: "#6c4fb3",
};
/// Documents' and data stores' teal.
pub const DATA: FillColors = FillColors {
    fill: "#d9eef1",
    border: "#22727d",
};
pub const CONNECTOR: FillColors = FillColors {
    fill: "#ffffff",
    border: "#333333",
};
/// A note's pale yellow.
pub const NOTE: FillColors = FillColors {
    fill: "#fbf8e3",
    border: "#8f8640",
};

/// A pool's header and frame, and the darker of its lanes' alternating fills.
pub const POOL: FillColors = FillColors {
    fill: "#e6e9f0",
    border: "#333333",
};
/// The lighter of a pool's lanes' alternating fills.
pub const LANE: FillColors = FillColors {
    fill: "#ffffff",
    border: "#333333",
};
/// Every other lane's fill.
pub const LANE_ALTERNATE: &str = "#f3f5f8";

/// Which way a pool's lanes run. Unset means horizontally: lanes are rows, one under the
/// other, and the flow goes right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Orientation {
    /// Lanes are columns, side by side, and the flow goes down.
    Vertical,
}

impl Orientation {
    /// Every choice, horizontal first.
    pub const CHOICES: [Option<Self>; 2] = [None, Some(Self::Vertical)];

    pub const fn label(orientation: Option<Self>) -> &'static str {
        match orientation {
            None => "Horizontal",
            Some(Self::Vertical) => "Vertical",
        }
    }
}

/// What a flowchart element has beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowchartProps {
    /// Which way a pool's lanes run: unset is horizontally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
}

/// An arrow: the flow from one step to the next, data read or written, or a note's link.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowchartArrow {
    /// Such as "Yes" or "No" out of a decision, shown near its start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The longest arrow label.
pub const LABEL_MAX: usize = 40;
/// The longest connector name.
pub const CONNECTOR_NAME_MAX: usize = 3;

/// Every height is a multiple of 40, so on the 20 px grid any two elements can be centred
/// level with each other and an arrow between them runs straight into a diamond's point.
pub const TERMINAL_SIZE: Size = Size::new(160.0, 40.0);
pub const STEP_SIZE: Size = Size::new(160.0, 80.0);
pub const DECISION_SIZE: Size = Size::new(160.0, 120.0);
pub const DATA_STORE_SIZE: Size = Size::new(120.0, 80.0);
pub const CONNECTOR_SIZE: Size = Size::new(40.0, 40.0);
/// A pool with its header and two lanes.
pub const POOL_SIZE: Size = Size::new(1200.0, 440.0);
/// A lane in a pool of horizontal lanes: as wide as the pool, which it fills.
pub const LANE_SIZE: Size = Size::new(1200.0, 200.0);
/// How wide a lane added to a pool of vertical lanes starts.
pub const COLUMN_WIDTH: f64 = 300.0;
/// The band across a pool's top holding its name, and across a lane's top holding its.
pub const POOL_HEADER: f64 = 40.0;
pub const LANE_HEADER: f64 = 40.0;
/// The lanes a new pool starts with.
pub const STARTING_LANES: [&str; 2] = ["Lane 1", "Lane 2"];

use FlowchartKind as K;

/// Where the flow can go next: any step but a start, and not a note.
const FLOW_TARGETS: &[FlowchartKind] = &[
    K::End,
    K::Process,
    K::Decision,
    K::InputOutput,
    K::Subprocess,
    K::Document,
    K::DataStore,
    K::Manual,
    K::Delay,
    K::Connector,
];

/// What a data store gives data to: the steps that do something.
const DATA_TARGETS: &[FlowchartKind] = &[
    K::Process,
    K::Decision,
    K::InputOutput,
    K::Subprocess,
    K::Manual,
    K::Delay,
];

/// What a note can be about: anything but another note.
const NOTE_TARGETS: &[FlowchartKind] = &[
    K::Start,
    K::End,
    K::Process,
    K::Decision,
    K::InputOutput,
    K::Subprocess,
    K::Document,
    K::DataStore,
    K::Manual,
    K::Delay,
    K::Connector,
];

const fn step(
    kind: FlowchartKind,
    label: &'static str,
    size: Size,
    connects_to: &'static [FlowchartKind],
) -> KindInfo<FlowchartKind> {
    KindInfo {
        connects_to,
        resizable: Resizable::Both,
        size: Some(size),
        layer: 1,
        ..KindInfo::new(kind, label)
    }
}

/// The elements offered in a flowchart's palette.
pub static PALETTE: [KindInfo<FlowchartKind>; 14] = [
    step(K::Start, "Start", TERMINAL_SIZE, FLOW_TARGETS),
    step(K::End, "End", TERMINAL_SIZE, &[]),
    step(K::Process, "Process", STEP_SIZE, FLOW_TARGETS),
    step(K::Decision, "Decision", DECISION_SIZE, FLOW_TARGETS),
    step(K::InputOutput, "Input / Output", STEP_SIZE, FLOW_TARGETS),
    step(K::Subprocess, "Subprocess", STEP_SIZE, FLOW_TARGETS),
    step(K::Document, "Document", STEP_SIZE, FLOW_TARGETS),
    step(K::DataStore, "Data Store", DATA_STORE_SIZE, DATA_TARGETS),
    step(K::Manual, "Manual Step", STEP_SIZE, FLOW_TARGETS),
    step(K::Delay, "Delay", STEP_SIZE, FLOW_TARGETS),
    step(K::Connector, "Connector", CONNECTOR_SIZE, FLOW_TARGETS),
    KindInfo {
        // Notes sit in front of the steps they're about.
        layer: 2,
        ..step(K::Note, "Note", STEP_SIZE, NOTE_TARGETS)
    },
    // A pool and its lanes, behind everything, under a heading of their own.
    KindInfo {
        can_contain: &[K::Lane],
        header: Some(POOL_HEADER),
        padding: Some(0.0),
        size: Some(POOL_SIZE),
        palette_heading: Some("Swimlanes"),
        ..KindInfo::new(K::Pool, "Pool")
    },
    KindInfo {
        can_contain: LANE_HOLDS,
        header: Some(LANE_HEADER),
        size: Some(LANE_SIZE),
        ..KindInfo::new(K::Lane, "Lane")
    },
];

/// What a lane holds: every step, and notes and connectors.
const LANE_HOLDS: &[FlowchartKind] = &[
    K::Start,
    K::End,
    K::Process,
    K::Decision,
    K::InputOutput,
    K::Subprocess,
    K::Document,
    K::DataStore,
    K::Manual,
    K::Delay,
    K::Connector,
    K::Note,
];
