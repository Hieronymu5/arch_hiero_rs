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
    let [r, g, b] = channels(hex).map(|c| {
        crate::geometry::js_round(f64::from(c) * fraction + 255.0 * (1.0 - fraction)) as u8
    });
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn hex_colors_are_recognised() {
        assert!(is_hex_color("#a0B1c2"));
        assert!(!is_hex_color("a0b1c2"));
        assert!(!is_hex_color("#a0b1"));
    }
}
