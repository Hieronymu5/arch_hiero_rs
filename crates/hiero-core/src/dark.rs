//! The dark canvas: how the editor recolours a scene's light colours when the app is dark.
//!
//! Scenes and exports keep their light colours. Only the painter maps them, flipping each
//! colour's lightness in OKLab and keeping its hue, so dark text and lines turn light and light
//! fills turn dark. White is not as dark as the paper, so even a white box stands off it, and a
//! container's fill ([`Tone::Container`]) sits between the two, so the elements in it stand out.

use crate::color::{from_oklab_fitted, to_oklab};
use crate::scene::Tone;

/// The paper behind a dark canvas, painted by the editor rather than mapped: OKLab lightness
/// 0.20, cool like the app's dark surface.
pub const PAPER: [u8; 3] = [0x13, 0x16, 0x1b];

/// The lightness white maps to in an element.
pub const ELEMENT_DARKEST: f64 = 0.35;
/// The lightness white maps to in a container's fill.
pub const CONTAINER_DARKEST: f64 = 0.24;
/// The lightness black maps to.
pub const LIGHTEST: f64 = 0.96;
/// How much stronger a fill's colour gets, so dark tints don't go muddy.
pub const CHROMA: f64 = 1.6;
/// How strong a coloured line's or text's colour is, such as a blue arrow's: softer than a
/// fill's, so it doesn't glare against the dark paper.
pub const INK_CHROMA: f64 = 0.6;
/// How much lighter a coloured line or text gets, to stay readable with less colour.
pub const INK_SHIFT: f64 = 0.05;
/// The ink lift of every diagram type, as their dark text and borders would otherwise read dim
/// (see [`crate::scene::Scene::ink_lift`]).
pub const INK_LIFT: f64 = 0.05;
/// Colours with less chroma than this are greys, which a scene's ink lift brightens.
const GREY_CHROMA: f64 = 0.03;
/// The lightest a lifted grey gets.
const LIFTED_LIGHTEST: f64 = 0.98;

/// Which part of a shape a colour paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Fill,
    /// A line or text.
    Ink,
}

/// `rgb` as the dark canvas shows it, painting `part` of a shape of `tone`. A grey, such as text
/// or a border, gets `ink_lift` lighter, the more so the darker it was: a scene's
/// [`crate::scene::Scene::ink_lift`]. A coloured line or text gets [`INK_CHROMA`] and
/// [`INK_SHIFT`].
pub fn map(rgb: [u8; 3], tone: Tone, part: Part, ink_lift: f64) -> [u8; 3] {
    let darkest = match tone {
        Tone::Element | Tone::Detail => ELEMENT_DARKEST,
        Tone::Container => CONTAINER_DARKEST,
        Tone::Paper => return PAPER,
        Tone::Fixed => return rgb,
    };
    let [lightness, a, b] = to_oklab(rgb);
    let mut mapped = LIGHTEST - (LIGHTEST - darkest) * lightness;
    let grey = a.hypot(b) < GREY_CHROMA;
    let mut chroma = CHROMA;
    let part = if tone == Tone::Detail {
        Part::Ink
    } else {
        part
    };
    if grey {
        if ink_lift > 0.0 {
            mapped = (mapped + ink_lift * (1.0 - lightness)).min(LIFTED_LIGHTEST.max(mapped));
        }
    } else if part == Part::Ink {
        mapped = (mapped + INK_SHIFT).min(LIFTED_LIGHTEST);
        chroma = INK_CHROMA;
    }
    from_oklab_fitted([mapped, a * chroma, b * chroma])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::channels;

    fn lightness(rgb: [u8; 3]) -> f64 {
        to_oklab(rgb)[0]
    }

    #[test]
    fn the_paper_is_as_dark_as_its_lightness() {
        assert!((lightness(PAPER) - 0.20).abs() < 0.005);
    }

    #[test]
    fn white_and_black_swap_places() {
        let white = map([255, 255, 255], Tone::Element, Part::Fill, 0.0);
        let black = map([0, 0, 0], Tone::Element, Part::Fill, 0.0);
        assert!((lightness(white) - ELEMENT_DARKEST).abs() < 0.005);
        assert!((lightness(black) - LIGHTEST).abs() < 0.005);
    }

    #[test]
    fn containers_sit_between_the_paper_and_elements() {
        let white = [255, 255, 255];
        let container = lightness(map(white, Tone::Container, Part::Fill, 0.0));
        assert!(lightness(PAPER) < container);
        assert!(container < lightness(map(white, Tone::Element, Part::Fill, 0.0)));
    }

    #[test]
    fn keeps_a_colours_hue() {
        // A pale blue stays blue: more blue than red.
        let [r, _, b] = map(channels("#dae8fc"), Tone::Element, Part::Fill, 0.0);
        assert!(b > r);
        // A pale yellow stays yellow: more red and green than blue.
        let [r, g, b] = map(channels("#fff2cc"), Tone::Element, Part::Fill, 0.0);
        assert!(r > b && g > b);
    }

    #[test]
    fn an_ink_lift_brightens_greys_but_not_colours() {
        let text = channels(crate::color::DARK_TEXT);
        assert!(
            lightness(map(text, Tone::Element, Part::Fill, 0.1))
                > lightness(map(text, Tone::Element, Part::Fill, 0.0)) + 0.05
        );
        let blue = channels("#0066cc");
        assert_eq!(
            map(blue, Tone::Element, Part::Fill, 0.1),
            map(blue, Tone::Element, Part::Fill, 0.0)
        );
    }

    #[test]
    fn coloured_lines_are_softer_and_lighter_than_fills() {
        let blue = channels("#0066cc");
        let chroma = |rgb| {
            let [_, a, b] = to_oklab(rgb);
            f64::hypot(a, b)
        };
        let fill = map(blue, Tone::Element, Part::Fill, 0.0);
        let line = map(blue, Tone::Element, Part::Ink, 0.0);
        assert!(chroma(line) < chroma(fill));
        assert!(lightness(line) > lightness(fill));
        // Greys are lines either way.
        let text = channels(crate::color::DARK_TEXT);
        assert_eq!(
            map(text, Tone::Element, Part::Ink, 0.0),
            map(text, Tone::Element, Part::Fill, 0.0)
        );
    }

    #[test]
    fn fixed_colours_stay_and_the_paper_is_the_paper() {
        assert_eq!(
            map([255, 255, 255], Tone::Fixed, Part::Fill, 0.0),
            [255, 255, 255]
        );
        assert_eq!(map([255, 255, 255], Tone::Paper, Part::Fill, 0.0), PAPER);
    }
}
