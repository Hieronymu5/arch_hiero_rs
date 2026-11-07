//! The high level conceptual diagram type, its two kinds of element, and what they and its
//! arrows have.
//!
//! A diagram is applications and groups: a group holds applications and other groups, or
//! nothing, as a boundary to show something without its parts. Wide arrows join any two of
//! them, where the picture needs one.

use hiero_core::geometry::Size;
use hiero_core::model::{ContainerLook, KindInfo, Resizable};
use hiero_core::palette::{BLUE, FillColors, GREEN, GREY, RED, YELLOW};
use serde::{Deserialize, Serialize};

/// The high level conceptual diagram type.
pub struct HighLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HighLevelKind {
    /// An application, or anything else shown as a box with its name inside, and an optional
    /// description under it.
    Application,
    /// A boundary round applications and other groups, or an empty one.
    Group,
    /// People, such as stakeholders or a team: three actors, one in front of two, with their
    /// name and an optional description under them.
    Actors,
}

impl HighLevelKind {
    /// The colour it's drawn in unless it has its own: applications and actors blue, groups
    /// grey.
    pub const fn default_color(self) -> Color {
        match self {
            Self::Application | Self::Actors => Color::Blue,
            Self::Group => Color::Grey,
        }
    }

    /// The size of its name unless it has its own.
    pub const fn default_font_size(self) -> u32 {
        match self {
            Self::Application | Self::Actors => 14,
            Self::Group => 16,
        }
    }
}

/// A colour an application or group is drawn in: a light fill with a darker border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Color {
    Blue,
    Green,
    Red,
    Yellow,
    Grey,
}

impl Color {
    /// Every choice, in the order offered.
    pub const CHOICES: [Self; 5] = [Self::Blue, Self::Green, Self::Red, Self::Yellow, Self::Grey];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Grey => "Grey",
        }
    }

    /// As stored and in files.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Grey => "grey",
        }
    }

    /// The fill and border it's drawn in.
    pub const fn colors(self) -> FillColors {
        match self {
            Self::Blue => BLUE,
            Self::Green => GREEN,
            Self::Red => RED,
            Self::Yellow => YELLOW,
            Self::Grey => GREY,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// What an application or group has beyond the common fields. Each is left out at its kind's
/// default, so every look has one spelling.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighLevelProps {
    /// Unset is the kind's colour: blue for an application, grey for a group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// The size of its name, in pixels. Unset is the kind's: 14 for an application, 16 for a
    /// group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u32>,
    /// A dashed border, rather than a solid one. Actors have no border.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dashed: bool,
}

impl HighLevelProps {
    /// The colour it's drawn in, as an element of `kind`.
    pub fn color_of(&self, kind: HighLevelKind) -> Color {
        self.color.unwrap_or(kind.default_color())
    }

    /// The size of its name, as an element of `kind`.
    pub fn font_size_of(&self, kind: HighLevelKind) -> u32 {
        self.font_size.unwrap_or(kind.default_font_size())
    }
}

/// A wide arrow between two elements.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockArrow {
    /// A head at each end, for elements that work both ways.
    #[serde(default, skip_serializing_if = "is_false")]
    pub two_way: bool,
}

/// The font sizes offered, in pixels. Any whole size from `FONT_SIZE_MIN` to `FONT_SIZE_MAX`
/// is valid, as an agent or a file can set.
pub const FONT_SIZES: [u32; 11] = [10, 11, 12, 13, 14, 16, 18, 20, 24, 28, 32];
pub const FONT_SIZE_MIN: u32 = 8;
pub const FONT_SIZE_MAX: u32 = 48;

/// The longest an application's description can be.
pub const DESCRIPTION_MAX: usize = 500;

pub const APPLICATION_SIZE: Size = Size::new(160.0, 80.0);
pub const GROUP_SIZE: Size = Size::new(400.0, 240.0);
pub const ACTORS_SIZE: Size = Size::new(160.0, 120.0);

/// The drawing order of elements not in a group: groups at the back.
const GROUPS: i32 = 0;
const APPLICATIONS: i32 = 1;

use HighLevelKind as K;

/// Arrows join any two elements, and a group holds any.
const EVERYTHING: &[HighLevelKind] = &[K::Application, K::Group, K::Actors];

/// The elements offered in the palette.
pub static PALETTE: [KindInfo<HighLevelKind>; 3] = [
    KindInfo {
        connects_to: EVERYTHING,
        has_description: true,
        layer: APPLICATIONS,
        resizable: Resizable::Both,
        size: Some(APPLICATION_SIZE),
        ..KindInfo::new(K::Application, "Application")
    },
    KindInfo {
        can_contain: EVERYTHING,
        connects_to: EVERYTHING,
        container_look: ContainerLook::Region,
        layer: GROUPS,
        size: Some(GROUP_SIZE),
        ..KindInfo::new(K::Group, "Group")
    },
    KindInfo {
        connects_to: EVERYTHING,
        has_description: true,
        layer: APPLICATIONS,
        resizable: Resizable::Both,
        size: Some(ACTORS_SIZE),
        ..KindInfo::new(K::Actors, "Actors")
    },
];
