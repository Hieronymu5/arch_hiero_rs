//! The flowchart's shapes (ISO 5807): a pill for a start or an end, a box for a process, a
//! diamond for a decision, a parallelogram for input or output, a box with doubled sides for a
//! subprocess, a page with a wavy bottom for a document, a cylinder for a data store, a
//! trapezoid for a manual step, a D for a delay, a circle for a connector, and a page with a
//! folded corner for a note. Each shows its name inside it.
//!
//! The scene and the draw.io file both come from here, so the editor, the SVG export, and
//! draw.io agree.

use crate::export::drawio::{Style, cell, rect_attributes};
use crate::export::svg::escape_xml;
use crate::geometry::{Point, Rect, fmt_num, round2};
use crate::look::{LINK_MARK_SIZE, LookElement, PADDING_X, TEXT, text_lines};
use crate::model::LineWeight;
use crate::routing::Side;
use crate::scene::{Anchor, Node, Paint};
use crate::text::{MeasureText, TextStyle, Weight, wrap_text};

/// What a flowchart element is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowShape {
    /// A pill, with fully round ends: a start or an end.
    Terminal,
    /// A box with slightly round corners: a process.
    Process,
    /// A diamond: a decision.
    Decision,
    /// A parallelogram leaning right: input or output.
    InputOutput,
    /// A box with a second line inside each side: a subprocess.
    Subprocess,
    /// A page with a wavy bottom: a document.
    Document,
    /// A cylinder: a data store.
    DataStore,
    /// A trapezoid, wide at the top: a manual step.
    Manual,
    /// A D, flat on its left and round on its right: a delay.
    Delay,
    /// A circle: a connector to another place in the flow.
    Connector,
    /// A page with its top-right corner folded: a note.
    Note,
}

/// A step's name, inside its shape.
pub const NAME: TextStyle = TextStyle::new(14.0, Weight::SemiBold, 18.0, 3);
/// A note's text, from its top-left corner.
pub const NOTE_TEXT: TextStyle = TextStyle::new(13.0, Weight::Regular, 17.0, 8);
/// A connector's label, in its circle.
pub const CONNECTOR_TEXT: TextStyle = TextStyle::new(14.0, Weight::Bold, 18.0, 1);
/// A process's corners.
pub const PROCESS_RADIUS: f64 = 4.0;
/// How far a subprocess's inner lines are from its sides.
pub const SUBPROCESS_INSET: f64 = 12.0;
/// How far a parallelogram or trapezoid slants, at most.
pub const SLANT: f64 = 20.0;
/// How deep a document's wave is, at most.
pub const WAVE: f64 = 10.0;
/// How tall the ellipses at a data store's top and bottom are, at most (half their height).
pub const CYLINDER_RY: f64 = 10.0;
/// How far a mark set off a shape's corner is from it, each way.
pub const LINK_MARK_CLEAR: f64 = 2.0;
/// A note's folded corner, at most.
pub const NOTE_FOLD: f64 = 14.0;

fn f(v: f64) -> String {
    fmt_num(round2(v))
}

/// How far a parallelogram or trapezoid in `rect` slants.
pub fn slant(rect: Rect) -> f64 {
    SLANT.min(rect.width / 4.0).max(0.0)
}

/// How deep a document's wave in `rect` is.
pub fn wave(rect: Rect) -> f64 {
    WAVE.min(rect.height / 8.0).max(0.0)
}

/// Half the height of a data store's ellipses in `rect`.
pub fn cylinder_ry(rect: Rect) -> f64 {
    CYLINDER_RY.min(rect.height / 6.0).max(0.0)
}

/// The radius of a pill's ends, or of a delay's round side, in `rect`.
fn end_radius(rect: Rect) -> f64 {
    (rect.height / 2.0).min(rect.width / 2.0).max(0.0)
}

/// A note's folded corner in `rect`.
pub fn note_fold(rect: Rect) -> f64 {
    NOTE_FOLD
        .min(rect.width / 4.0)
        .min(rect.height / 4.0)
        .max(0.0)
}

