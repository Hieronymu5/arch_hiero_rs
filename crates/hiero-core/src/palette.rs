//! Colour tables used by more than one diagram type.

/// What a deployed component is, and the colours it's drawn in: a light fill, with a darker
/// border, as draw.io's. Physical zones outside a network reuse the fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentColors {
    pub label: &'static str,
    pub fill: &'static str,
    pub border: &'static str,
}

/// A core component of the system itself: green.
pub const CORE_COMPONENT: ComponentColors = ComponentColors {
    label: "Core Component",
    fill: "#d5e8d4",
    border: "#82b366",
};

/// Something of the same organisation's that the system depends on: yellow.
pub const INTERNAL_DEPENDENCY: ComponentColors = ComponentColors {
    label: "Internal Dependency",
    fill: "#fff2cc",
    border: "#d6b656",
};

/// Something of another organisation's that the system depends on: red.
pub const EXTERNAL_DEPENDENCY: ComponentColors = ComponentColors {
    label: "External Dependency",
    fill: "#f8cecc",
    border: "#b85450",
};
