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

/// A light fill with the darker border and text drawn on it, such as a C4 software system's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillColors {
    pub fill: &'static str,
    pub border: &'static str,
}

/// Blue: a physical diagram's blue zone, and in a C4 diagram the software system in scope and
/// everything inside it.
pub const BLUE: FillColors = FillColors {
    fill: "#dae8fc",
    border: "#2d5f9e",
};

/// Green: a physical diagram's green zone, and a colour any C4 element can take.
pub const GREEN: FillColors = FillColors {
    fill: CORE_COMPONENT.fill,
    border: "#356b24",
};

/// Orange: in a C4 diagram, another of the organisation's own software systems.
pub const ORANGE: FillColors = FillColors {
    fill: "#ffe6cc",
    border: "#b46504",
};

/// Red: a physical diagram's red zone, and in a C4 diagram a software system outside the
/// organisation.
pub const RED: FillColors = FillColors {
    fill: EXTERNAL_DEPENDENCY.fill,
    border: "#9c3530",
};

/// Yellow: a colour a sequence diagram's participants can take.
pub const YELLOW: FillColors = FillColors {
    fill: INTERNAL_DEPENDENCY.fill,
    border: "#9a7210",
};

/// Grey: a colour a high level conceptual diagram's applications and groups can take, and its
/// groups' usual one.
pub const GREY: FillColors = FillColors {
    fill: "#f5f5f5",
    border: "#666666",
};
