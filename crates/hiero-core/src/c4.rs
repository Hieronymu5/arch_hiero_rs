//! The C4 model's shapes, shared by its three diagram types (system context, container, and
//! component): a person's figure with its text underneath, a box for a software system, a browser window for a web application, a folder
//! for static content, a box with a prompt for a backend, a bucket for a store, a cylinder for
//! a database, a cylinder on its side for a queue or topic, a hexagon for a service, a box with two tabs for a component, and a boundary around what a system or
//! container holds. Each shows its name, its type (with any technology) in square brackets,
//! and its description inside it; a boundary shows them in its bottom-left corner.
//!
//! The scene and the draw.io file both come from here, so the editor, the SVG export, and
//! draw.io agree.

use crate::export::drawio::{Style, cell, rect_attributes};
use crate::export::svg::escape_xml;
use crate::figures::actor;
use crate::geometry::{Point, Rect, fmt_num, round2};
use crate::look::{CORNER_NAME, DESCRIPTION, LookElement, NAME, PADDING_X, text_lines};
use crate::model::LineWeight;
use crate::routing::Side;
use crate::scene::{Anchor, Node, Paint};
use crate::text::{MeasureText, TextStyle, Weight, wrap_text};

/// What a C4 element is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4Shape {
    /// The actor figure, with the name, type, and description under it: a person.
    Person,
    /// A box with round corners: a software system.
    Box,
    /// A browser window, with a bar of three dots and an address across its top.
    Browser,
    /// A folder, with a tab on its top-left: static content.
    Folder,
    /// A box with a prompt (">_") in its top-left corner: a backend.
    Terminal,
    /// A bucket, wider at the top: a store, such as an S3 bucket.
    Bucket,
    /// A cylinder: a database.
    Cylinder,
    /// A cylinder lying on its side, its open end on the left: a queue or a topic.
    Pipe,
    /// A hexagon with points on its left and right: a service, such as a microservice's API.
    Hexagon,
    /// A box with two small tabs on its left edge: a component.
    Component,
    /// A line round what a software system or container holds, with its text in the
    /// bottom-left corner.
    Boundary,
}

/// The type line under a name, such as "[Container: Java and Spring Boot]".
pub const TYPE_LINE: TextStyle = TextStyle::new(11.0, Weight::Regular, 15.0, 2);
/// The corner of a box, browser, terminal, component, or boundary.
pub const CORNER_RADIUS: f64 = 8.0;
/// The bar across the top of a browser window.
pub const BROWSER_BAR: f64 = 20.0;
/// A folder's tab: its height, and its widest.
pub const FOLDER_TAB_HEIGHT: f64 = 14.0;
pub const FOLDER_TAB_MAX_WIDTH: f64 = 120.0;
/// The space a terminal keeps at its top for its prompt.
pub const TERMINAL_TOP: f64 = 30.0;
/// Half the height of a cylinder's or bucket's top, and of a bucket's bottom.
pub const CYLINDER_RY: f64 = 12.0;
pub const BUCKET_BOTTOM_RY: f64 = 8.0;
/// How far in a bucket's bottom is on each side, as a fraction of its width.
pub const BUCKET_TAPER: f64 = 0.1;
/// A component's two tabs, which stick out of its left edge.
pub const COMPONENT_TAB_WIDTH: f64 = 24.0;
pub const COMPONENT_TAB_HEIGHT: f64 = 10.0;
pub const COMPONENT_TAB_TOPS: [f64; 2] = [14.0, 30.0];
/// Space between a name, its type, and its description.
const TEXT_GAP: f64 = 2.0;
/// Space between a boundary's text and its bottom edge.
const BOUNDARY_TEXT_BOTTOM: f64 = 8.0;

/// A store drawn as a bucket in draw.io: a stencil, as the other figures' are.
const BUCKET_STENCIL: &str = "fVLRbsMgDPwaHiOl0EXb49S12m841E1QKESGNunfl9SsWzI1b77z+TgMQu1CCz0KWTo4o1BfQspP0u23QfKJrS+6w5jIVA/cltuScct4U2UMoUcdmbwCGagtcidE8h0O5hjzjHEtknn4qr1QO+2dS7PGuyDkgbkadNeQv7gj4x6m6ak6++uUeGSvfPotp5EPmD2scX+UcjuXvs20QDr1KGs/KtZSFr8zHAsYTSjIR5jSzhJYoAaLZFOcLDSzVhgQ+//0T65NtRZseYm1C2vrAz5RKn63lsBypSdPOMPGWn6rpwVaa/rwct3l6tfICz+8dE/FMkOi+Euq/R0=";

/// How far down its area a person's figure starts, and the gap between the figure and the
/// text under it.
pub const PERSON_FIGURE_TOP: f64 = 5.0;
pub const PERSON_TEXT_GAP: f64 = 6.0;

