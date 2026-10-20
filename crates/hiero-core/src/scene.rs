//! A drawing as a tree of SVG-like shapes. Each diagram type builds one scene from its laid-out
//! content; the SVG export serialises it and the editor paints it, so they always agree.

use crate::geometry::{Point, Rect};
use crate::text::Weight;

/// Fill and stroke, inherited from groups as in SVG. `None` inherits; a fill of `"none"`
/// means no fill.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paint {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    /// Dash and gap lengths, such as `(8, 4)`.
    pub dash: Option<(f64, f64)>,
}

impl Paint {
    pub fn fill(color: impl Into<String>) -> Self {
        Self {
            fill: Some(color.into()),
            ..Self::default()
        }
    }

    pub fn stroke(color: impl Into<String>, width: f64) -> Self {
        Self {
            stroke: Some(color.into()),
            stroke_width: Some(width),
            ..Self::default()
        }
    }

    pub fn fill_stroke(fill: impl Into<String>, stroke: impl Into<String>, width: f64) -> Self {
        Self {
            fill: Some(fill.into()),
            stroke: Some(stroke.into()),
            stroke_width: Some(width),
            dash: None,
        }
    }

    pub fn dashed(mut self, dash: Option<(f64, f64)>) -> Self {
        self.dash = dash;
        self
    }

    /// This paint over `parent`: anything unset is inherited.
    pub fn over(&self, parent: &Paint) -> Paint {
        Paint {
            fill: self.fill.clone().or_else(|| parent.fill.clone()),
            stroke: self.stroke.clone().or_else(|| parent.stroke.clone()),
            stroke_width: self.stroke_width.or(parent.stroke_width),
            dash: self.dash.or(parent.dash),
        }
    }
}

/// How text lines up with its x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// The head drawn at the end of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// An arrow's head: 12 px.
    Arrowhead,
    /// The small head of a return arrow: 6 px.
    ReturnArrowhead,
}

impl Marker {
    pub const fn size(self) -> f64 {
        match self {
            Self::Arrowhead => 12.0,
            Self::ReturnArrowhead => 6.0,
        }
    }
}

/// A shape in a scene.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Rect {
        rect: Rect,
        /// Corner radius.
        rx: f64,
        paint: Paint,
    },
    Circle {
        center: Point,
        r: f64,
        paint: Paint,
    },
    Ellipse {
        center: Point,
        rx: f64,
        ry: f64,
        paint: Paint,
    },
    Line {
        from: Point,
        to: Point,
        paint: Paint,
        marker: Option<Marker>,
    },
    /// An SVG path, as its `d` attribute.
    Path {
        d: String,
        paint: Paint,
    },
    Text {
        /// x, and the baseline's y.
        at: Point,
        anchor: Anchor,
        size: f64,
        weight: Weight,
        text: String,
        /// The text colour. `None` inherits the scene's text colour.
        fill: Option<String>,
        /// An outline around the glyphs, drawn under them so text reads over lines.
        halo: Option<(String, f64)>,
    },
    /// A PNG image.
    Image {
        rect: Rect,
        png: &'static [u8],
    },
    Group {
        /// Moves the group's contents by this much.
        translate: Option<Point>,
        paint: Paint,
        children: Vec<Node>,
    },
}

impl Node {
    pub fn rect(rect: Rect, rx: f64, paint: Paint) -> Self {
        Self::Rect { rect, rx, paint }
    }

    pub fn line(from: Point, to: Point, paint: Paint) -> Self {
        Self::Line {
            from,
            to,
            paint,
            marker: None,
        }
    }

    pub fn path(d: impl Into<String>, paint: Paint) -> Self {
        Self::Path { d: d.into(), paint }
    }

    pub fn group(translate: Option<Point>, paint: Paint, children: Vec<Node>) -> Self {
        Self::Group {
            translate,
            paint,
            children,
        }
    }

    /// Text in the scene's text colour, or in `fill`.
    pub fn text(
        at: Point,
        anchor: Anchor,
        size: f64,
        weight: Weight,
        text: impl Into<String>,
    ) -> Self {
        Self::Text {
            at,
            anchor,
            size,
            weight,
            text: text.into(),
            fill: None,
            halo: None,
        }
    }
}

/// A drawing: its shapes, back to front, and the area its content covers.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub nodes: Vec<Node>,
    /// What the drawing covers, which exports crop to (plus a margin).
    pub bounds: Rect,
    /// The colour of text that doesn't set its own.
    pub text_color: String,
    /// The colour of arrowheads.
    pub marker_color: String,
    /// The paper behind the drawing.
    pub background: String,
}

/// One segment of a parsed path, in absolute coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Arc {
        rx: f64,
        ry: f64,
        rotation: f64,
        large: bool,
        sweep: bool,
        to: Point,
    },
    Close,
}

