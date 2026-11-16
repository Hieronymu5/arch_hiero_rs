//! A key to what a diagram's lines and colours mean, with when it was exported and which
//! version it shows. The draw.io writer draws it as cells and [`Legend::nodes`] as shapes in a
//! scene, for the SVG export and for viewing, both to the right of the drawing, lined up with
//! its bottom (see [`Legend::beside`]).

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

/// A key to what a diagram's lines and colours mean, drawn beside the drawing: its lines on
/// one row, its swatches in one or two columns, any icons under a heading of their own, and a
/// band at the bottom with the date the export was made and the version it shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Legend {
    /// Lines, such as a dashed one for manual activity. At most two fit their row.
    pub lines: Vec<LegendLine>,
    /// Filled squares, such as the colour of domains.
    pub swatches: Vec<Swatch>,
    /// How many swatches go to a row: two, or one for longer labels.
    pub swatch_columns: usize,
    /// Pictures, two to a row under the swatches, such as a technology's logo.
    pub icons: Vec<LegendIcon>,
    /// A heading over the icons, such as "Technology Stack", set apart from the swatches.
    pub icons_title: Option<String>,
    /// How wide it is: [`LEGEND_WIDTH`], or wider for longer labels.
    pub width: f64,
    pub stamp: Stamp,
}

/// A line in a legend, in the colour of the diagram's arrows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendLine {
    pub label: String,
    pub dashed: bool,
}

/// A filled square in a legend, with what its colour means. It's bordered in its own `border`,
/// or else in a shade of its fill, as the elements are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Swatch {
    pub label: String,
    pub fill: String,
    pub border: Option<String>,
}

impl Swatch {
    /// A swatch bordered in a shade of its fill.
    pub fn new(label: impl Into<String>, fill: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            fill: fill.into(),
            border: None,
        }
    }

    /// The colour of its border.
    pub fn border(&self) -> String {
        self.border
            .clone()
            .unwrap_or_else(|| crate::color::border_for(&self.fill))
    }
}

/// A picture in a legend, such as a technology's logo, with its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendIcon {
    pub label: String,
    pub png: &'static [u8],
}

/// The legend's size and layout: its usual width, the bands' heights, the columns' width and
/// the rows' height, and the usual gap between the drawing and the legend.
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
/// The size of an icon, such as a technology's logo.
pub const LEGEND_ICON: f64 = 16.0;
/// The gap between the swatches and the icons' heading, and the heading's height.
const SECTION_GAP: f64 = 8.0;
const SECTION_TITLE: f64 = 22.0;

/// Where each part of a legend goes, with its top-left corner at a given point.
pub(crate) struct LegendLayout {
    pub width: f64,
    pub height: f64,
    /// How wide each of its two columns is.
    pub column: f64,
    /// Each line's left edge and top.
    pub lines: Vec<Point>,
    /// Each swatch's top-left corner.
    pub swatches: Vec<Point>,
    /// The icons' heading's left edge and top, when there's one.
    pub icons_title: Option<Point>,
    /// Each icon's top-left corner.
    pub icons: Vec<Point>,
}

impl Legend {
    /// How many rows the swatches take.
    fn swatch_rows(&self) -> usize {
        self.swatches.len().div_ceil(self.swatch_columns.max(1))
    }

    /// How tall the icons' heading is, with the gap above it, when there are icons under one.
    fn icons_title_height(&self) -> f64 {
        if self.icons_title.is_some() && !self.icons.is_empty() {
            SECTION_GAP + SECTION_TITLE
        } else {
            0.0
        }
    }

    /// The legend's height: the bands, a row of lines, the swatches' rows, and the icons'
    /// rows, which start on a row of their own, under their heading if they have one.
    pub fn height(&self) -> f64 {
        let lines = if self.lines.is_empty() { 0.0 } else { 32.0 };
        let rows = (self.swatch_rows() + self.icons.len().div_ceil(2)) as f64;
        LEGEND_HEADER + 12.0 + lines + rows * LEGEND_ROW + self.icons_title_height() - 10.0
            + 12.0
            + LEGEND_FOOTER
    }

    /// Where its top-left corner goes beside a drawing that covers `drawing`: `gap` to the
    /// right of it, with the legend's bottom level with the drawing's, or level with the
    /// drawing's top when the legend is the taller.
    pub fn beside(&self, drawing: Rect, gap: f64) -> Point {
        Point::new(
            drawing.right() + gap,
            drawing.y.max(drawing.bottom() - self.height()),
        )
    }