/// A person's figure in draw.io: the actor's head and shoulders in its 48 × 46 box.
const PERSON_STENCIL: &str = "fZLRbsIwDEW/Jo+VIO0mXifGxG+YYNqIEEdORsvfL5Ej1qBpb773yrcnVlW/jxMEVHrj4Yaq/1Raf7CZjhaZshuQI/ls5nmWeNiJnKp8FwkxoEni3YEtnBxKEhPTFWd7TnXF+gnZppL2B9XvDXmfdy35qPSXeCcw15Hp259Fo3M2xAK6SMl2kPaHSN0w6k3DWGWtzsNr+4UYG22dE+znVoCCX6Yb3VccLcbwtv4SsMkRLy0UP1q9dLDY2DElKDeQsGYOeMQu93QXB2MTxRkxrOxtbasg/3EZRxHX9/h93B8vz8PrfbIl/01/+AE=";

impl C4Shape {
    /// The part of `rect` its text is centred in, clear of what's drawn at its top and bottom.
    /// A person's text starts at the top of it, under the figure.
    pub fn text_area(self, rect: Rect) -> Rect {
        let (top, bottom, inset) = match self {
            Self::Person => (
                PERSON_FIGURE_TOP + actor::HEIGHT + PERSON_TEXT_GAP - 4.0,
                0.0,
                0.0,
            ),
            Self::Box | Self::Component | Self::Boundary => (0.0, 0.0, 0.0),
            Self::Browser => (BROWSER_BAR, 0.0, 0.0),
            Self::Folder => (FOLDER_TAB_HEIGHT, 0.0, 0.0),
            Self::Terminal => (TERMINAL_TOP, 0.0, 0.0),
            Self::Cylinder => (CYLINDER_RY * 2.0, CYLINDER_RY, 0.0),
            Self::Pipe => return pipe_text_area(rect),
            // Half the point, as the text sits across the hexagon's widest part.
            Self::Hexagon => (0.0, 0.0, hexagon_point(rect) / 2.0),
            Self::Bucket => (
                CYLINDER_RY * 2.0,
                BUCKET_BOTTOM_RY,
                rect.width * BUCKET_TAPER,
            ),
        };
        // A hexagon's slants keep its text off its outline, so it needs little padding.
        let padding = if self == Self::Hexagon {
            4.0
        } else {
            PADDING_X
        };
        Rect::new(
            rect.x + padding + inset,
            rect.y + top + 4.0,
            (rect.width - (padding + inset) * 2.0).max(0.0),
            (rect.height - top - bottom - 8.0).max(0.0),
        )
    }
}

/// How wide the ellipse at each end of a pipe is, at most: half its width.
pub const PIPE_RX: f64 = 14.0;

/// Half the width of the ellipses at a pipe's ends.
pub fn pipe_rx(rect: Rect) -> f64 {
    PIPE_RX.min(rect.width / 4.0).max(0.0)
}

/// A pipe's text sits between its left end's ellipse and its right end's curve.
fn pipe_text_area(rect: Rect) -> Rect {
    let rx = pipe_rx(rect);
    Rect::new(
        rect.x + rx * 2.0 + 4.0,
        rect.y + 4.0,
        (rect.width - rx * 3.0 - 8.0).max(0.0),
        (rect.height - 8.0).max(0.0),
    )
}

/// How far in from `edge` an ellipse `rx` wide and `ry` tall, centred `rx` in, is at `offset`
/// from its middle across it.
fn ellipse_depth(rx: f64, ry: f64, offset: f64) -> f64 {
    if ry <= 0.0 {
        return rx;
    }
    let t = (offset / ry).clamp(-1.0, 1.0);
    rx * (1.0 - (1.0 - t * t).sqrt())
}

/// Where an arrow ending at `point`, on `side` of a pipe filling `rect`, meets the pipe: on
/// the curve of its left or right end, or on its top or bottom, curving round the ends.
pub fn pipe_edge_point(rect: Rect, side: Side, point: Point) -> Point {
    let rx = pipe_rx(rect);
    let ry = rect.height / 2.0;
    if rx <= 0.0 || ry <= 0.0 {
        return point;
    }
    let middle = rect.y + ry;
    match side {
        Side::Left | Side::Right => {
            let into = ellipse_depth(rx, ry, point.y - middle);
            let x = if side == Side::Left {
                rect.x + into
            } else {
                rect.right() - into
            };
            Point::new(x, point.y)
        }
        Side::Top | Side::Bottom => {
            let from_side = (point.x - rect.x).min(rect.right() - point.x);
            if from_side >= rx {
                return point;
            }
            let down = ellipse_depth(ry, rx, rx - from_side.max(0.0));
            let y = if side == Side::Top {
                rect.y + down
            } else {
                rect.bottom() - down
            };
            Point::new(point.x, y)
        }
    }
}

/// How far a hexagon's left and right points stand out from its top and bottom edges: as a
/// regular hexagon's, but no more than a quarter of its width.
pub fn hexagon_point(rect: Rect) -> f64 {
    (rect.height / 2.0 * 30f64.to_radians().tan() * 2.0)
        .min(rect.width / 4.0)
        .max(0.0)
}

