//! A key to what a diagram's lines and colours mean, with when it was exported and which
//! version it shows. The draw.io writer draws it as cells and [`Legend::add_to`] as shapes in
//! a scene, for the SVG export, both in the lower-right corner under the drawing.

use crate::geometry::{Point, Rect, fmt_num, round2};
use crate::scene::{Anchor, Marker, Node, Paint, Scene};
use crate::text::Weight;

const TEXT: &str = "#1b1f24";
const BACKGROUND: &str = "#ffffff";
/// How rounded the legend's card is.
const RADIUS: f64 = 12.0;

/// When an export was made and which version it shows, for its legend: such as
/// "4 Nov 2026" and "Version 3".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub generated: String,
    pub version: String,
}

impl Stamp {
    /// A stamp dated today, in the local time zone.
    pub fn today(version: impl Into<String>) -> Self {
        Self {
            generated: jiff::Zoned::now().strftime("%-d %b %Y").to_string(),
            version: version.into(),
        }
    }
}

/// A key to what a diagram's lines and colours mean, drawn in the lower-right corner under
/// the drawing: its lines on one row, its swatches in two columns, and a band at the bottom
/// with the date the export was made and the version it shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Legend {
    /// Lines, such as a dashed one for manual activity. At most two fit their row.
    pub lines: Vec<LegendLine>,
    /// Filled squares, such as the colour of domains.
    pub swatches: Vec<(String, String)>,
    pub stamp: Stamp,
}

/// A line in a legend, in the colour of the diagram's arrows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendLine {
    pub label: String,
    pub dashed: bool,
}

/// The legend's size and layout: its width, the bands' heights, the columns' width and the
/// rows' height, and the gap between the drawing and the legend.
pub const LEGEND_WIDTH: f64 = 260.0;
pub const LEGEND_HEADER: f64 = 34.0;
pub const LEGEND_FOOTER: f64 = 30.0;
pub const LEGEND_PADDING: f64 = 14.0;
pub const LEGEND_COLUMN: f64 = 122.0;
pub const LEGEND_ROW: f64 = 24.0;
pub const LEGEND_GAP: f64 = 40.0;
pub const LEGEND_BAND: &str = "#f3f4f6";
pub const LEGEND_BORDER: &str = "#9ca3af";
pub const LEGEND_MUTED: &str = "#6b7280";

impl Legend {
    /// The legend's height: the bands, a row of lines, and the swatches' rows.
    pub fn height(&self) -> f64 {
        let lines = if self.lines.is_empty() { 0.0 } else { 32.0 };
        let rows = self.swatches.len().div_ceil(2) as f64;
        LEGEND_HEADER + 12.0 + lines + rows * LEGEND_ROW - 10.0 + 12.0 + LEGEND_FOOTER
    }

    /// Adds the legend to `scene`, under its drawing and lined up with its right edge, and
    /// grows the scene's bounds to hold it.
    pub fn add_to(&self, scene: &mut Scene, line_color: &str, border: &str) {
        let b = scene.bounds;
        let at = Point::new(b.right() - LEGEND_WIDTH, b.bottom() + LEGEND_GAP);
        scene.nodes.extend(self.nodes(at, line_color, border));
        let left = b.x.min(at.x);
        scene.bounds = Rect::new(
            left,
            b.y,
            b.right().max(at.x + LEGEND_WIDTH) - left,
            at.y + self.height() - b.y,
        );
    }

    /// The legend's shapes, with its top-left corner at `at`, its lines drawn in `line_color`
    /// and its swatches' borders in `border`.
    pub fn nodes(&self, at: Point, line_color: &str, border: &str) -> Vec<Node> {
        let (x, y, w, h) = (at.x, at.y, LEGEND_WIDTH, self.height());
        let text = |at: Point, anchor: Anchor, size: f64, bold: bool, value: &str, color: &str| {
            Node::Text {
                at: Point::new(round2(at.x), round2(at.y)),
                anchor,
                size,
                weight: if bold { Weight::Bold } else { Weight::Regular },
                text: value.to_owned(),
                fill: Some(color.to_owned()),
                halo: None,
            }
        };
        // The bands fill the card's rounded corners, inside its border.
        let (left, right, r) = (x + 1.0, x + w - 1.0, RADIUS - 1.0);
        let n = |v: f64| fmt_num(round2(v));
        let (top, header) = (y + 1.0, y + LEGEND_HEADER);
        let (footer, bottom) = (y + h - LEGEND_FOOTER, y + h - 1.0);
        let header_band = format!(
            "M{} {} V{} A{r} {r} 0 0 1 {} {} H{} A{r} {r} 0 0 1 {} {} V{} Z",
            n(left),
            n(header),
            n(top + r),
            n(left + r),
            n(top),
            n(right - r),
            n(right),
            n(top + r),
            n(header),
            r = n(r),
        );
        let footer_band = format!(
            "M{} {} H{} V{} A{r} {r} 0 0 1 {} {} H{} A{r} {r} 0 0 1 {} {} Z",
            n(left),
            n(footer),
            n(right),
            n(bottom - r),
            n(right - r),
            n(bottom),
            n(left + r),
            n(left),
            n(bottom - r),
            r = n(r),
        );
        let mut nodes = vec![
            Node::rect(
                Rect::new(x, y, w, h),
                RADIUS,
                Paint::fill_stroke(BACKGROUND, LEGEND_BORDER, 1.0),
            ),
            Node::path(header_band, Paint::fill(LEGEND_BAND)),
            Node::path(footer_band, Paint::fill(LEGEND_BAND)),
            text(
                Point::new(x + w / 2.0, y + LEGEND_HEADER / 2.0 + 5.0),
                Anchor::Middle,
                14.0,
                true,
                "Legend",
                TEXT,
            ),
        ];
        let mut row = y + LEGEND_HEADER + 12.0;
        for (i, l) in self.lines.iter().enumerate() {
            let left = x + LEGEND_PADDING + i as f64 * LEGEND_COLUMN;
            let middle = row + 9.0;
            nodes.push(Node::Line {
                from: Point::new(left, middle),
                to: Point::new(left + 36.0, middle),
                paint: Paint::stroke(line_color, 2.0).dashed(l.dashed.then_some((8.0, 4.0))),
                marker: Some(Marker::Arrowhead),
            });
            nodes.push(text(
                Point::new(left + 44.0, middle + 4.0),
                Anchor::Start,
                11.0,
                false,
                &l.label,
                TEXT,
            ));
        }
        if !self.lines.is_empty() {
            row += 32.0;
        }
        for (i, (label, fill)) in self.swatches.iter().enumerate() {
            let left = x + LEGEND_PADDING + (i % 2) as f64 * LEGEND_COLUMN;
            let top = row + (i / 2) as f64 * LEGEND_ROW;
            nodes.push(Node::rect(
                Rect::new(left, top, 14.0, 14.0),
                0.0,
                Paint::fill_stroke(fill, border, 1.0),
            ));
            nodes.push(text(
                Point::new(left + 22.0, top + 11.0),
                Anchor::Start,
                11.0,
                false,
                label,
                TEXT,
            ));
        }
        let baseline = y + h - LEGEND_FOOTER / 2.0 + 3.5;
        nodes.push(text(
            Point::new(x + LEGEND_PADDING, baseline),
            Anchor::Start,
            10.0,
            false,
            &format!("Generated {}", self.stamp.generated),
            LEGEND_MUTED,
        ));
        nodes.push(text(
            Point::new(x + w - LEGEND_PADDING, baseline),
            Anchor::End,
            10.0,
            true,
            &self.stamp.version,
            TEXT,
        ));
        nodes
    }
}
