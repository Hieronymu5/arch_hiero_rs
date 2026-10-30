//! Points, sizes, and rectangles in canvas pixels, plus the rounding helpers that match the
//! TypeScript prototype's `Math.round` behaviour.

use serde::{Deserialize, Serialize};

/// Spacing between grid dots, in pixels. Elements snap to this grid.
pub const GRID: f64 = 20.0;

/// Size of the drawing area, in pixels.
pub const CANVAS_WIDTH: f64 = 2400.0;
pub const CANVAS_HEIGHT: f64 = 1600.0;

/// A position in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }
}

/// A size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// An area in pixels: its top-left corner and size.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_parts(position: Point, size: Size) -> Self {
        Self::new(position.x, position.y, size.width, size.height)
    }

    pub const fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub const fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Whether two areas overlap, rather than only touching.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }

    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    pub fn contains_rect(&self, inner: &Rect) -> bool {
        inner.x >= self.x
            && inner.y >= self.y
            && inner.right() <= self.right()
            && inner.bottom() <= self.bottom()
    }

    pub fn with_position(self, p: Point) -> Self {
        Self {
            x: p.x,
            y: p.y,
            ..self
        }
    }

    pub fn with_size(self, s: Size) -> Self {
        Self {
            width: s.width,
            height: s.height,
            ..self
        }
    }
}

/// JavaScript's `Math.round`: halves round towards positive infinity.
pub fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// Rounds to two decimal places, as the prototype does for exported coordinates.
pub fn round2(value: f64) -> f64 {
    let rounded = js_round(value * 100.0) / 100.0;
    // Avoid "-0" in output.
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// Rounds a value to the nearest grid line.
pub fn snap(value: f64) -> f64 {
    js_round(value / GRID) * GRID
}

/// The smallest of `first` and every value in `rest`, as `Math.min(first, ...rest)`.
pub fn min_of(first: f64, rest: impl IntoIterator<Item = f64>) -> f64 {
    rest.into_iter().fold(first, f64::min)
}

/// The largest of `first` and every value in `rest`, as `Math.max(first, ...rest)`.
pub fn max_of(first: f64, rest: impl IntoIterator<Item = f64>) -> f64 {
    rest.into_iter().fold(first, f64::max)
}

/// Formats a number as JavaScript's `String(number)` does for the values diagrams use:
/// integers without a decimal point, others with the shortest round-trip form.
pub fn fmt_num(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    format!("{value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_matches_math_round() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-2.6), -3.0);
        assert_eq!(js_round(0.49), 0.0);
    }

    #[test]
    fn snap_rounds_to_grid() {
        assert_eq!(snap(29.0), 20.0);
        assert_eq!(snap(30.0), 40.0);
        assert_eq!(snap(-10.0), 0.0);
    }

    #[test]
    fn numbers_format_like_javascript() {
        assert_eq!(fmt_num(10.0), "10");
        assert_eq!(fmt_num(-4.0), "-4");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(12.35), "12.35");
        assert_eq!(fmt_num(-0.0), "0");
    }

    #[test]
    fn overlap_excludes_touching() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(!a.overlaps(&Rect::new(10.0, 0.0, 10.0, 10.0)));
        assert!(a.overlaps(&Rect::new(9.0, 9.0, 10.0, 10.0)));
    }
}