/// Where an arrow ending at `point`, on `side` of a hexagon filling `rect`, meets the hexagon:
/// the point moved straight in until it reaches the outline. Points on its top and bottom
/// edges, between the slants, stay where they are.
pub fn hexagon_edge_point(rect: Rect, side: Side, point: Point) -> Point {
    let inset = hexagon_point(rect);
    let half = rect.height / 2.0;
    if inset <= 0.0 || half <= 0.0 {
        return point;
    }
    let middle = rect.y + half;
    match side {
        Side::Left | Side::Right => {
            let into = inset * ((point.y - middle).abs() / half).min(1.0);
            let x = if side == Side::Left {
                rect.x + into
            } else {
                rect.right() - into
            };
            Point::new(x, point.y)
        }
        Side::Top | Side::Bottom => {
            // How far in from the nearer side it is, as a share of the slant's width.
            let from_side = (point.x - rect.x).min(rect.right() - point.x);
            if from_side >= inset {
                return point;
            }
            let down = half * (1.0 - (from_side / inset).max(0.0));
            let y = if side == Side::Top {
                rect.y + down
            } else {
                rect.bottom() - down
            };
            Point::new(point.x, y)
        }
    }
}

/// A folder's tab width in an element `width` wide.
pub fn folder_tab_width(width: f64) -> f64 {
    (width * 0.4).min(FOLDER_TAB_MAX_WIDTH)
}

/// An element's text laid out in lines.
struct Text {
    name: Vec<String>,
    kind: Vec<String>,
    description: Vec<String>,
}

impl Text {
    fn height(&self, name: TextStyle) -> f64 {
        let blocks = [
            self.name.len() as f64 * name.line_height,
            self.kind.len() as f64 * TYPE_LINE.line_height,
            self.description.len() as f64 * DESCRIPTION.line_height,
        ];
        let gaps = blocks
            .iter()
            .filter(|h| **h > 0.0)
            .count()
            .saturating_sub(1) as f64;
        blocks.iter().sum::<f64>() + gaps * TEXT_GAP
    }

    /// The nodes, the first line's box starting at `top`, anchored at `x`.
    fn nodes(&self, x: f64, top: f64, name: TextStyle, anchor: Anchor) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut y = top;
        for (lines, style) in [
            (&self.name, name),
            (&self.kind, TYPE_LINE),
            (&self.description, DESCRIPTION),
        ] {
            if lines.is_empty() {
                continue;
            }
            nodes.extend(text_lines(lines, x, y, style, anchor));
            y += lines.len() as f64 * style.line_height + TEXT_GAP;
        }
        nodes
    }
}

/// The text inside an element, wrapped to its text area: the name on up to two lines, the
/// type, and as much of the description as fits under them.
fn inside_text(element: &LookElement, area: Rect, measure: &dyn MeasureText) -> Text {
    let name = wrap_text(&element.label, area.width, NAME, measure);
    let kind = element
        .look
        .type_line
        .as_deref()
        .map(|t| wrap_text(t, area.width, TYPE_LINE, measure))
        .unwrap_or_default();
    let mut text = Text {
        name,
        kind,
        description: Vec::new(),
    };
    if let Some(d) = element
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        let room = area.height - text.height(NAME) - TEXT_GAP;
        let lines = (room / DESCRIPTION.line_height).floor().max(0.0) as usize;
        if lines > 0 {
            text.description = wrap_text(d, area.width, DESCRIPTION.with_max_lines(lines), measure);
        }
    }
    text
}

/// A boundary's text, in its bottom-left corner: the name and type, and the description on
/// one line.
fn corner_text(element: &LookElement, measure: &dyn MeasureText) -> Text {
    let width = (element.rect.width - PADDING_X * 2.0).max(0.0);
    Text {
        name: wrap_text(&element.label, width, CORNER_NAME, measure),
        kind: element
            .look
            .type_line
            .as_deref()
            .map(|t| wrap_text(t, width, TYPE_LINE.with_max_lines(1), measure))
            .unwrap_or_default(),
        description: element
            .description
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(|d| wrap_text(d, width, DESCRIPTION.with_max_lines(1), measure))
            .unwrap_or_default(),
    }
}

/// Where a person's text ends: under the figure, the name, the type, and as much of the
/// description as fits. An arrow from below ends just under it.
pub fn person_text_bottom(element: &LookElement, measure: &dyn MeasureText) -> f64 {
    let area = C4Shape::Person.text_area(element.rect);
    area.y + inside_text(element, area, measure).height(NAME)
}

/// The height a boundary keeps below what it holds, for its text.
pub const BOUNDARY_FOOTER: f64 = 60.0;

fn f(v: f64) -> String {
    fmt_num(round2(v))
}