    /// Where each part goes, with its top-left corner at `at`.
    pub(crate) fn layout(&self, at: Point) -> LegendLayout {
        let (x, y) = (at.x, at.y);
        // Each column is wider by half of any extra width.
        let column = LEGEND_COLUMN + (self.width - LEGEND_WIDTH) / 2.0;
        let line_top = y + LEGEND_HEADER + 12.0;
        let swatch_top = line_top + if self.lines.is_empty() { 0.0 } else { 32.0 };
        let columns = self.swatch_columns.max(1);
        let below_swatches = swatch_top + self.swatch_rows() as f64 * LEGEND_ROW;
        let icon_top = below_swatches + self.icons_title_height();
        LegendLayout {
            width: self.width,
            height: self.height(),
            column,
            lines: (0..self.lines.len())
                .map(|i| Point::new(x + LEGEND_PADDING + i as f64 * column, line_top))
                .collect(),
            swatches: (0..self.swatches.len())
                .map(|i| {
                    Point::new(
                        x + LEGEND_PADDING + (i % columns) as f64 * column,
                        swatch_top + (i / columns) as f64 * LEGEND_ROW,
                    )
                })
                .collect(),
            icons_title: (self.icons_title_height() > 0.0)
                .then(|| Point::new(x + LEGEND_PADDING, below_swatches + SECTION_GAP)),
            // Centred on where a swatch would be, a pixel bigger each way.
            icons: (0..self.icons.len())
                .map(|i| {
                    Point::new(
                        x + LEGEND_PADDING + (i % 2) as f64 * column - 1.0,
                        icon_top + (i / 2) as f64 * LEGEND_ROW - 1.0,
                    )
                })
                .collect(),
        }
    }

    /// What it explains, in order, as one description, such as for a screen reader.
    pub fn description(&self) -> String {
        let items: Vec<String> = self
            .lines
            .iter()
            .map(|l| format!("{} line", l.label))
            .chain(self.swatches.iter().map(|s| s.label.clone()))
            .collect();
        let icons: Vec<&str> = self.icons.iter().map(|i| i.label.as_str()).collect();
        // Under their heading, when they have one.
        let icons = match (&self.icons_title, icons.is_empty()) {
            (_, true) => String::new(),
            (Some(title), false) => format!(". {title}: {}", icons.join(", ")),
            (None, false) => format!(", {}", icons.join(", ")),
        };
        format!(
            "Legend: {}{icons}. Generated {}, {}.",
            items.join(", "),
            self.stamp.generated,
            self.stamp.version
        )
    }