/// How far in from the edge of a round end `r` across a point `offset` from its middle is.
fn round_depth(r: f64, offset: f64) -> f64 {
    if r <= 0.0 {
        return 0.0;
    }
    let t = (offset / r).clamp(-1.0, 1.0);
    r * (1.0 - (1.0 - t * t).sqrt())
}

/// Where the mark that says a step drawn in `rect` opens a diagram goes (its top-left
/// corner): off the top-right corner, up and to the right, on every shape alike, so it never
/// touches a slant, a curve, or a subprocess's inner line.
pub fn link_mark_at(rect: Rect) -> Point {
    Point::new(
        rect.right() + LINK_MARK_CLEAR,
        rect.y - LINK_MARK_SIZE - LINK_MARK_CLEAR,
    )
}

impl FlowShape {
    /// The part of `rect` its text is centred in (for a note, starts in), clear of its
    /// outline.
    pub fn text_area(self, rect: Rect) -> Rect {
        let (left, top, right, bottom) = match self {
            Self::Terminal => {
                let r = end_radius(rect) / 2.0;
                (r, 0.0, r, 0.0)
            }
            Self::Process => (0.0, 0.0, 0.0, 0.0),
            // The middle of the diamond, where a line or two fits between its slants.
            Self::Decision => (
                rect.width * 0.2 - PADDING_X,
                rect.height * 0.2,
                rect.width * 0.2 - PADDING_X,
                rect.height * 0.2,
            ),
            Self::InputOutput | Self::Manual => {
                let s = slant(rect);
                (s, 0.0, s, 0.0)
            }
            Self::Subprocess => (SUBPROCESS_INSET, 0.0, SUBPROCESS_INSET, 0.0),
            Self::Document => (0.0, 0.0, 0.0, wave(rect) * 2.0),
            Self::DataStore => {
                let ry = cylinder_ry(rect);
                (0.0, ry * 2.0, 0.0, ry)
            }
            Self::Delay => (0.0, 0.0, end_radius(rect) / 2.0, 0.0),
            Self::Connector => (-PADDING_X + 2.0, 0.0, -PADDING_X + 2.0, 0.0),
            Self::Note => (-2.0, 4.0, note_fold(rect) - PADDING_X + 4.0, 4.0),
        };
        Rect::new(
            rect.x + PADDING_X + left,
            rect.y + 4.0 + top,
            (rect.width - PADDING_X * 2.0 - left - right).max(0.0),
            (rect.height - 8.0 - top - bottom).max(0.0),
        )
    }

