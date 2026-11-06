//! The dark canvas: how the editor recolours a scene's light colours when the app is dark.
//!
//! Scenes and exports keep their light colours. Only the painter maps them, the way draw.io's
//! dark mode does, so a diagram looks the same in both: the CSS filter
//! `invert(93%) hue-rotate(180deg)`, in sRGB. Light fills turn into dark tints of their colour,
//! and dark lines and text into light ones. White becomes the paper, [`PAPER`].

use crate::scene::Tone;

/// How much the colours are inverted: 93%, as draw.io's.
const INVERT: f64 = 0.93;

/// The paper behind a dark canvas: white, mapped. The editor paints it.
pub const PAPER: [u8; 3] = [0x12, 0x12, 0x12];

/// CSS `hue-rotate(180deg)` as a matrix on sRGB channels, from the Filter Effects spec with
/// cos = −1 and sin = 0.
const HUE_ROTATE_180: [[f64; 3]; 3] = [
    [-0.574, 1.430, 0.144],
    [0.426, 0.430, 0.144],
    [0.426, 1.430, -0.856],
];

/// `rgb` as the dark canvas shows it, for a shape of `tone`.
pub fn map(rgb: [u8; 3], tone: Tone) -> [u8; 3] {
    match tone {
        Tone::Paper => PAPER,
        Tone::Element => {
            let inverted = rgb.map(|c| {
                let c = f64::from(c) / 255.0;
                INVERT * (1.0 - c) + (1.0 - INVERT) * c
            });
            HUE_ROTATE_180.map(|row| {
                let c = row[0] * inverted[0] + row[1] * inverted[1] + row[2] * inverted[2];
                (c.clamp(0.0, 1.0) * 255.0).round() as u8
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::channels;

    /// Within two steps of each channel, as draw.io's PNGs are rounded and antialiased.
    fn near(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
    }

    #[test]
    fn matches_draw_ios_dark_mode() {
        // Each light colour, and what draw.io's dark mode shows for it.
        for (light, dark) in [
            ("#ffffff", "#121212"),
            ("#e6e9f0", "#23252b"),
            ("#f3f5f8", "#191b1d"),
            ("#dae8fc", "#1d293b"),
            ("#fff2cc", "#281d00"),
            ("#d5e8d4", "#1f2f1e"),
            ("#e1d5e7", "#392f3f"),
            ("#d9eef1", "#15282a"),
            ("#f8cecc", "#512d2b"),
            ("#356b24", "#7fae71"),
            ("#9c3530", "#f49b97"),
            ("#6c4fb3", "#ab92e8"),
            ("#22727d", "#62a7b0"),
            ("#333333", "#c1c1c1"),
            ("#1b1f24", "#cfd3d7"),
        ] {
            let mapped = map(channels(light), Tone::Element);
            assert!(
                near(mapped, channels(dark)),
                "{light}: {mapped:?}, not {dark}"
            );
        }
    }

    #[test]
    fn white_is_the_paper() {
        assert_eq!(map([255, 255, 255], Tone::Element), PAPER);
        assert_eq!(map([0, 0, 0], Tone::Paper), PAPER);
    }
}