    /// The legend's shapes, with its top-left corner at `at`, its lines drawn in `line_color`
    /// and its own border `border_width` wide, as the diagram's elements' lines are.
    pub fn nodes(&self, at: Point, line_color: &str, border_width: f64) -> Vec<Node> {
        let layout = self.layout(at);
        let (x, y, w, h) = (at.x, at.y, layout.width, layout.height);
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
            // The card's fill, then its bands, then its border over them, so a thick border
            // isn't half hidden under the bands.
            Node::rect(Rect::new(x, y, w, h), RADIUS, Paint::fill(BACKGROUND)),
            Node::path(header_band, Paint::fill(LEGEND_BAND)),
            Node::path(footer_band, Paint::fill(LEGEND_BAND)),
            Node::rect(
                Rect::new(x, y, w, h),
                RADIUS,
                Paint::fill_stroke("none", LEGEND_BORDER, border_width),
            ),
            text(
                Point::new(x + w / 2.0, y + LEGEND_HEADER / 2.0 + 5.0),
                Anchor::Middle,
                14.0,
                true,
                "Legend",
                TEXT,
            ),
        ];
        for (l, at) in self.lines.iter().zip(&layout.lines) {
            let middle = at.y + 9.0;
            nodes.push(Node::Line {
                from: Point::new(at.x, middle),
                to: Point::new(at.x + 36.0, middle),
                paint: Paint::stroke(line_color, 2.0).dashed(l.dashed.then_some((8.0, 4.0))),
                marker: Some(Marker::Arrowhead),
            });
            nodes.push(text(
                Point::new(at.x + 44.0, middle + 4.0),
                Anchor::Start,
                11.0,
                false,
                &l.label,
                TEXT,
            ));
        }
        for (s, at) in self.swatches.iter().zip(&layout.swatches) {
            nodes.push(Node::rect(
                Rect::new(at.x, at.y, 14.0, 14.0),
                0.0,
                Paint::fill_stroke(&s.fill, s.border(), 1.0),
            ));
            nodes.push(text(
                Point::new(at.x + 22.0, at.y + 11.0),
                Anchor::Start,
                11.0,
                false,
                &s.label,
                TEXT,
            ));
        }
        if let (Some(title), Some(at)) = (&self.icons_title, layout.icons_title) {
            nodes.push(text(
                Point::new(at.x, at.y + 15.0),
                Anchor::Start,
                12.0,
                true,
                title,
                TEXT,
            ));
        }
        for (icon, at) in self.icons.iter().zip(&layout.icons) {
            nodes.push(Node::Image {
                rect: Rect::new(at.x, at.y, LEGEND_ICON, LEGEND_ICON),
                png: icon.png,
            });
            nodes.push(text(
                Point::new(at.x + 23.0, at.y + 12.0),
                Anchor::Start,
                11.0,
                false,
                &icon.label,
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

/// A legend and how it goes beside its diagram's drawing: how far to its right, the colour of
/// the diagram's arrows for its lines, and how wide the diagram's lines are, for its border.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendBeside {
    pub legend: Legend,
    pub gap: f64,
    pub line_color: String,
    pub border_width: f64,
}

impl LegendBeside {
    /// Adds the legend to `scene`, beside its drawing, and grows the scene's bounds to hold
    /// it, the half of its border outside its edge too.
    pub fn add_to(&self, scene: &mut Scene) {
        let b = scene.bounds;
        let at = self.legend.beside(b, self.gap);
        scene
            .nodes
            .extend(self.legend.nodes(at, &self.line_color, self.border_width));
        let bleed = self.border_width / 2.0;
        let (right, bottom) = (
            at.x + self.legend.width + bleed,
            at.y + self.legend.height() + bleed,
        );
        let (left, top) = (b.x.min(at.x - bleed), b.y.min(at.y - bleed));
        scene.bounds = Rect::new(
            left,
            top,
            b.right().max(right) - left,
            b.bottom().max(bottom) - top,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> Stamp {
        Stamp {
            generated: "4 Nov 2026".into(),
            version: "Version 3".into(),
        }
    }

    fn plain(swatches: usize) -> Legend {
        Legend {
            lines: Vec::new(),
            swatches: (0..swatches)
                .map(|i| Swatch::new(format!("S{i}"), "#dae8fc"))
                .collect(),
            swatch_columns: 2,
            icons: Vec::new(),
            icons_title: None,
            width: LEGEND_WIDTH,
            stamp: stamp(),
        }
    }

    #[test]
    fn it_goes_beside_the_drawing_lined_up_with_its_bottom() {
        let legend = plain(2);
        let h = legend.height();
        let drawing = Rect::new(20.0, 40.0, 500.0, 400.0);
        assert_eq!(legend.beside(drawing, 40.0), Point::new(560.0, 440.0 - h));
        // Taller than the drawing, it starts level with the drawing's top.
        let short = Rect::new(20.0, 40.0, 500.0, 20.0);
        assert_eq!(legend.beside(short, 20.0), Point::new(540.0, 40.0));
    }

    #[test]
    fn icons_go_under_their_heading_after_the_swatches_one_to_a_row() {
        let mut legend = plain(3);
        legend.swatch_columns = 1;
        legend.width = 320.0;
        let without = legend.height();
        legend.icons = vec![
            LegendIcon {
                label: "Rust".into(),
                png: &[],
            };
            3
        ];
        legend.icons_title = Some("Technology Stack".into());
        // Two rows of icons and the heading.
        assert_eq!(
            legend.height(),
            without + 2.0 * LEGEND_ROW + SECTION_GAP + SECTION_TITLE
        );
        let layout = legend.layout(Point::new(0.0, 0.0));
        // One swatch to a row, and two icons, in columns half the extra width wider.
        assert_eq!(layout.swatches[1].x, layout.swatches[0].x);
        assert_eq!(layout.column, LEGEND_COLUMN + 30.0);
        assert_eq!(layout.icons[1].x - layout.icons[0].x, LEGEND_COLUMN + 30.0);
        assert!(layout.icons_title.unwrap().y > layout.swatches[2].y);
        assert_eq!(
            legend.description(),
            "Legend: S0, S1, S2. Technology Stack: Rust, Rust, Rust. Generated 4 Nov 2026, Version 3."
        );
    }

    #[test]
    fn its_border_is_as_wide_as_the_lines_and_the_scene_holds_it() {
        let placed = LegendBeside {
            legend: plain(2),
            gap: 40.0,
            line_color: "#000000".into(),
            border_width: 3.0,
        };
        let mut scene = Scene {
            nodes: Vec::new(),
            bounds: Rect::new(0.0, 0.0, 400.0, 300.0),
            text_color: TEXT.into(),
            marker_color: TEXT.into(),
            background: BACKGROUND.into(),
        };
        placed.add_to(&mut scene);
        let border = scene.nodes.iter().find_map(|n| match n {
            Node::Rect { paint, .. } if paint.stroke.as_deref() == Some(LEGEND_BORDER) => {
                paint.stroke_width
            }
            _ => None,
        });
        assert_eq!(border, Some(3.0));
        assert_eq!(scene.bounds.right(), 400.0 + 40.0 + LEGEND_WIDTH + 1.5);
        assert_eq!(scene.bounds.bottom(), 301.5);
    }
}
