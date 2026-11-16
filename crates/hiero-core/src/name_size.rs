//! The sizes an element's name can be given, as on a high level conceptual diagram or a
//! conceptual diagram's domains and boxes, and how tall a header is to fit one.

use crate::geometry::GRID;
use crate::text::{MeasureText, TextStyle, wrap_text};

/// The font sizes offered, in pixels. Any whole size from `FONT_SIZE_MIN` to `FONT_SIZE_MAX`
/// is valid, as an agent or a file can set.
pub const FONT_SIZES: [u32; 11] = [10, 11, 12, 13, 14, 16, 18, 20, 24, 28, 32];
pub const FONT_SIZE_MIN: u32 = 8;
pub const FONT_SIZE_MAX: u32 = 48;

/// The space above and below a name in its header.
pub const NAME_PADDING: f64 = 8.0;
/// The shortest a header is: two grid squares.
pub const HEADER_MIN: f64 = 40.0;

/// The height of a line of text in `size`.
pub fn line_height(size: f64) -> f64 {
    (size * 1.25).round()
}

/// The height of a header for a name in `size`: a line with space above and below it, rounded
/// up to the grid so what's under it stays on it.
pub fn header_for(size: f64) -> f64 {
    let needed = line_height(size) + NAME_PADDING * 2.0;
    ((needed / GRID).ceil() * GRID).max(HEADER_MIN)
}

/// The most lines a description under a name in a header wraps to before its font shrinks.
pub const HEADER_DESCRIPTION_LINES: usize = 3;

/// A description fitted under a name in a header: its lines, and the style they're drawn in.
#[derive(Debug, Clone, PartialEq)]
pub struct FittedText {
    pub lines: Vec<String>,
    pub style: TextStyle,
}

impl FittedText {
    /// How tall its lines are, without the gap above them.
    pub fn height(&self) -> f64 {
        self.lines.len() as f64 * self.style.line_height
    }
}

/// `description` wrapped across `width` in `style`, for a header such as a domain's or a
/// group's, with lines `line_height` tall for a font size. It wraps to at most
/// `HEADER_DESCRIPTION_LINES` lines, a pixel smaller at a time until it fits, down to
/// `FONT_SIZE_MIN`. It's never cut short: what still doesn't fit at that size takes more lines.
/// Blank lines are left out; other line breaks are kept.
pub fn fit_description(
    description: Option<&str>,
    width: f64,
    style: TextStyle,
    line_height: impl Fn(f64) -> f64,
    measure: &dyn MeasureText,
) -> FittedText {
    let text = description
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let smallest = f64::from(FONT_SIZE_MIN).min(style.size);
    let mut size = style.size;
    loop {
        let at = TextStyle {
            size,
            line_height: line_height(size),
            max_lines: usize::MAX,
            ..style
        };
        if text.is_empty() {
            return FittedText {
                lines: Vec::new(),
                style: at,
            };
        }
        let lines = wrap_text(&text, width.max(1.0), at, measure);
        if lines.len() <= HEADER_DESCRIPTION_LINES || size - 1.0 < smallest {
            return FittedText { lines, style: at };
        }
        size -= 1.0;
    }
}

/// `height` rounded up to the grid.
pub fn on_grid(height: f64) -> f64 {
    (height / GRID).ceil() * GRID
}

/// Checks a font size: a whole number of pixels, from `FONT_SIZE_MIN` to `FONT_SIZE_MAX`.
pub fn validate_font_size(size: u32) -> Result<(), String> {
    if (FONT_SIZE_MIN..=FONT_SIZE_MAX).contains(&size) {
        Ok(())
    } else {
        Err(format!(
            "Font size must be from {FONT_SIZE_MIN} to {FONT_SIZE_MAX}."
        ))
    }
}

/// The font sizes offered for a name now `current` pixels: the usual ones, and `current` too
/// when it's another, such as one an agent set.
pub fn font_size_choices(current: u32) -> Vec<u32> {
    let mut sizes = FONT_SIZES.to_vec();
    if !sizes.contains(&current) {
        sizes.push(current);
        sizes.sort_unstable();
    }
    sizes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_wraps_to_three_lines_then_shrinks_and_is_never_cut() {
        use crate::text::{EstimateText, Weight};
        let style = TextStyle::new(13.0, Weight::Regular, 17.0, 1);
        let fit = |text: &str, width: f64| {
            fit_description(
                Some(text),
                width,
                style,
                |size| (size * 17.0 / 13.0).round(),
                &EstimateText,
            )
        };
        assert!(fit(" \n ", 200.0).lines.is_empty());
        // 13 px: 7.15 px a character, so 27 to a 200 px line. Blank lines are left out.
        let short = fit("Orders\n\nBilling", 200.0);
        assert_eq!(short.lines, ["Orders", "Billing"]);
        assert_eq!((short.style.size, short.style.line_height), (13.0, 17.0));
        // Three lines' worth at 13 px fits as it is.
        let three = fit(&"word ".repeat(15), 200.0);
        assert_eq!((three.lines.len(), three.style.size), (3, 13.0));
        // Four lines' worth shrinks until it takes three.
        let four = fit(&"word ".repeat(20), 200.0);
        assert_eq!(four.lines.len(), 3);
        assert!(four.style.size < 13.0 && four.style.size >= 8.0);
        assert_eq!(
            four.style.line_height,
            (four.style.size * 17.0 / 13.0).round()
        );
        // Too much for three lines even at 8 px: more lines, nothing cut.
        let long = fit(&"word ".repeat(100), 200.0);
        assert_eq!(long.style.size, 8.0);
        assert!(long.lines.len() > 3);
        assert!(!long.lines.concat().contains('…'));
        assert_eq!(long.lines.join(" ").split_whitespace().count(), 100);
    }

    #[test]
    fn headers_grow_on_the_grid_for_larger_names() {
        assert_eq!(header_for(14.0), 40.0);
        assert_eq!(header_for(16.0), 40.0);
        assert_eq!(header_for(24.0), 60.0);
        assert_eq!(header_for(32.0), 60.0);
        assert_eq!(header_for(48.0), 80.0);
    }

    #[test]
    fn an_unusual_size_is_offered_in_order() {
        assert_eq!(font_size_choices(16), FONT_SIZES.to_vec());
        assert_eq!(&font_size_choices(15)[5..7], &[15, 16]);
    }
}