    /// Where an arrow ending at `point`, on `side` of the shape filling `rect`, meets its
    /// outline: the point moved straight in until it reaches it.
    pub fn edge_point(self, rect: Rect, side: Side, point: Point) -> Point {
        let middle = rect.center();
        let inward = |depth: f64| match side {
            Side::Left => Point::new(rect.x + depth, point.y),
            Side::Right => Point::new(rect.right() - depth, point.y),
            Side::Top => Point::new(point.x, rect.y + depth),
            Side::Bottom => Point::new(point.x, rect.bottom() - depth),
        };
        // How far the point is from the nearer end of its side.
        let from_end = if side.is_vertical() {
            (point.y - rect.y).min(rect.bottom() - point.y)
        } else {
            (point.x - rect.x).min(rect.right() - point.x)
        }
        .max(0.0);
        let depth = match self {
            Self::Process | Self::Subprocess | Self::Note | Self::Document => 0.0,
            Self::Decision => {
                let (half, across) = if side.is_vertical() {
                    (rect.height / 2.0, rect.width / 2.0)
                } else {
                    (rect.width / 2.0, rect.height / 2.0)
                };
                if half <= 0.0 {
                    0.0
                } else {
                    across * (1.0 - from_end / half).clamp(0.0, 1.0)
                }
            }
            Self::Terminal => {
                let r = end_radius(rect);
                if side.is_vertical() {
                    round_depth(r, point.y - middle.y)
                } else if from_end < r {
                    round_depth(r, r - from_end)
                } else {
                    0.0
                }
            }
            Self::Connector => {
                let (rx, ry) = (rect.width / 2.0, rect.height / 2.0);
                if side.is_vertical() {
                    round_depth(ry, point.y - middle.y) * rx / ry.max(f64::EPSILON)
                } else {
                    round_depth(rx, point.x - middle.x) * ry / rx.max(f64::EPSILON)
                }
            }
            Self::Delay => {
                let r = end_radius(rect);
                match side {
                    Side::Left => 0.0,
                    Side::Right => round_depth(r, point.y - middle.y),
                    Side::Top | Side::Bottom => {
                        let from_right = (rect.right() - point.x).max(0.0);
                        if from_right < r {
                            round_depth(r, r - from_right)
                        } else {
                            0.0
                        }
                    }
                }
            }
            Self::InputOutput => {
                let s = slant(rect);
                let down = if rect.height > 0.0 {
                    (point.y - rect.y) / rect.height
                } else {
                    0.0
                };
                match side {
                    Side::Left => s * (1.0 - down),
                    Side::Right => s * down,
                    _ => 0.0,
                }
            }
            Self::Manual => {
                let s = slant(rect);
                let down = if rect.height > 0.0 {
                    (point.y - rect.y) / rect.height
                } else {
                    0.0
                };
                if side.is_vertical() { s * down } else { 0.0 }
            }
            Self::DataStore => {
                let ry = cylinder_ry(rect);
                let rx = rect.width / 2.0;
                match side {
                    // On the top ellipse's near half, or the bottom one's far half.
                    Side::Top | Side::Bottom => {
                        ry - ry * (1.0 - ((point.x - middle.x) / rx).powi(2)).max(0.0).sqrt()
                    }
                    _ => 0.0,
                }
            }
        };
        let p = inward(depth);
        Point::new(round2(p.x), round2(p.y))
    }

