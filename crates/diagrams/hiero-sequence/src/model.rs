//! The sequence diagram type, its participants, and what its messages have.
//!
//! A sequence diagram shows how participants work together over time: they stand in a row
//! along the top, each with a lifeline running down from it, and the messages they send each
//! other go across between the lifelines, the first at the top. The participants are the
//! conceptual diagram's actors, applications, and components, each in one of four colours
//! that its lifeline takes too.

use hiero_core::figures::component::FIGURE_SIZE;
use hiero_core::geometry::Size;
use hiero_core::model::{KindInfo, Resizable};
use hiero_core::palette::{BLUE, FillColors, GREEN, RED, YELLOW};
use serde::{Deserialize, Serialize};

/// The sequence diagram type.
pub struct Sequence;

/// A participant: what sends and receives messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SequenceKind {
    /// A person, or a bot, drawn as a figure with its name under it.
    Actor,
    /// An application, drawn as a box with its name in bold and an optional description under
    /// it.
    Application,
    /// A part of an application, drawn as a box as an application is, or as a database or
    /// storage with only its name.
    Component,
}

/// A participant's colour, which its lifeline and activations take too. Unset means blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParticipantColor {
    Green,
    Red,
    Yellow,
}

impl ParticipantColor {
    /// Every choice, blue first.
    pub const CHOICES: [Option<Self>; 4] =
        [None, Some(Self::Green), Some(Self::Red), Some(Self::Yellow)];

    pub const fn label(color: Option<Self>) -> &'static str {
        match color {
            None => "Blue",
            Some(Self::Green) => "Green",
            Some(Self::Red) => "Red",
            Some(Self::Yellow) => "Yellow",
        }
    }

    /// The fill and border it's drawn in.
    pub const fn colors(color: Option<Self>) -> FillColors {
        match color {
            None => BLUE,
            Some(Self::Green) => GREEN,
            Some(Self::Red) => RED,
            Some(Self::Yellow) => YELLOW,
        }
    }
}

/// What a component is, which sets how it's drawn. Unset means a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentType {
    Database,
    Storage,
}

impl ComponentType {
    /// Every choice, a box first.
    pub const CHOICES: [Option<Self>; 3] = [None, Some(Self::Database), Some(Self::Storage)];

    pub const fn label(kind: Option<Self>) -> &'static str {
        match kind {
            None => "Box",
            Some(Self::Database) => "Database",
            Some(Self::Storage) => "Storage",
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// What a participant has beyond the common fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceProps {
    /// Its colour and its lifeline's: unset is blue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ParticipantColor>,
    /// Shown as a bot, for actors.
    #[serde(default, skip_serializing_if = "is_false")]
    pub agent: bool,
    /// A database or storage, for components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_type: Option<ComponentType>,
}

/// What a message is. Unset means a call the sender waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageKind {
    /// Sent without waiting for an answer: an open head.
    Async,
    /// The answer to a call, back to its caller: dashed, with an open head.
    Return,
}

impl MessageKind {
    /// Every choice, a call first.
    pub const CHOICES: [Option<Self>; 3] = [None, Some(Self::Async), Some(Self::Return)];

    pub const fn label(kind: Option<Self>) -> &'static str {
        match kind {
            None => "Call",
            Some(Self::Async) => "Asynchronous",
            Some(Self::Return) => "Return",
        }
    }

    /// As agents and files spell it.
    pub const fn as_str(kind: Option<Self>) -> &'static str {
        match kind {
            None => "call",
            Some(Self::Async) => "async",
            Some(Self::Return) => "return",
        }
    }
}

/// A message from one participant to another, or to itself. Its place in the diagram's list
/// of messages is its place in time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceMessage {
    /// Unset is a call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MessageKind>,
    /// Such as "getOrder(id)", or what a return gives back, such as "Order": shown above the
    /// line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl SequenceMessage {
    pub fn is_return(&self) -> bool {
        self.kind == Some(MessageKind::Return)
    }
}

/// The longest message text.
pub const LABEL_MAX: usize = 100;
/// The longest description of an application or component.
pub const DESCRIPTION_MAX: usize = 500;

/// A box participant's size: as tall as an actor's figure with one line of name under it, so
/// the lifelines start level. A box grows taller when its name and description need it.
pub const BOX_SIZE: Size = Size::new(160.0, 80.0);
/// An actor's, or a component drawn as a database or storage: the figure with its name under it.
pub const FIGURE: Size = FIGURE_SIZE;

use SequenceKind as K;

/// Any participant can send a message to any other, or to itself.
const EVERYONE: &[SequenceKind] = &[K::Actor, K::Application, K::Component];

/// The participants offered in a sequence diagram's palette.
pub static PALETTE: [KindInfo<SequenceKind>; 3] = [
    KindInfo {
        connects_to: EVERYONE,
        size: Some(FIGURE),
        ..KindInfo::new(K::Actor, "Actor")
    },
    KindInfo {
        connects_to: EVERYONE,
        has_description: true,
        resizable: Resizable::Width,
        size: Some(BOX_SIZE),
        ..KindInfo::new(K::Application, "Application")
    },
    KindInfo {
        connects_to: EVERYONE,
        has_description: true,
        resizable: Resizable::Width,
        size: Some(BOX_SIZE),
        ..KindInfo::new(K::Component, "Component")
    },
];
