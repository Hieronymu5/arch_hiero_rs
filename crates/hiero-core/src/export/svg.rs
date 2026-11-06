//! Serialises a [`Scene`] as a standalone SVG document, cropped to the drawing plus a margin.

use std::fmt::Write as _;

use base64::Engine as _;

use crate::geometry::fmt_num;
use crate::scene::{Anchor, Marker, Node, Paint, Scene};
use crate::text::FONT_FAMILY;

/// Space kept around the drawing.
pub const MARGIN: f64 = 20.0;

/// Escapes text for XML content and attributes.
pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn n(value: f64) -> String {
    fmt_num(value)
}

fn paint_attrs(out: &mut String, paint: &Paint) {
    if let Some(fill) = &paint.fill {
        let _ = write!(out, r#" fill="{fill}""#);
    }
    if let Some(stroke) = &paint.stroke {
        let _ = write!(out, r#" stroke="{stroke}""#);
    }
    if let Some(width) = paint.stroke_width {
        let _ = write!(out, r#" stroke-width="{}""#, n(width));
    }
    if let Some((dash, gap)) = paint.dash {
        let _ = write!(out, r#" stroke-dasharray="{} {}""#, n(dash), n(gap));
    }
}

fn write_node(out: &mut String, node: &Node) {
    match node {
        Node::Rect { rect, rx, paint } => {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}""#,
                n(rect.x),
                n(rect.y),
                n(rect.width),
                n(rect.height)
            );
            if *rx != 0.0 {
                let _ = write!(out, r#" rx="{}""#, n(*rx));
            }
            paint_attrs(out, paint);
            out.push_str("/>");
        }
        Node::Circle { center, r, paint } => {
            let _ = write!(
                out,
                r#"<circle cx="{}" cy="{}" r="{}""#,
                n(center.x),
                n(center.y),
                n(*r)
            );
            paint_attrs(out, paint);
            out.push_str("/>");
        }
        Node::Ellipse {
            center,
            rx,
            ry,
            paint,
        } => {
            let _ = write!(
                out,
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}""#,
                n(center.x),
                n(center.y),
                n(*rx),
                n(*ry)
            );
            paint_attrs(out, paint);
            out.push_str("/>");
        }
        Node::Line {
            from,
            to,
            paint,
            marker,
        } => {
            let _ = write!(
                out,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}""#,
                n(from.x),
                n(from.y),
                n(to.x),
                n(to.y)
            );
            paint_attrs(out, paint);
            match marker {
                Some(Marker::Arrowhead) => out.push_str(r#" marker-end="url(#arrowhead)""#),
                Some(Marker::ReturnArrowhead) => {
                    out.push_str(r#" marker-end="url(#return-arrowhead)""#);
                }
                None => {}
            }
            out.push_str("/>");
        }
        Node::Path { d, paint } => {
            let _ = write!(out, r#"<path d="{d}""#);
            paint_attrs(out, paint);
            out.push_str("/>");
        }
        Node::Text {
            at,
            anchor,
            size,
            weight,
            text,
            fill,
            halo,
        } => {
            let anchor = match anchor {
                Anchor::Start => "start",
                Anchor::Middle => "middle",
                Anchor::End => "end",
            };
            let _ = write!(
                out,
                r#"<text x="{}" y="{}" text-anchor="{anchor}" font-size="{}" font-weight="{}""#,
                n(at.x),
                n(at.y),
                n(*size),
                weight.css()
            );
            if let Some(fill) = fill {
                let _ = write!(out, r#" fill="{fill}""#);
            }
            if let Some((color, width)) = halo {
                let _ = write!(
                    out,
                    r#" stroke="{color}" stroke-width="{}" stroke-linejoin="round" paint-order="stroke""#,
                    n(*width)
                );
            }
            let _ = write!(out, ">{}</text>", escape_xml(text));
        }
        Node::Image { rect, png } => {
            let data = base64::engine::general_purpose::STANDARD.encode(png);
            let _ = write!(
                out,
                r#"<image x="{}" y="{}" width="{}" height="{}" href="data:image/png;base64,{data}"/>"#,
                n(rect.x),
                n(rect.y),
                n(rect.width),
                n(rect.height)
            );
        }
        Node::Group {
            translate,
            paint,
            children,
        } => {
            out.push_str("<g");
            if let Some(t) = translate {
                let _ = write!(out, r#" transform="translate({} {})""#, n(t.x), n(t.y));
            }
            paint_attrs(out, paint);
            out.push('>');
            for child in children {
                write_node(out, child);
            }
            out.push_str("</g>");
        }
    }
}

/// The scene as an SVG document titled `title`.
pub fn scene_to_svg(title: &str, scene: &Scene) -> String {
    let b = scene.bounds;
    let min_x = b.x - MARGIN;
    let min_y = b.y - MARGIN;
    let width = b.right() + MARGIN - min_x;
    let height = b.bottom() + MARGIN - min_y;
    let lines = &scene.marker_color;
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}" font-family="{FONT_FAMILY}" fill="{text}">"#,
        w = n(width),
        h = n(height),
        x = n(min_x),
        y = n(min_y),
        text = scene.text_color,
    );
    let _ = writeln!(out, "<title>{}</title>", escape_xml(title));
    let _ = writeln!(
        out,
        r#"<defs><marker id="arrowhead" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="12" markerHeight="12" markerUnits="userSpaceOnUse" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" fill="{lines}"/></marker><marker id="return-arrowhead" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="6" markerHeight="6" markerUnits="userSpaceOnUse" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" fill="{lines}"/></marker></defs>"#
    );
    let _ = writeln!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
        n(min_x),
        n(min_y),
        n(width),
        n(height),
        scene.background
    );
    for node in &scene.nodes {
        write_node(&mut out, node);
        out.push('\n');
    }
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Rect};
    use crate::text::Weight;

    #[test]
    fn writes_a_cropped_document() {
        let scene = Scene {
            nodes: vec![
                Node::rect(
                    Rect::new(40.0, 40.0, 100.0, 50.0),
                    6.0,
                    Paint::fill_stroke("#fff", "#000", 2.0),
                ),
                Node::text(
                    Point::new(90.0, 70.0),
                    Anchor::Middle,
                    16.0,
                    Weight::Bold,
                    "A & B",
                ),
            ],
            bounds: Rect::new(40.0, 40.0, 100.0, 50.0),
            text_color: "#1b1f24".into(),
            marker_color: "#0066cc".into(),
            background: "#ffffff".into(),
        };
        let svg = scene_to_svg("T<1>", &scene);
        assert!(svg.contains(r#"viewBox="20 20 140 90""#));
        assert!(svg.contains("<title>T&lt;1&gt;</title>"));
        assert!(svg.contains(r##"<rect x="40" y="40" width="100" height="50" rx="6" fill="#fff" stroke="#000" stroke-width="2"/>"##));
        assert!(svg.contains(r#"font-weight="700">A &amp; B</text>"#));
    }
}
