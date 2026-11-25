//! The colour schemes a conceptual diagram can start from.
//!
//! Each scheme sets every one of the diagram's colours ([`DiagramColors`]), only the light
//! ones: the dark canvas maps them, as it does any diagram's. Every fill's text, dark or light,
//! meets WCAG AA in both. Settings offers them in a Scheme dropdown above the colours, which
//! reads Custom once a colour is changed by hand.

use std::collections::BTreeMap;

use crate::look::DiagramColors;
use crate::model::ColorScheme;

/// A named set of a conceptual diagram's colours, to start from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagramColorScheme {
    pub id: &'static str,
    pub name: &'static str,
    /// Each colour, by its key in a diagram file, as `#rrggbb`, in the order of
    /// [`DiagramColors::FIELDS`].
    pub colors: [&'static str; 9],
}

impl DiagramColorScheme {
    /// The scheme's colours, by key.
    pub fn colors(&self) -> BTreeMap<String, String> {
        DiagramColors::FIELDS
            .iter()
            .zip(self.colors)
            .map(|((key, _), color)| ((*key).to_owned(), color.to_owned()))
            .collect()
    }

    /// The colours its swatch shows, the ones that tell schemes apart.
    pub fn key_colors(&self) -> [&'static str; 4] {
        SCHEME_KEY_COLORS.map(|key| {
            let at = DiagramColors::FIELDS
                .iter()
                .position(|(k, _)| *k == key)
                .expect("a colour key");
            self.colors[at]
        })
    }
}

/// Every one of a conceptual diagram's colours, by key: the defaults with `overrides`, those
/// that differ from them, applied.
pub fn colors_with(overrides: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let colors = DiagramColors::from_overrides(overrides);
    DiagramColors::FIELDS
        .iter()
        .filter_map(|(key, _)| Some(((*key).to_owned(), colors.get(key)?)))
        .collect()
}

/// The keys of the colours a scheme's swatch shows.
pub const SCHEME_KEY_COLORS: [&str; 4] = ["applications", "components", "data", "processes"];

/// The colour schemes offered for conceptual diagrams, Classic, the default, first. Colours are
/// domains, applications, components, connectors, data, processes, borders, lines, and section
/// names.
pub const DIAGRAM_COLOR_SCHEMES: [DiagramColorScheme; 10] = [
    DiagramColorScheme {
        id: "classic",
        name: "Classic",
        colors: [
            "#f5f5f5", "#d0e6fb", "#a1c6e9", "#cfd4da", "#ffe599", "#1466b3", "#1e1e1e", "#333333",
            "#0b3d91",
        ],
    },
    DiagramColorScheme {
        id: "harbour",
        name: "Harbour",
        colors: [
            "#f2f5f7", "#cdeceb", "#9ecdcc", "#d5dde3", "#fde8b0", "#0f6e6e", "#1c2a33", "#2f3d47",
            "#0d4f57",
        ],
    },
    DiagramColorScheme {
        id: "sage",
        name: "Sage",
        colors: [
            "#f4f6f1", "#dcead2", "#b5caac", "#d9dccf", "#f3dcc4", "#3f6b3a", "#232a1f", "#383f33",
            "#2f5130",
        ],
    },
    DiagramColorScheme {
        id: "sandstone",
        name: "Sandstone",
        colors: [
            "#f7f4ef", "#f6dfcc", "#e2b9a4", "#ddd6cc", "#e3ecf5", "#a4482a", "#2b211b", "#3d322b",
            "#7a3218",
        ],
    },
    DiagramColorScheme {
        id: "graphite",
        name: "Graphite",
        colors: [
            "#f6f6f6", "#e2e2e2", "#bcbcbc", "#d0d0d0", "#f0f0f0", "#4a4a4a", "#1a1a1a", "#333333",
            "#2a2a2a",
        ],
    },
    DiagramColorScheme {
        id: "clear",
        name: "Clear Sight",
        colors: [
            "#f5f5f5", "#cfe7f7", "#93cbed", "#d9d9d9", "#f7ef9e", "#b04a00", "#000000", "#222222",
            "#005a8f",
        ],
    },
    DiagramColorScheme {
        id: "slate",
        name: "Slate",
        colors: [
            "#f3f5f7", "#dfe5ec", "#b6bfc9", "#d3d8de", "#ffdc9e", "#3b4c60", "#1b232c", "#2e3843",
            "#2b3a4c",
        ],
    },
    DiagramColorScheme {
        id: "blueprint",
        name: "Blueprint",
        colors: [
            "#f2f7fa", "#d8eef8", "#a7c2d6", "#cfdde6", "#fbfdff", "#123d6e", "#0f2440", "#1d3557",
            "#123d6e",
        ],
    },
    DiagramColorScheme {
        id: "evergreen",
        name: "Evergreen",
        colors: [
            "#f3f6f3", "#d5efdf", "#a7cbb6", "#d2dbd5", "#fff0a8", "#1d5e3a", "#16261c", "#2a3a30",
            "#17502f",
        ],
    },
    DiagramColorScheme {
        id: "contrast",
        name: "High Contrast",
        colors: [
            "#ffffff", "#c9e1ff", "#97b5da", "#c4c4c4", "#ffe14d", "#002f6c", "#000000", "#000000",
            "#002f6c",
        ],
    },
];