/// The nodes that draw a C4 element, with lines as thick as `weight`. `border` is used when
/// the element sets no border colour.
pub fn draw(
    element: &LookElement,
    shape: C4Shape,
    measure: &dyn MeasureText,
    border: &str,
    weight: LineWeight,
) -> Vec<Node> {
    let rect = element.rect;
    let Rect {
        x,
        y,
        width,
        height,
    } = rect;
    let (right, bottom) = (rect.right(), rect.bottom());
    let look = &element.look;
    let border = look
        .border_color
        .clone()
        .unwrap_or_else(|| border.to_owned());
    let fill = look.fill.clone().unwrap_or_else(|| {
        if shape == C4Shape::Boundary {
            "none".into()
        } else {
            "#ffffff".into()
        }
    });
    let line = weight.width();
    let paint = Paint::fill_stroke(fill.clone(), border.clone(), line);
    let mut nodes = match shape {
        // The figure in the element's colour, outlined as other figures are.
        C4Shape::Person => vec![Node::group(
            Some(Point::new(
                round2(x + width / 2.0 - actor::WIDTH / 2.0),
                round2(y + PERSON_FIGURE_TOP),
            )),
            paint,
            crate::look::actor_figure(false),
        )],
        C4Shape::Box | C4Shape::Boundary => {
            vec![Node::rect(rect, CORNER_RADIUS, paint)]
        }
        C4Shape::Component => {
            let mut nodes = vec![Node::rect(rect, CORNER_RADIUS, paint.clone())];
            nodes.extend(COMPONENT_TAB_TOPS.iter().map(|top| {
                Node::rect(
                    Rect::new(
                        x - COMPONENT_TAB_WIDTH / 2.0,
                        y + top,
                        COMPONENT_TAB_WIDTH,
                        COMPONENT_TAB_HEIGHT,
                    ),
                    2.0,
                    paint.clone(),
                )
            }));
            nodes
        }
        C4Shape::Browser => {
            let r = CORNER_RADIUS;
            let bar = format!(
                "M{} {}A{r} {r} 0 0 1 {} {}H{}A{r} {r} 0 0 1 {} {}V{}H{}Z",
                f(x),
                f(y + r),
                f(x + r),
                f(y),
                f(right - r),
                f(right),
                f(y + r),
                f(y + BROWSER_BAR),
                f(x)
            );
            let middle = y + BROWSER_BAR / 2.0;
            let mut nodes = vec![
                Node::rect(rect, r, paint),
                Node::path(
                    bar,
                    Paint::fill_stroke(border.clone(), border.clone(), line),
                ),
            ];
            let white = Paint::fill("#ffffff");
            nodes.extend([10.0, 20.0, 30.0].map(|dx| Node::Circle {
                center: Point::new(x + dx, middle),
                r: 3.5,
                paint: white.clone(),
            }));
            if width > 60.0 {
                nodes.push(Node::rect(
                    Rect::new(x + 40.0, middle - 4.5, width - 48.0, 9.0),
                    4.5,
                    white,
                ));
            }
            nodes
        }
        C4Shape::Folder => {
            let tab = folder_tab_width(width);
            vec![
                Node::rect(
                    Rect::new(x, y, tab, FOLDER_TAB_HEIGHT + 6.0),
                    3.0,
                    paint.clone(),
                ),
                Node::rect(
                    Rect::new(x, y + FOLDER_TAB_HEIGHT, width, height - FOLDER_TAB_HEIGHT),
                    3.0,
                    paint,
                ),
            ]
        }
        C4Shape::Terminal => {
            let prompt = format!(
                "M{} {}L{} {}L{} {}M{} {}H{}",
                f(x + 12.0),
                f(y + 10.0),
                f(x + 22.0),
                f(y + 16.0),
                f(x + 12.0),
                f(y + 22.0),
                f(x + 25.0),
                f(y + 23.0),
                f(x + 37.0)
            );
            vec![
                Node::rect(rect, CORNER_RADIUS, paint),
                Node::path(
                    prompt,
                    Paint {
                        fill: Some("none".into()),
                        ..Paint::stroke(border.clone(), 2.5)
                    },
                ),
            ]
        }
        C4Shape::Cylinder => {
            let (rx, ry) = (width / 2.0, CYLINDER_RY);
            let side = format!(
                "M{} {}V{}A{} {} 0 0 0 {} {}V{}",
                f(x),
                f(y + ry),
                f(bottom - ry),
                f(rx),
                f(ry),
                f(right),
                f(bottom - ry),
                f(y + ry)
            );
            vec![
                Node::path(side, paint.clone()),
                Node::Ellipse {
                    center: Point::new(x + rx, y + ry),
                    rx,
                    ry,
                    paint,
                },
            ]
        }
        C4Shape::Pipe => {
            let (rx, ry) = (pipe_rx(rect), height / 2.0);
            let body = format!(
                "M{} {}H{}A{} {} 0 0 1 {} {}H{}Z",
                f(x + rx),
                f(y),
                f(right - rx),
                f(rx),
                f(ry),
                f(right - rx),
                f(bottom),
                f(x + rx)
            );
            vec![
                Node::path(body, paint.clone()),
                Node::Ellipse {
                    center: Point::new(x + rx, y + ry),
                    rx,
                    ry,
                    paint,
                },
            ]
        }
        C4Shape::Hexagon => {
            let inset = hexagon_point(rect);
            let middle = y + height / 2.0;
            let outline = format!(
                "M{} {}L{} {}H{}L{} {}L{} {}H{}Z",
                f(x),
                f(middle),
                f(x + inset),
                f(y),
                f(right - inset),
                f(right),
                f(middle),
                f(right - inset),
                f(bottom),
                f(x + inset)
            );
            vec![Node::path(outline, paint)]
        }
        C4Shape::Bucket => {
            let (rx, ry) = (width / 2.0, CYLINDER_RY);
            let taper = width * BUCKET_TAPER;
            let side = format!(
                "M{} {}L{} {}A{} {} 0 0 0 {} {}L{} {}Z",
                f(x),
                f(y + ry),
                f(x + taper),
                f(bottom - BUCKET_BOTTOM_RY),
                f(rx - taper),
                f(BUCKET_BOTTOM_RY),
                f(right - taper),
                f(bottom - BUCKET_BOTTOM_RY),
                f(right),
                f(y + ry)
            );
            vec![
                Node::path(side, paint.clone()),
                Node::Ellipse {
                    center: Point::new(x + rx, y + ry),
                    rx,
                    ry,
                    paint,
                },
            ]
        }
    };
    let text = if shape == C4Shape::Boundary {
        let text = corner_text(element, measure);
        let top = bottom - BOUNDARY_TEXT_BOTTOM - text.height(CORNER_NAME);
        text.nodes(x + PADDING_X, top, CORNER_NAME, Anchor::Start)
    } else {
        let area = shape.text_area(rect);
        let text = inside_text(element, area, measure);
        let top = if shape == C4Shape::Person {
            area.y
        } else {
            area.y + ((area.height - text.height(NAME)) / 2.0).max(0.0)
        };
        text.nodes(area.x + area.width / 2.0, top, NAME, Anchor::Middle)
    };
    if !text.is_empty() {
        nodes.push(match &look.text_color {
            Some(color) => Node::group(None, Paint::fill(color.clone()), text),
            None => Node::group(None, Paint::default(), text),
        });
    }
    nodes
}