    /// The outline of the shape filling `rect`, as an SVG path.
    pub fn outline(self, rect: Rect) -> String {
        let Rect {
            x,
            y,
            width,
            height,
        } = rect;
        let (right, bottom) = (rect.right(), rect.bottom());
        let (cx, cy) = (x + width / 2.0, y + height / 2.0);
        match self {
            Self::Terminal => {
                let r = end_radius(rect);
                format!(
                    "M{} {}H{}A{r} {r} 0 0 1 {} {}H{}A{r} {r} 0 0 1 {} {}Z",
                    f(x + r),
                    f(y),
                    f(right - r),
                    f(right - r),
                    f(bottom),
                    f(x + r),
                    f(x + r),
                    f(y),
                    r = f(r)
                )
            }
            Self::Process | Self::Subprocess => {
                format!("M{} {}H{}V{}H{}Z", f(x), f(y), f(right), f(bottom), f(x))
            }
            Self::Decision => format!(
                "M{} {}L{} {}L{} {}L{} {}Z",
                f(cx),
                f(y),
                f(right),
                f(cy),
                f(cx),
                f(bottom),
                f(x),
                f(cy)
            ),
            Self::InputOutput => {
                let s = slant(rect);
                format!(
                    "M{} {}H{}L{} {}H{}Z",
                    f(x + s),
                    f(y),
                    f(right),
                    f(right - s),
                    f(bottom),
                    f(x)
                )
            }
            Self::Manual => {
                let s = slant(rect);
                format!(
                    "M{} {}H{}L{} {}H{}Z",
                    f(x),
                    f(y),
                    f(right),
                    f(right - s),
                    f(bottom),
                    f(x + s)
                )
            }
            Self::Document => {
                // A sheet whose bottom edge is one S-shaped wave: up near the right, down
                // to the bottom near the left. A cubic peaks at about 0.29 of its pull, so
                // pulling by 3.5 waves takes it one wave either side of the ends. (The
                // editor's path parser has no `T`, so this is a single `C`.)
                let a = wave(rect);
                let mid = bottom - a;
                let pull = a * 3.5;
                format!(
                    "M{} {}H{}V{}C{} {} {} {} {} {}Z",
                    f(x),
                    f(y),
                    f(right),
                    f(mid),
                    f(right - width / 3.0),
                    f(mid - pull),
                    f(x + width / 3.0),
                    f(mid + pull),
                    f(x),
                    f(mid)
                )
            }
            Self::DataStore => {
                let (rx, ry) = (width / 2.0, cylinder_ry(rect));
                format!(
                    "M{} {}A{} {} 0 0 1 {} {}V{}A{} {} 0 0 1 {} {}Z",
                    f(x),
                    f(y + ry),
                    f(rx),
                    f(ry),
                    f(right),
                    f(y + ry),
                    f(bottom - ry),
                    f(rx),
                    f(ry),
                    f(x),
                    f(bottom - ry)
                )
            }
            Self::Delay => {
                let r = end_radius(rect);
                format!(
                    "M{} {}H{}A{} {} 0 0 1 {} {}H{}Z",
                    f(x),
                    f(y),
                    f(right - r),
                    f(r),
                    f(height / 2.0),
                    f(right - r),
                    f(bottom),
                    f(x)
                )
            }
            Self::Connector => {
                let (rx, ry) = (width / 2.0, height / 2.0);
                format!(
                    "M{} {}A{} {} 0 0 1 {} {}A{} {} 0 0 1 {} {}Z",
                    f(x),
                    f(cy),
                    f(rx),
                    f(ry),
                    f(right),
                    f(cy),
                    f(rx),
                    f(ry),
                    f(x),
                    f(cy)
                )
            }
            Self::Note => {
                let fold = note_fold(rect);
                format!(
                    "M{} {}H{}L{} {}V{}H{}Z",
                    f(x),
                    f(y),
                    f(right - fold),
                    f(right),
                    f(y + fold),
                    f(bottom),
                    f(x)
                )
            }
        }
    }

    /// Lines drawn over the outline: a subprocess's inner sides, a data store's top's front,
    /// and a note's fold.
    fn details(self, rect: Rect) -> Option<String> {
        let Rect { x, y, width, .. } = rect;
        let (right, bottom) = (rect.right(), rect.bottom());
        match self {
            Self::Subprocess => {
                let inset = SUBPROCESS_INSET.min(width / 4.0);
                Some(format!(
                    "M{} {}V{}M{} {}V{}",
                    f(x + inset),
                    f(y),
                    f(bottom),
                    f(right - inset),
                    f(y),
                    f(bottom)
                ))
            }
            Self::DataStore => {
                let (rx, ry) = (width / 2.0, cylinder_ry(rect));
                Some(format!(
                    "M{} {}A{} {} 0 0 0 {} {}",
                    f(x),
                    f(y + ry),
                    f(rx),
                    f(ry),
                    f(right),
                    f(y + ry)
                ))
            }
            Self::Note => {
                let fold = note_fold(rect);
                Some(format!(
                    "M{} {}V{}H{}",
                    f(right - fold),
                    f(y),
                    f(y + fold),
                    f(right)
                ))
            }
            _ => None,
        }
    }

    /// The style of the element's name.
    const fn text_style(self) -> TextStyle {
        match self {
            Self::Note => NOTE_TEXT,
            Self::Connector => CONNECTOR_TEXT,
            _ => NAME,
        }
    }
}