/// Parses an SVG path's `d` attribute: M, L, H, V, C, Q, A, and Z, absolute or relative.
pub fn parse_path(d: &str) -> Vec<Seg> {
    let mut tokens: Vec<Token> = Vec::new();
    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            tokens.push(Token::Cmd(c));
            i += 1;
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut seen_dot = c == '.';
            while i < chars.len() {
                let n = chars[i];
                if n.is_ascii_digit() {
                    i += 1;
                } else if n == '.' && !seen_dot {
                    seen_dot = true;
                    i += 1;
                } else if (n == 'e' || n == 'E') && i + 1 < chars.len() {
                    i += 1;
                    if chars[i] == '-' || chars[i] == '+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token::Num(text.parse().unwrap_or(0.0)));
        } else {
            i += 1;
        }
    }

    let mut segs = Vec::new();
    let mut pos = Point::default();
    let mut start = Point::default();
    let mut cmd = 'M';
    let mut t = 0;
    let num = |t: &mut usize| -> Option<f64> {
        match tokens.get(*t) {
            Some(Token::Num(n)) => {
                *t += 1;
                Some(*n)
            }
            _ => None,
        }
    };
    while t < tokens.len() {
        if let Token::Cmd(c) = tokens[t] {
            cmd = c;
            t += 1;
            if c == 'Z' || c == 'z' {
                segs.push(Seg::Close);
                pos = start;
                continue;
            }
        }
        let relative = cmd.is_ascii_lowercase();
        let base = if relative { pos } else { Point::default() };
        let pt = |x: f64, y: f64| Point::new(base.x + x, base.y + y);
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(x), Some(y)) = (num(&mut t), num(&mut t)) else {
                    break;
                };
                pos = pt(x, y);
                start = pos;
                segs.push(Seg::Move(pos));
                // Further pairs are lines.
                cmd = if relative { 'l' } else { 'L' };
            }
            'L' => {
                let (Some(x), Some(y)) = (num(&mut t), num(&mut t)) else {
                    break;
                };
                pos = pt(x, y);
                segs.push(Seg::Line(pos));
            }
            'H' => {
                let Some(x) = num(&mut t) else { break };
                pos = Point::new(if relative { pos.x + x } else { x }, pos.y);
                segs.push(Seg::Line(pos));
            }
            'V' => {
                let Some(y) = num(&mut t) else { break };
                pos = Point::new(pos.x, if relative { pos.y + y } else { y });
                segs.push(Seg::Line(pos));
            }
            'C' => {
                let v: Vec<f64> = (0..6).filter_map(|_| num(&mut t)).collect();
                if v.len() < 6 {
                    break;
                }
                let (a, b, c) = (pt(v[0], v[1]), pt(v[2], v[3]), pt(v[4], v[5]));
                pos = c;
                segs.push(Seg::Cubic(a, b, c));
            }
            'Q' => {
                let v: Vec<f64> = (0..4).filter_map(|_| num(&mut t)).collect();
                if v.len() < 4 {
                    break;
                }
                let (a, b) = (pt(v[0], v[1]), pt(v[2], v[3]));
                pos = b;
                segs.push(Seg::Quad(a, b));
            }
            'A' => {
                let v: Vec<f64> = (0..7).filter_map(|_| num(&mut t)).collect();
                if v.len() < 7 {
                    break;
                }
                let to = pt(v[5], v[6]);
                segs.push(Seg::Arc {
                    rx: v[0],
                    ry: v[1],
                    rotation: v[2],
                    large: v[3] != 0.0,
                    sweep: v[4] != 0.0,
                    to,
                });
                pos = to;
            }
            _ => {
                t += 1;
            }
        }
    }
    segs
}

#[derive(Debug, Clone, Copy)]
enum Token {
    Cmd(char),
    Num(f64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_figure_paths() {
        let segs = parse_path("M4 45 A20 20 0 0 1 44 45 Z");
        assert_eq!(segs.len(), 3);
        assert!(matches!(
            segs[1],
            Seg::Arc {
                rx: 20.0,
                sweep: true,
                ..
            }
        ));
        let hull = parse_path("M1 27H42V7H50V27H67Q66 39 56 39H9Q2 39 1 27Z");
        assert_eq!(hull[1], Seg::Line(Point::new(42.0, 27.0)));
        assert_eq!(hull[2], Seg::Line(Point::new(42.0, 7.0)));
        assert!(matches!(hull.last(), Some(Seg::Close)));
        let multi = parse_path("M1.5 10V30M3 10H-4.5");
        assert_eq!(multi[2], Seg::Move(Point::new(3.0, 10.0)));
        assert_eq!(multi[3], Seg::Line(Point::new(-4.5, 10.0)));
    }

    #[test]
    fn paint_inherits() {
        let parent = Paint::fill_stroke("#fff", "#000", 2.0);
        let child = Paint::fill("#f00");
        let p = child.over(&parent);
        assert_eq!(p.fill.as_deref(), Some("#f00"));
        assert_eq!(p.stroke.as_deref(), Some("#000"));
        assert_eq!(p.stroke_width, Some(2.0));
    }
}