/// An element's text as draw.io HTML: the name in bold, then the type and description in
/// smaller text, each line break kept.
pub fn drawio_label(element: &LookElement) -> String {
    let mut out = format!("<b>{}</b>", escape_xml(&element.label));
    if let Some(t) = &element.look.type_line {
        out.push_str(&format!(
            r#"<br><span style="font-size: 11px">{}</span>"#,
            escape_xml(t)
        ));
    }
    if let Some(d) = element
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        let lines: Vec<String> = d.lines().map(escape_xml).collect();
        out.push_str(&format!(
            r#"<br><span style="font-size: 13px">{}</span>"#,
            lines.join("<br>")
        ));
    }
    out
}

/// The draw.io cells for a C4 element at `left`, `top` in `parent`: its own cell, which
/// arrows attach to, then any details drawn over it as its children, such as a browser's bar.
pub fn drawio_cells(
    element: &LookElement,
    shape: C4Shape,
    (left, top): (f64, f64),
    parent: &str,
    border: &str,
    weight: LineWeight,
) -> Vec<String> {
    let look = &element.look;
    let rect = element.rect;
    let border = look.border_color.as_deref().unwrap_or(border);
    let fill = look
        .fill
        .as_deref()
        .unwrap_or(if shape == C4Shape::Boundary {
            "none"
        } else {
            "#ffffff"
        });
    let text = look.text_color.as_deref().unwrap_or(crate::look::TEXT);
    let area = shape.text_area(Rect::new(0.0, 0.0, rect.width, rect.height));
    let mut style = Style::new()
        .set("html", 1)
        .set("whiteSpace", "wrap")
        .set("strokeColor", border)
        .num("strokeWidth", weight.width())
        .set("fillColor", fill)
        .set("fontColor", text)
        .set("fontSize", 16)
        .num("spacingLeft", area.x)
        .num("spacingRight", rect.width - area.right())
        .num("spacingTop", area.y)
        .num("spacingBottom", rect.height - area.bottom());
    let rounded = |s: Style| {
        s.set("rounded", 1)
            .set("absoluteArcSize", 1)
            .num("arcSize", CORNER_RADIUS * 2.0)
    };
    style = match shape {
        C4Shape::Box | C4Shape::Component | C4Shape::Browser | C4Shape::Terminal => rounded(style),
        C4Shape::Folder => style
            .set("shape", "folder")
            .set("tabPosition", "left")
            .num("tabWidth", folder_tab_width(rect.width))
            .num("tabHeight", FOLDER_TAB_HEIGHT),
        C4Shape::Cylinder => style
            .set("shape", "cylinder3")
            .set("boundedLbl", 1)
            .set("backgroundOutline", 1)
            .num("size", CYLINDER_RY),
        C4Shape::Bucket => style.set("shape", format!("stencil({BUCKET_STENCIL})")),
        // draw.io's cylinder turned so its top faces left; its text stays level.
        C4Shape::Pipe => style
            .set("shape", "cylinder3")
            .set("direction", "north")
            .set("boundedLbl", 1)
            .set("backgroundOutline", 1)
            .num("size", pipe_rx(rect)),
        C4Shape::Hexagon => style
            .set("shape", "hexagon")
            .set("perimeter", "hexagonPerimeter2")
            .set("fixedSize", 1)
            .num("size", round2(hexagon_point(rect))),
        // Only text: the figure is a detail on top of it.
        C4Shape::Person => style
            .set("fillColor", "none")
            .set("strokeColor", "none")
            .set("verticalAlign", "top"),
        C4Shape::Boundary => rounded(style)
            .set("container", 1)
            .set("collapsible", 0)
            .set("align", "left")
            .set("verticalAlign", "bottom")
            .num("spacingLeft", PADDING_X)
            .num("spacingBottom", BOUNDARY_TEXT_BOTTOM),
    };
    let id = element.id.as_str();
    let mut cells = vec![cell(
        id,
        &escape_xml(&drawio_label(element)),
        &style.build(),
        parent,
        &rect_attributes(Rect::new(left, top, rect.width, rect.height)),
    )];
    // Details, placed in the element: they don't take arrows.
    let detail = |name: &str, style: Style, r: Rect| {
        cell(
            &format!("{id}-{name}"),
            "",
            &style
                .set("connectable", 0)
                .set("pointerEvents", 0)
                .set("resizable", 0)
                .build(),
            &escape_xml(id),
            &rect_attributes(r),
        )
    };
    let solid = |color: &str| {
        Style::new()
            .set("html", 1)
            .set("fillColor", color)
            .set("strokeColor", "none")
    };
    match shape {
        C4Shape::Person => {
            cells.push(detail(
                "figure",
                Style::new()
                    .set("shape", format!("stencil({PERSON_STENCIL})"))
                    .set("html", 1)
                    .set("fillColor", fill)
                    .set("strokeColor", border)
                    .num("strokeWidth", weight.width()),
                Rect::new(
                    rect.width / 2.0 - actor::WIDTH / 2.0,
                    PERSON_FIGURE_TOP,
                    actor::WIDTH,
                    actor::HEIGHT,
                ),
            ));
        }
        C4Shape::Browser => {
            cells.push(detail(
                "bar",
                rounded(solid(border))
                    .set("strokeColor", border)
                    .num("strokeWidth", weight.width()),
                Rect::new(0.0, 0.0, rect.width, BROWSER_BAR),
            ));
            for (i, dx) in [10.0, 20.0, 30.0].into_iter().enumerate() {
                cells.push(detail(
                    &format!("dot-{i}"),
                    solid("#ffffff").set("ellipse", 1),
                    Rect::new(dx - 3.5, BROWSER_BAR / 2.0 - 3.5, 7.0, 7.0),
                ));
            }
            if rect.width > 60.0 {
                cells.push(detail(
                    "address",
                    solid("#ffffff").set("rounded", 1).set("arcSize", 50),
                    Rect::new(40.0, BROWSER_BAR / 2.0 - 4.5, rect.width - 48.0, 9.0),
                ));
            }
        }
        C4Shape::Terminal => {
            cells.push(
                detail(
                    "prompt",
                    Style::new()
                        .set("text", 1)
                        .set("html", 1)
                        .set("fillColor", "none")
                        .set("strokeColor", "none")
                        .set("fontColor", border)
                        .set("fontStyle", 1)
                        .set("fontSize", 18)
                        .set("align", "left")
                        .set("verticalAlign", "top")
                        .set("spacing", 0)
                        .set("whiteSpace", "nowrap"),
                    Rect::new(10.0, 4.0, 30.0, 24.0),
                )
                .replace(r#"value="""#, r#"value="&amp;gt;_""#),
            );
        }
        C4Shape::Component => {
            for (i, tab_top) in COMPONENT_TAB_TOPS.into_iter().enumerate() {
                cells.push(detail(
                    &format!("tab-{i}"),
                    Style::new()
                        .set("html", 1)
                        .set("rounded", 1)
                        .set("absoluteArcSize", 1)
                        .set("arcSize", 4)
                        .set("fillColor", fill)
                        .set("strokeColor", border)
                        .num("strokeWidth", weight.width()),
                    Rect::new(
                        -COMPONENT_TAB_WIDTH / 2.0,
                        tab_top,
                        COMPONENT_TAB_WIDTH,
                        COMPONENT_TAB_HEIGHT,
                    ),
                ));
            }
        }
        _ => {}
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::svg::scene_to_svg;
    use crate::look::{Look, Shape};
    use crate::scene::Scene;
    use crate::text::EstimateText;

    fn element(shape: C4Shape, rect: Rect) -> LookElement {
        LookElement {
            id: "e".into(),
            parent_id: None,
            label: "Backend".into(),
            description: Some("Provides banking over a JSON API.".into()),
            look: Look {
                type_line: Some("[Container: Java]".into()),
                fill: Some("#dae8fc".into()),
                border_color: Some("#2d5f9e".into()),
                ..Look::new(Shape::C4(shape))
            },
            rect,
        }
    }

    fn svg(nodes: Vec<Node>) -> String {
        scene_to_svg(
            "t",
            &Scene {
                nodes,
                bounds: Rect::new(0.0, 0.0, 400.0, 400.0),
                text_color: "#000000".into(),
                marker_color: "#000000".into(),
                background: "#ffffff".into(),
            },
        )
    }

    #[test]
    fn shows_the_name_type_and_description_inside() {
        let e = element(C4Shape::Terminal, Rect::new(40.0, 40.0, 240.0, 160.0));
        let out = svg(draw(
            &e,
            C4Shape::Terminal,
            &EstimateText,
            "#000000",
            LineWeight::Medium,
        ));
        assert!(out.contains(r##"rx="8" fill="#dae8fc" stroke="#2d5f9e" stroke-width="1.5""##));
        assert!(out.contains(r#"font-weight="700">Backend</text>"#));
        assert!(out.contains(r#"font-size="11" font-weight="400">[Container: Java]</text>"#));
        assert!(out.contains(">API.</text>"), "{out}");
        // The prompt in the top-left corner.
        assert!(out.contains(r#"<path d="M52 50L62 56L52 62M65 63H77""#));
    }

    #[test]
    fn keeps_text_clear_of_a_shapes_top_and_bottom() {
        let rect = Rect::new(0.0, 0.0, 240.0, 160.0);
        assert_eq!(C4Shape::Browser.text_area(rect).y, 24.0);
        assert_eq!(C4Shape::Cylinder.text_area(rect).bottom(), 144.0);
        let bucket = C4Shape::Bucket.text_area(rect);
        assert_eq!((bucket.x, bucket.width), (34.0, 172.0));
    }

    #[test]
    fn services_are_hexagons_that_arrows_meet_on_their_slants() {
        let rect = Rect::new(0.0, 0.0, 240.0, 160.0);
        // A regular hexagon's point would be 80 × tan 30° × 2 = 92.38, more than a quarter of
        // the width.
        assert_eq!(hexagon_point(rect), 60.0);
        let out = svg(draw(
            &element(C4Shape::Hexagon, rect),
            C4Shape::Hexagon,
            &EstimateText,
            "#000000",
            LineWeight::Medium,
        ));
        assert!(
            out.contains(r#"<path d="M0 80L60 0H180L240 80L180 160H60Z""#),
            "{out}"
        );
        // At the left point, the arrow ends there; halfway up, halfway in; at a corner, all in.
        let left = |y| hexagon_edge_point(rect, Side::Left, Point::new(0.0, y));
        assert_eq!(left(80.0), Point::new(0.0, 80.0));
        assert_eq!(left(40.0), Point::new(30.0, 40.0));
        assert_eq!(left(0.0), Point::new(60.0, 0.0));
        // On the top edge between the slants, it stays; over a slant, it moves down onto it.
        let top = |x| hexagon_edge_point(rect, Side::Top, Point::new(x, 0.0));
        assert_eq!(top(120.0), Point::new(120.0, 0.0));
        assert_eq!(top(30.0), Point::new(30.0, 40.0));
        assert_eq!(top(210.0), Point::new(210.0, 40.0));
        // draw.io's hexagon with the same point.
        let cells = drawio_cells(
            &element(C4Shape::Hexagon, rect),
            C4Shape::Hexagon,
            (0.0, 0.0),
            "1",
            "#000000",
            LineWeight::Medium,
        );
        assert!(cells[0].contains("shape=hexagon;perimeter=hexagonPerimeter2;fixedSize=1;size=60"));
    }

    #[test]
    fn queues_are_cylinders_on_their_side_that_arrows_meet_on_their_ends() {
        let rect = Rect::new(0.0, 0.0, 240.0, 140.0);
        let out = svg(draw(
            &element(C4Shape::Pipe, rect),
            C4Shape::Pipe,
            &EstimateText,
            "#000000",
            LineWeight::Medium,
        ));
        assert!(
            out.contains(r#"<path d="M14 0H226A14 70 0 0 1 226 140H14Z""#),
            "{out}"
        );
        assert!(
            out.contains(r#"<ellipse cx="14" cy="70" rx="14" ry="70""#),
            "{out}"
        );
        // Text clear of the left end's ellipse and the right end's curve.
        let area = C4Shape::Pipe.text_area(rect);
        assert_eq!((area.x, area.right()), (32.0, 222.0));
        // Level with the middle, arrows end at the ends' outer points; near a corner, in on the
        // curve; on the top, between the ends, where they are.
        let left = |y| pipe_edge_point(rect, Side::Left, Point::new(0.0, y));
        assert_eq!(left(70.0), Point::new(0.0, 70.0));
        assert!((left(10.0).x - 6.79).abs() < 0.01, "{:?}", left(10.0));
        let right = pipe_edge_point(rect, Side::Right, Point::new(240.0, 0.0));
        assert_eq!(right, Point::new(226.0, 0.0));
        let top = |x| pipe_edge_point(rect, Side::Top, Point::new(x, 0.0));
        assert_eq!(top(120.0), Point::new(120.0, 0.0));
        assert!(top(2.0).y > 30.0, "{:?}", top(2.0));
        let cells = drawio_cells(
            &element(C4Shape::Pipe, rect),
            C4Shape::Pipe,
            (0.0, 0.0),
            "1",
            "#000000",
            LineWeight::Medium,
        );
        assert!(cells[0].contains("shape=cylinder3;direction=north;"));
    }

    #[test]
    fn people_show_their_text_under_the_figure_in_the_usual_sizes() {
        let rect = Rect::new(0.0, 0.0, 200.0, 160.0);
        let mut e = element(C4Shape::Person, rect);
        e.label = "Customer".into();
        e.look.type_line = Some("[Person]".into());
        e.description = Some("Banks online.".into());
        e.look.border_color = None;
        let out = svg(draw(
            &e,
            C4Shape::Person,
            &EstimateText,
            "#1e1e1e",
            LineWeight::Medium,
        ));
        // The figure, centred at the top, in the element's fill with a dark outline.
        assert!(
            out.contains(r##"<g transform="translate(76 5)" fill="#dae8fc" stroke="#1e1e1e""##),
            "{out}"
        );
        // The name, type, and description in the same sizes as other elements', from the top.
        assert!(out.contains(r#"font-size="16" font-weight="700">Customer</text>"#));
        assert!(out.contains(r#"font-size="11" font-weight="400">[Person]</text>"#));
        assert!(out.contains(r#"font-size="13" font-weight="400">Banks online.</text>"#));
        // Under the figure, and ending after the three lines.
        let area = C4Shape::Person.text_area(rect);
        assert_eq!(area.y, 57.0);
        assert_eq!(
            person_text_bottom(&e, &EstimateText),
            57.0 + 24.0 + 2.0 + 15.0 + 2.0 + 17.0
        );
        let cells = drawio_cells(
            &e,
            C4Shape::Person,
            (0.0, 0.0),
            "1",
            "#1e1e1e",
            LineWeight::Medium,
        );
        // Only text, at the top: the figure is drawn by its own cell.
        for part in ["strokeColor=none", "fillColor=none", "verticalAlign=top"] {
            assert!(cells[0].contains(part), "{part}: {}", cells[0]);
        }
        assert!(cells[1].contains(r#"id="e-figure""#));
        assert!(cells[1].contains(r#"x="76" y="5" width="48" height="46""#));
    }

    #[test]
    fn boundaries_put_their_text_in_the_bottom_left_corner() {
        let mut e = element(C4Shape::Boundary, Rect::new(0.0, 0.0, 600.0, 400.0));
        e.look.fill = None;
        e.look.type_line = Some("[Software System]".into());
        let out = svg(draw(
            &e,
            C4Shape::Boundary,
            &EstimateText,
            "#000000",
            LineWeight::Medium,
        ));
        assert!(out.contains(r##"fill="none" stroke="#2d5f9e""##));
        assert!(out.contains(r#"<text x="10" "#));
        assert!(!out.contains(r#"text-anchor="middle""#), "{out}");
    }

    #[test]
    fn draws_the_same_shapes_in_drawio() {
        let e = element(C4Shape::Browser, Rect::new(40.0, 40.0, 240.0, 160.0));
        let cells = drawio_cells(
            &e,
            C4Shape::Browser,
            (40.0, 40.0),
            "1",
            "#000000",
            LineWeight::Medium,
        )
        .join("\n");
        assert!(cells.contains(r#"id="e" value="&lt;b&gt;Backend&lt;/b&gt;"#));
        assert!(cells.contains(r#"id="e-bar""#));
        assert!(cells.contains(r#"parent="e""#));
        let cylinder = drawio_cells(
            &element(C4Shape::Cylinder, Rect::new(0.0, 0.0, 240.0, 160.0)),
            C4Shape::Cylinder,
            (0.0, 0.0),
            "1",
            "#000000",
            LineWeight::Medium,
        );
        assert!(cylinder[0].contains("shape=cylinder3"));
    }
}