/// The nodes that draw a flowchart element, with lines as thick as `weight` (or `bold`, for
/// an end). `border` is used when the element sets no border colour.
pub fn draw(
    element: &LookElement,
    shape: FlowShape,
    measure: &dyn MeasureText,
    border: &str,
    weight: LineWeight,
) -> Vec<Node> {
    let rect = element.rect;
    let look = &element.look;
    let border = look
        .border_color
        .clone()
        .unwrap_or_else(|| border.to_owned());
    let fill = look.fill.clone().unwrap_or_else(|| "#ffffff".into());
    // A bold border marks an end.
    let line = weight.width() * if look.bold_border { 2.0 } else { 1.0 };
    let mut nodes = vec![Node::path(
        shape.outline(rect),
        Paint::fill_stroke(fill, border.clone(), line),
    )];
    if let Some(d) = shape.details(rect) {
        nodes.push(Node::path(
            d,
            Paint {
                fill: Some("none".into()),
                ..Paint::stroke(border, line)
            },
        ));
    }
    let area = shape.text_area(rect);
    let style = shape.text_style();
    let lines = (area.height / style.line_height).floor().max(1.0) as usize;
    let name = wrap_text(
        &element.label,
        area.width,
        style.with_max_lines(lines.min(style.max_lines)),
        measure,
    );
    let text = if shape == FlowShape::Note {
        text_lines(&name, area.x, area.y, style, Anchor::Start)
    } else {
        let height = name.len() as f64 * style.line_height;
        text_lines(
            &name,
            area.x + area.width / 2.0,
            area.y + ((area.height - height) / 2.0).max(0.0),
            style,
            Anchor::Middle,
        )
    };
    if !text.is_empty() {
        nodes.push(match &look.text_color {
            Some(color) => Node::group(None, Paint::fill(color.clone()), text),
            None => Node::group(None, Paint::default(), text),
        });
    }
    nodes
}

