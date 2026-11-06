//! Colour helpers: WCAG contrast and readable text colours. A port of `utils/colors.ts`.

/// Dark text for light colours, as used throughout the app.
pub const DARK_TEXT: &str = "#1b1f24";
/// Light text for dark colours.
pub const LIGHT_TEXT: &str = "#ffffff";

/// The red, green, and blue of a `#rrggbb` colour, from 0 to 255. Malformed colours read as
/// black.
pub fn channels(hex: &str) -> [u8; 3] {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    [
        ((value >> 16) & 255) as u8,
        ((value >> 8) & 255) as u8,
        (value & 255) as u8,
    ]
}

/// Whether `text` is a `#rrggbb` colour.
pub fn is_hex_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// A colour's relative luminance, as WCAG defines it: 0 for black, 1 for white.
pub fn luminance(hex: &str) -> f64 {
    let [r, g, b] = channels(hex).map(|channel| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The WCAG contrast ratio between two colours, from 1 to 21.
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// Dark or light text, whichever contrasts more with `background`.
pub fn readable_text(background: &str) -> &'static str {
    if contrast_ratio(DARK_TEXT, background) >= contrast_ratio(LIGHT_TEXT, background) {
        DARK_TEXT
    } else {
        LIGHT_TEXT
    }
}

/// Black or white, whichever contrasts more with `background`.
pub fn black_or_white(background: &str) -> &'static str {
    if contrast_ratio("#000000", background) >= contrast_ratio("#ffffff", background) {
        "#000000"
    } else {
        "#ffffff"
    }
}

/// `hex` mixed with white, keeping `fraction` of it.
pub fn tint(hex: &str, fraction: f64) -> String {
    mix(hex, "#ffffff", fraction)
}

/// `a` mixed with `b`, keeping `fraction` of `a`.
pub fn mix(a: &str, b: &str, fraction: f64) -> String {
    let (a, b) = (channels(a), channels(b));
    let [r, g, b] = [0, 1, 2].map(|i| {
        crate::geometry::js_round(f64::from(a[i]) * fraction + f64::from(b[i]) * (1.0 - fraction))
            as u8
    });
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_gamma(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

pub(crate) fn to_oklab([r, g, b]: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = [r, g, b].map(to_linear);
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

/// Linear sRGB, which may fall outside 0 to 1 for colours sRGB can't show.
fn from_oklab([lightness, a, b]: [f64; 3]) -> [f64; 3] {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}

fn in_gamut(rgb: [f64; 3]) -> bool {
    rgb.iter().all(|c| (-1e-4..=1.0 + 1e-4).contains(c))
}

/// The sRGB colour of OKLab `lightness, a, b`, its chroma weakened until sRGB can show it.
pub(crate) fn from_oklab_fitted([lightness, a, b]: [f64; 3]) -> [u8; 3] {
    let at = |k: f64| from_oklab([lightness, a * k, b * k]);
    let mut linear = at(1.0);
    if !in_gamut(linear) {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            if in_gamut(at(mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        linear = at(lo);
    }
    linear.map(to_gamma)
}

/// The lightness of an element's border, unless its fill is dark already.
const BORDER_LIGHTNESS: f64 = 0.55;
/// How much stronger a border's colour is than its fill's.
const BORDER_CHROMA: f64 = 3.2;
/// The strongest a border's colour gets.
const BORDER_CHROMA_MAX: f64 = 0.13;

/// A border for an element filled with `fill`: a darker shade of its colour, as draw.io's and
/// the deployed component and flowchart types' borders are. A grey fill gets a grey border.
pub fn border_for(fill: &str) -> String {
    let [lightness, a, b] = to_oklab(channels(fill));
    let chroma = a.hypot(b);
    let border = BORDER_LIGHTNESS.min(lightness * 0.7);
    let scale = if chroma < 1e-4 {
        0.0
    } else {
        (chroma * BORDER_CHROMA).min(BORDER_CHROMA_MAX) / chroma
    };
    let [r, g, b] = from_oklab_fitted([border, a * scale, b * scale]);
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borders_are_a_darker_shade_of_their_fill() {
        // A pale blue gets a mid blue, about as dark as draw.io's.
        let [r, g, b] = channels(&border_for("#d0e6fb"));
        assert!(b > r && b > g);
        assert!(luminance(&border_for("#d0e6fb")) < luminance("#d0e6fb") / 3.0);
        // A grey stays grey.
        let [r, g, b] = channels(&border_for("#f5f5f5"));
        assert!(r == g && g == b);
        // A dark fill gets a darker border still.
        assert!(luminance(&border_for("#1466b3")) < luminance("#1466b3"));
    }

    #[test]
    fn contrast_of_black_and_white_is_21() {
        assert!((contrast_ratio("#000000", "#ffffff") - 21.0).abs() < 1e-9);
    }

    #[test]
    fn readable_text_picks_light_on_dark() {
        assert_eq!(readable_text("#1466b3"), LIGHT_TEXT);
        assert_eq!(readable_text("#ffe599"), DARK_TEXT);
    }

    #[test]
    fn tint_mixes_with_white() {
        assert_eq!(tint("#000000", 0.5), "#808080");
    }

    #[test]
    fn mix_takes_the_share_of_each() {
        // The default component colour: a quarter of the way from applications to processes.
        assert_eq!(mix("#d0e6fb", "#1466b3", 0.75), "#a1c6e9");
        assert_eq!(mix("#000000", "#ffffff", 1.0), "#000000");
    }

    #[test]
    fn hex_colors_are_recognised() {
        assert!(is_hex_color("#a0B1c2"));
        assert!(!is_hex_color("a0b1c2"));
        assert!(!is_hex_color("#a0b1"));
    }
}