/// The scheme whose colours `colors` are, given every colour by key, ignoring case, or none
/// when any differs: Custom.
pub fn scheme_of(colors: &BTreeMap<String, String>) -> Option<&'static DiagramColorScheme> {
    DIAGRAM_COLOR_SCHEMES.iter().find(|scheme| {
        scheme.colors().iter().all(|(key, color)| {
            colors
                .get(key)
                .is_some_and(|c| c.eq_ignore_ascii_case(color))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{channels, contrast_ratio, readable_text};
    use crate::dark;
    use crate::scene::Tone;

    #[test]
    fn classic_is_the_default_colours_and_names_and_ids_are_unique() {
        let defaults = colors_with(&BTreeMap::new());
        assert_eq!(DIAGRAM_COLOR_SCHEMES[0].colors(), defaults);
        let ids: std::collections::BTreeSet<_> =
            DIAGRAM_COLOR_SCHEMES.iter().map(|s| s.id).collect();
        let names: std::collections::BTreeSet<_> =
            DIAGRAM_COLOR_SCHEMES.iter().map(|s| s.name).collect();
        assert_eq!((ids.len(), names.len()), (10, 10));
        for scheme in &DIAGRAM_COLOR_SCHEMES {
            for color in scheme.colors {
                assert!(crate::color::is_hex_color(color), "{} {color}", scheme.name);
            }
        }
    }

    #[test]
    fn every_fills_text_is_readable_to_wcag_aa_light_and_dark() {
        let hex = |rgb: [u8; 3]| format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        let dark = |color: &str| hex(dark::map(channels(color), Tone::Element));
        let fills = [
            "domains",
            "applications",
            "components",
            "connectors",
            "data",
            "processes",
        ];
        for scheme in &DIAGRAM_COLOR_SCHEMES {
            let colors = scheme.colors();
            for fill in fills {
                let color = &colors[fill];
                let text = readable_text(color);
                let name = scheme.name;
                assert!(contrast_ratio(text, color) >= 4.5, "{name} {fill}");
                assert!(
                    contrast_ratio(&dark(text), &dark(color)) >= 4.5,
                    "{name} {fill}, dark"
                );
            }
            assert!(contrast_ratio(&colors["sectionNames"], &colors["domains"]) >= 4.5);
        }
    }

    #[test]
    fn finds_the_scheme_colours_are_in_any_case_or_none() {
        let harbour = &DIAGRAM_COLOR_SCHEMES[1];
        let upper = harbour
            .colors()
            .into_iter()
            .map(|(k, v)| (k, v.to_uppercase()))
            .collect();
        assert_eq!(scheme_of(&upper), Some(harbour));
        let mut changed = harbour.colors();
        changed.insert("lines".into(), "#000001".into());
        assert_eq!(scheme_of(&changed), None);
        assert_eq!(
            harbour.key_colors(),
            ["#cdeceb", "#9ecdcc", "#fde8b0", "#0f6e6e"]
        );
    }
}