/// The draw.io cell for a flowchart element at `left`, `top` in `parent`.
pub fn drawio_cell(
    element: &LookElement,
    shape: FlowShape,
    (left, top): (f64, f64),
    parent: &str,
    border: &str,
    weight: LineWeight,
) -> String {
    let look = &element.look;
    let rect = element.rect;
    let border = look.border_color.as_deref().unwrap_or(border);
    let style = shape.text_style();
    let line = weight.width() * if look.bold_border { 2.0 } else { 1.0 };
    let base = Style::new()
        .set("html", 1)
        .set("whiteSpace", "wrap")
        .set("strokeColor", border)
        .num("strokeWidth", line)
        .set("fillColor", look.fill.as_deref().unwrap_or("#ffffff"))
        .set("fontColor", look.text_color.as_deref().unwrap_or(TEXT))
        .num("fontSize", style.size)
        .set("fontStyle", u8::from(style.weight != Weight::Regular));
    let style = match shape {
        FlowShape::Terminal => base.set("rounded", 1).set("arcSize", 50),
        FlowShape::Process => base
            .set("rounded", 1)
            .set("absoluteArcSize", 1)
            .num("arcSize", PROCESS_RADIUS * 2.0),
        FlowShape::Decision => base.set("rhombus", 1),
        FlowShape::InputOutput => base
            .set("shape", "parallelogram")
            .set("perimeter", "parallelogramPerimeter")
            .set("fixedSize", 1)
            .num("size", slant(rect)),
        FlowShape::Subprocess => base
            .set("shape", "process")
            .set("backgroundOutline", 1)
            .set("fixedSize", 1)
            .num("size", SUBPROCESS_INSET.min(rect.width / 4.0)),
        FlowShape::Document => base.set("shape", "document").set("boundedLbl", 1).num(
            "size",
            round2(wave(rect) * 2.0 / rect.height.max(f64::EPSILON)),
        ),
        FlowShape::DataStore => base
            .set("shape", "cylinder3")
            .set("boundedLbl", 1)
            .set("backgroundOutline", 1)
            .num("size", cylinder_ry(rect)),
        // draw.io's trapezoid is wide at the bottom: flipped, its text stays upright.
        FlowShape::Manual => base
            .set("shape", "trapezoid")
            .set("perimeter", "trapezoidPerimeter")
            .set("fixedSize", 1)
            .num("size", slant(rect))
            .set("flipV", 1),
        FlowShape::Delay => base.set("shape", "delay"),
        FlowShape::Connector => base.set("ellipse", 1),
        FlowShape::Note => base
            .set("shape", "note")
            .set("backgroundOutline", 1)
            .num("size", note_fold(rect))
            .set("align", "left")
            .set("verticalAlign", "top")
            .num("spacingLeft", PADDING_X - 4.0)
            .num("spacingTop", 4.0),
    };
    cell(
        element.id.as_str(),
        &escape_xml(&escape_xml(&element.label)),
        &style.build(),
        parent,
        &rect_attributes(Rect::new(left, top, rect.width, rect.height)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{Look, Shape};
    use crate::text::EstimateText;

    const RECT: Rect = Rect::new(100.0, 100.0, 160.0, 100.0);

    #[test]
    fn the_link_mark_sits_off_every_shapes_corner() {
        // Every outline stays inside its box, so a mark clear of the box's top-right corner
        // is clear of every shape. Its lines run 1.5 px in from its 12 px box.
        let at = link_mark_at(RECT);
        assert!(at.x + 1.5 > RECT.right() + 3.0);
        assert!(at.y + LINK_MARK_SIZE - 1.5 < RECT.y - 3.0);
    }

    #[test]
    fn outlines_use_only_what_the_editor_draws() {
        // The editor's path parser skips commands it doesn't know, so a shape using one
        // would look broken in the app while its SVG looked fine.
        use FlowShape::*;
        for shape in [
            Terminal,
            Process,
            Decision,
            InputOutput,
            Subprocess,
            Document,
            DataStore,
            Manual,
            Delay,
            Connector,
            Note,
        ] {
            let d = shape.outline(RECT);
            assert!(
                d.chars()
                    .filter(char::is_ascii_alphabetic)
                    .all(|c| "MLHVCQAZ".contains(c)),
                "{shape:?}: {d}"
            );
        }
    }

    #[test]
    fn a_documents_wave_stays_in_its_box() {
        let d = FlowShape::Document.outline(RECT);
        let segs = crate::scene::parse_path(&d);
        assert!(
            segs.iter()
                .any(|s| matches!(s, crate::scene::Seg::Cubic(..)))
        );
        // One wave (10 px) either side of a line a wave above the bottom: the pull is
        // 3.5 waves, and a cubic reaches about 0.29 of it.
        assert!(d.contains("V190C"), "{d}");
        assert!(d.contains("206.67 155 153.33 225 100 190Z"), "{d}");
    }

    #[test]
    fn arrows_end_on_a_diamonds_slants() {
        let d = FlowShape::Decision;
        // Its points stay where they are.
        assert_eq!(
            d.edge_point(RECT, Side::Left, Point::new(100.0, 150.0)),
            Point::new(100.0, 150.0)
        );
        assert_eq!(
            d.edge_point(RECT, Side::Top, Point::new(180.0, 100.0)),
            Point::new(180.0, 100.0)
        );
        // Halfway up its left side, it's halfway in.
        assert_eq!(
            d.edge_point(RECT, Side::Left, Point::new(100.0, 125.0)),
            Point::new(140.0, 125.0)
        );
        assert_eq!(
            d.edge_point(RECT, Side::Bottom, Point::new(140.0, 200.0)),
            Point::new(140.0, 175.0)
        );
    }

    #[test]
    fn arrows_end_on_round_and_slanted_outlines() {
        let pill = Rect::new(0.0, 0.0, 160.0, 60.0);
        // The middle of a pill's end is on its box; nearer its top it's further in.
        assert_eq!(
            FlowShape::Terminal.edge_point(pill, Side::Left, Point::new(0.0, 30.0)),
            Point::new(0.0, 30.0)
        );
        assert!(
            FlowShape::Terminal
                .edge_point(pill, Side::Left, Point::new(0.0, 10.0))
                .x
                > 5.0
        );
        // Its flat top is its box.
        assert_eq!(
            FlowShape::Terminal.edge_point(pill, Side::Top, Point::new(80.0, 0.0)),
            Point::new(80.0, 0.0)
        );
        // A parallelogram's left side leans in at the top, its right side at the bottom.
        let io = FlowShape::InputOutput;
        assert_eq!(
            io.edge_point(RECT, Side::Left, Point::new(100.0, 150.0)),
            Point::new(110.0, 150.0)
        );
        assert_eq!(
            io.edge_point(RECT, Side::Right, Point::new(260.0, 150.0)),
            Point::new(250.0, 150.0)
        );
        // A data store's top is its ellipse.
        let top = FlowShape::DataStore.edge_point(RECT, Side::Top, Point::new(110.0, 100.0));
        assert!(top.y > 100.0 && top.y < 110.0);
        assert_eq!(
            FlowShape::DataStore.edge_point(RECT, Side::Top, Point::new(180.0, 100.0)),
            Point::new(180.0, 100.0)
        );
        // A delay's left side is flat, its right round.
        assert_eq!(
            FlowShape::Delay.edge_point(RECT, Side::Left, Point::new(100.0, 110.0)),
            Point::new(100.0, 110.0)
        );
        assert!(
            FlowShape::Delay
                .edge_point(RECT, Side::Right, Point::new(260.0, 110.0))
                .x
                < 250.0
        );
    }

    fn element(shape: FlowShape) -> LookElement {
        LookElement {
            id: "e".into(),
            parent_id: None,
            label: "Check stock".into(),
            description: None,
            look: Look {
                fill: Some("#dae8fc".into()),
                border_color: Some("#2d5f9e".into()),
                ..Look::new(Shape::Flow(shape))
            },
            rect: RECT,
        }
    }

    #[test]
    fn draws_each_shape_with_its_name_inside() {
        static ESTIMATE: EstimateText = EstimateText;
        for shape in [
            FlowShape::Terminal,
            FlowShape::Process,
            FlowShape::Decision,
            FlowShape::InputOutput,
            FlowShape::Subprocess,
            FlowShape::Document,
            FlowShape::DataStore,
            FlowShape::Manual,
            FlowShape::Delay,
            FlowShape::Connector,
            FlowShape::Note,
        ] {
            let nodes = draw(
                &element(shape),
                shape,
                &ESTIMATE,
                "#000000",
                LineWeight::Medium,
            );
            let Node::Path { d, paint } = &nodes[0] else {
                panic!("{shape:?} starts with its outline");
            };
            assert!(d.starts_with('M') && d.ends_with('Z'), "{shape:?}: {d}");
            assert_eq!(paint.fill.as_deref(), Some("#dae8fc"));
            let has_name = nodes.iter().any(|n| {
                matches!(n, Node::Group { children, .. }
                    if children.iter().any(|c| matches!(c, Node::Text { text, .. } if text.contains("Check"))))
            });
            assert!(has_name, "{shape:?}");
        }
        assert_eq!(
            FlowShape::Decision.outline(RECT),
            "M180 100L260 150L180 200L100 150Z"
        );
    }

    #[test]
    fn exports_each_shape_as_its_drawio_shape() {
        let xml = |shape| {
            drawio_cell(
                &element(shape),
                shape,
                (0.0, 0.0),
                "1",
                "#000000",
                LineWeight::Medium,
            )
        };
        assert!(xml(FlowShape::Decision).contains("rhombus=1"));
        assert!(xml(FlowShape::InputOutput).contains("shape=parallelogram"));
        assert!(xml(FlowShape::Subprocess).contains("shape=process"));
        assert!(xml(FlowShape::Document).contains("shape=document"));
        assert!(xml(FlowShape::DataStore).contains("shape=cylinder3"));
        assert!(xml(FlowShape::Manual).contains("flipV=1"));
        assert!(xml(FlowShape::Delay).contains("shape=delay"));
        assert!(xml(FlowShape::Note).contains("shape=note"));
        assert!(xml(FlowShape::Terminal).contains("arcSize=50"));
        assert!(xml(FlowShape::Process).contains(r#"value="Check stock""#));
    }
}
