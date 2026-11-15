//! The sizes an element's name can be given, as on a high level conceptual diagram or a
//! conceptual diagram's domains and boxes, and how tall a header is to fit one.

use crate::geometry::GRID;

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
