//! Renders a diagram as an uncompressed draw.io (diagrams.net) file. Containers become draw.io
//! containers holding their children, and arrows become connectors attached to their elements
//! at the same points as in the editor. A port of the prototype's `utils/diagram-drawio.ts`.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::color::readable_text;
use crate::figures::{
    CLOUD_DROP, CLOUD_TEXT_INSET, FIGURE_BADGE_FONT_SIZE, FIGURE_BADGE_GAP,
    FIGURE_BADGE_LINE_HEIGHT, FIGURE_NAME_GAP, PhysicalFigure, database,
};
use crate::geometry::{Point, Rect, fmt_num, round2};
use crate::look::{ArrowLook, DiagramColors, LookElement, NameAt, Shape};
use crate::model::{ConnectionId, ElementId};
use crate::routing::{
    ARROW_TAG_BORDER_WIDTH, Align, CONNECTOR_BOX_FONT_SIZE, RETURN_ARROW_LENGTH, Vertical,
    label_placement, return_arrow_placement,
};

#[allow(clippy::wildcard_imports, reason = "the stencil table")]
use super::stencils::*;
use super::svg::escape_xml;

const TEXT: &str = "#1b1f24";
const BACKGROUND: &str = "#ffffff";
const BOX_FILL: &str = "#ffffff";
/// Where the drawing starts in draw.io, so it isn't against the edge of the page.
pub const ORIGIN: f64 = 40.0;
const RETURN_ARROW_HEIGHT: f64 = 6.0;
const ACTOR_NAME_BOTTOM: f64 = 5.0;
const COMPONENT_NAME_BOTTOM: f64 = 10.0;
const SECTION_NAME_GAP: &str = "&nbsp;&nbsp;";

/// The heading's text sizes and line heights, and the gap below it.
pub const HEADING_NAME_SIZE: f64 = 20.0;
pub const HEADING_NAME_LINE: f64 = 28.0;
pub const HEADING_DESCRIPTION_SIZE: f64 = 14.0;
pub const HEADING_DESCRIPTION_LINE: f64 = 20.0;
pub const HEADING_GLYPH_WIDTH: f64 = 7.5;
pub const HEADING_MIN_WIDTH: f64 = 400.0;
pub const HEADING_GAP: f64 = 20.0;

/// A diagram's name and description, shown in the top-left corner above the drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub name: String,
    pub description: Option<String>,
}

/// An arrow to export, between two elements.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawioArrow {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub look: ArrowLook,
}

/// A draw.io style: key-value pairs in order. Setting a key again keeps its place, as a
/// JavaScript object spread does.
#[derive(Debug, Clone, Default)]
pub struct Style(Vec<(&'static str, String)>);

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        if let Some(entry) = self.0.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.0.push((key, value));
        }
        self
    }

    pub fn num(self, key: &'static str, value: f64) -> Self {
        self.set(key, fmt_num(value))
    }

    pub fn build(&self) -> String {
        self.0
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(";")
    }
}

fn base_style(colors: &DiagramColors) -> Style {
    Style::new()
        .set("html", 1)
        .set("whiteSpace", "wrap")
        .set("strokeColor", &colors.borders)
        .set("strokeWidth", 2)
        .set("fontColor", TEXT)
}

fn rounded(style: Style) -> Style {
    style
        .set("rounded", 1)
        .set("absoluteArcSize", 1)
        .set("arcSize", 12)
}

fn with_spacing(style: Style) -> Style {
    style
        .set("spacingLeft", 10)
        .set("spacingRight", 10)
        .set("spacingTop", 6)
        .set("spacingBottom", 6)
}

/// The geometry attributes of a rectangle.
pub fn rect_attributes(r: Rect) -> String {
    format!(
        r#"x="{}" y="{}" width="{}" height="{}""#,
        fmt_num(round2(r.x)),
        fmt_num(round2(r.y)),
        fmt_num(round2(r.width)),
        fmt_num(round2(r.height))
    )
}

/// A vertex cell. `value` is already escaped for the attribute.
pub fn cell(id: &str, value: &str, style: &str, parent: &str, geometry: &str) -> String {
    format!(
        "        <mxCell id=\"{}\" value=\"{value}\" style=\"{style}\" vertex=\"1\" parent=\"{parent}\">\n          <mxGeometry {geometry} as=\"geometry\" />\n        </mxCell>",
        escape_xml(id)
    )
}

/// The file around `cells`.
pub fn wrap(title: &str, cells: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, r#"<mxfile host="ArchHiero" type="device">"#);
    let _ = writeln!(
        out,
        r#"  <diagram id="archhiero-diagram" name="{}">"#,
        escape_xml(title)
    );
    let _ = writeln!(
        out,
        r#"    <mxGraphModel grid="1" gridSize="20" guides="1" tooltips="1" connect="1" arrows="1" fold="1" page="0" pageScale="1" math="0" shadow="0">"#
    );
    let _ = writeln!(out, "      <root>");
    let _ = writeln!(out, r#"        <mxCell id="0" />"#);
    let _ = writeln!(out, r#"        <mxCell id="1" parent="0" />"#);
    for c in cells {
        let _ = writeln!(out, "{c}");
    }
    let _ = writeln!(out, "      </root>");
    let _ = writeln!(out, "    </mxGraphModel>");
    let _ = writeln!(out, "  </diagram>");
    let _ = writeln!(out, "</mxfile>");
    out
}

/// The heading as a text cell at the origin, `width` wide, and its height.
pub fn heading_cell(heading: &Heading, width: f64) -> (String, f64) {
    let paragraphs: Vec<&str> = heading
        .description
        .as_deref()
        .map(|d| d.split('\n').collect())
        .unwrap_or_default();
    let lines: f64 = paragraphs
        .iter()
        .map(|p| {
            ((p.chars().count() as f64 * HEADING_GLYPH_WIDTH) / width)
                .ceil()
                .max(1.0)
        })
        .sum();
    let height = HEADING_NAME_LINE + lines * HEADING_DESCRIPTION_LINE;
    let name = format!(
        r#"<b style="font-size: {}px">{}</b>"#,
        fmt_num(HEADING_NAME_SIZE),
        escape_xml(&heading.name)
    );
    let label = if paragraphs.is_empty() {
        name
    } else {
        format!(
            "{name}<br>{}",
            paragraphs
                .iter()
                .map(|p| escape_xml(p))
                .collect::<Vec<_>>()
                .join("<br>")
        )
    };
    let style = Style::new()
        .set("text", 1)
        .set("html", 1)
        .set("whiteSpace", "wrap")
        .set("align", "left")
        .set("verticalAlign", "top")
        .set("spacing", 0)
        .set("strokeColor", "none")
        .set("fillColor", "none")
        .set("fontColor", TEXT)
        .num("fontSize", HEADING_DESCRIPTION_SIZE);
    (
        cell(
            "archhiero-heading",
            &escape_xml(&label),
            &style.build(),
            "1",
            &rect_attributes(Rect::new(ORIGIN, ORIGIN, width, height)),
        ),
        height,
    )
}

/// Where an arrow meets an element, as draw.io's connection constraint.
fn constraint(point: Point, element: Rect) -> (f64, f64, f64, f64) {
    let on_side = |v: f64, edge: f64| (v - edge).abs() < 0.01;
    let along = Point::new(round2(point.x) - element.x, round2(point.y) - element.y);
    if on_side(point.x, element.x) || on_side(point.x, element.right()) {
        return (
            if on_side(point.x, element.x) {
                0.0
            } else {
                1.0
            },
            0.0,
            0.0,
            round2(along.y),
        );
    }
    if on_side(point.y, element.y) || on_side(point.y, element.bottom()) {
        return (
            0.0,
            if on_side(point.y, element.y) {
                0.0
            } else {
                1.0
            },
            round2(along.x),
            0.0,
        );
    }
    (0.0, 0.0, round2(along.x), round2(along.y))
}

fn physical_stencil(figure: PhysicalFigure, detail: Option<&str>) -> &'static str {
    let white = detail.is_some_and(|d| d.eq_ignore_ascii_case("#ffffff"));
    let pick = |black: &'static str, w: &'static str| if white { w } else { black };
    match figure {
        PhysicalFigure::User => USER_STENCIL,
        PhysicalFigure::Server => pick(SERVER_STENCIL_BLACK, SERVER_STENCIL_WHITE),
        PhysicalFigure::DatabaseCylinder => PHYSICAL_DATABASE_STENCIL,
        PhysicalFigure::Firewall => pick(FIREWALL_STENCIL_BLACK, FIREWALL_STENCIL_WHITE),
        PhysicalFigure::LoadBalancer => {
            pick(LOAD_BALANCER_STENCIL_BLACK, LOAD_BALANCER_STENCIL_WHITE)
        }
        PhysicalFigure::Gateway => pick(GATEWAY_STENCIL_BLACK, GATEWAY_STENCIL_WHITE),
        PhysicalFigure::StorageArray => STORAGE_ARRAY_STENCIL,
        PhysicalFigure::VirtualMachine => VIRTUAL_MACHINE_STENCIL,
        PhysicalFigure::ContainerStack => CONTAINER_STENCIL,
    }
}

fn box_on_arrow_style(colors: &DiagramColors) -> String {
    rounded(base_style(colors))
        .set("arcSize", 6)
        .set("whiteSpace", "nowrap")
        .set("spacing", 0)
        .num("strokeWidth", 1.5)
        .set("fillColor", &colors.connectors)
        .set("fontColor", readable_text(&colors.connectors))
        .set("fontStyle", 1)
        .num("fontSize", CONNECTOR_BOX_FONT_SIZE)
        .set("resizable", 0)
        .build()
}

/// Renders the diagram. Elements should come before the elements they contain. A `heading`,
/// when given, goes in the top-left corner with the drawing below it.
pub fn diagram_to_drawio(
    title: &str,
    elements: &[LookElement],
    arrows: &[DrawioArrow],
    heading: Option<&Heading>,
    colors: &DiagramColors,
) -> String {
    let by_id: HashMap<&ElementId, &LookElement> = elements.iter().map(|e| (&e.id, e)).collect();
    let top_level: Vec<&LookElement> = elements
        .iter()
        .filter(|e| e.parent_id.as_ref().is_none_or(|p| !by_id.contains_key(p)))
        .collect();
    let min_x = top_level
        .iter()
        .map(|e| e.rect.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = top_level
        .iter()
        .map(|e| e.rect.right())
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = top_level
        .iter()
        .map(|e| e.rect.y)
        .fold(f64::INFINITY, f64::min);
    let (min_x, max_x, min_y) = if top_level.is_empty() {
        (0.0, 0.0, 0.0)
    } else {
        (min_x, max_x, min_y)
    };
    let heading = heading.map(|h| heading_cell(h, (max_x - min_x).max(HEADING_MIN_WIDTH)));
    let offset_x = ORIGIN - min_x;
    let offset_y = ORIGIN + heading.as_ref().map_or(0.0, |(_, h)| h + HEADING_GAP) - min_y;

    let place = |e: &LookElement| -> (f64, f64, String) {
        match e.parent_id.as_ref().and_then(|p| by_id.get(p)) {
            Some(parent) => (
                e.rect.x - parent.rect.x,
                e.rect.y - parent.rect.y,
                escape_xml(parent.id.as_str()),
            ),
            None => (e.rect.x + offset_x, e.rect.y + offset_y, "1".into()),
        }
    };

    let element_cell = |e: &LookElement| -> String {
        let (left, top, parent) = place(e);
        let geometry = rect_attributes(Rect::new(left, top, e.rect.width, e.rect.height));
        let look = &e.look;
        match look.shape {
            Shape::Actor | Shape::Database | Shape::Storage | Shape::File => {
                let stencil = match look.shape {
                    Shape::Actor if look.agent => AGENT_STENCIL,
                    Shape::Actor => ACTOR_STENCIL,
                    Shape::Database => DATABASE_STENCIL,
                    Shape::File => FILE_STENCIL,
                    _ => STORAGE_STENCIL,
                };
                let style = base_style(colors)
                    .set("shape", format!("stencil({stencil})"))
                    .set("fillColor", look.fill.as_deref().unwrap_or(BOX_FILL))
                    .set("align", "center")
                    .set("verticalAlign", "bottom")
                    .num(
                        "spacingBottom",
                        if look.shape == Shape::Actor {
                            ACTOR_NAME_BOTTOM
                        } else {
                            COMPONENT_NAME_BOTTOM
                        },
                    )
                    .set("fontStyle", 1)
                    .set("fontSize", 16);
                cell(
                    e.id.as_str(),
                    &escape_xml(&e.label),
                    &style.build(),
                    &parent,
                    &geometry,
                )
            }
            Shape::Physical(figure) => {
                let style = base_style(colors)
                    .set(
                        "shape",
                        format!(
                            "stencil({})",
                            physical_stencil(figure, look.detail_color.as_deref())
                        ),
                    )
                    .set("fillColor", look.fill.as_deref().unwrap_or(BOX_FILL))
                    .set("align", "center")
                    .set("verticalAlign", "top")
                    .num("spacingTop", figure.size().height + FIGURE_NAME_GAP)
                    .set("fontColor", look.text_color.as_deref().unwrap_or(TEXT))
                    .set("fontStyle", 1)
                    .set("fontSize", 16);
                let mut out = cell(
                    e.id.as_str(),
                    &escape_xml(&e.label),
                    &style.build(),
                    &parent,
                    &geometry,
                );
                if let (Some(text), PhysicalFigure::DatabaseCylinder) = (&look.figure_text, figure)
                {
                    let side_top = database::CYLINDER_TOP_CENTER.y + database::CYLINDER_TOP_RY;
                    let size = figure.size();
                    let text_style = Style::new()
                        .set("text", "")
                        .set("html", 1)
                        .set("align", "center")
                        .set("verticalAlign", "middle")
                        .set("fillColor", "none")
                        .set("strokeColor", "none")
                        .set(
                            "fontColor",
                            look.detail_color.as_deref().unwrap_or("#000000"),
                        )
                        .set("fontStyle", 1)
                        .num("fontSize", database::TEXT_SIZE)
                        .set("spacing", 0)
                        .set("pointerEvents", 0);
                    out.push('\n');
                    out.push_str(&cell(
                        &format!("{}-text", e.id),
                        &escape_xml(text),
                        &text_style.build(),
                        &parent,
                        &rect_attributes(Rect::new(
                            left + (e.rect.width - size.width) / 2.0,
                            top + side_top,
                            size.width,
                            size.height - side_top,
                        )),
                    ));
                }
                out
            }
            Shape::Section => {
                let style = base_style(colors)
                    .set("shape", format!("stencil({SECTION_STENCIL})"))
                    .set("strokeWidth", 3)
                    .set("fillColor", "none")
                    .set("labelBackgroundColor", BACKGROUND)
                    .set("fontColor", look.text_color.as_deref().unwrap_or(TEXT))
                    .set("fontStyle", 1)
                    .set("fontSize", 20)
                    .set("whiteSpace", "nowrap");
                let name = format!(
                    "{SECTION_NAME_GAP}{}{SECTION_NAME_GAP}",
                    escape_xml(&e.label)
                );
                cell(
                    e.id.as_str(),
                    &escape_xml(&name),
                    &style.build(),
                    &parent,
                    &geometry,
                )
            }
            Shape::Container => {
                let mut style = base_style(colors);
                style = if look.square {
                    style.set("rounded", 0)
                } else {
                    rounded(style)
                };
                style = style
                    .set("container", 1)
                    .set("collapsible", 0)
                    .num("strokeWidth", look.border_width.unwrap_or(3.0));
                if let Some(border) = &look.border_color {
                    style = style.set("strokeColor", border);
                }
                if look.dashed {
                    style = style.set("dashed", 1).set("dashPattern", "4 2");
                }
                style = style
                    .set("fillColor", look.fill.as_deref().unwrap_or(&colors.domains))
                    .set("fontColor", look.text_color.as_deref().unwrap_or(TEXT));
                if look.name_at != NameAt::TopCenter {
                    style = style.set("align", "left").set("spacingLeft", 10);
                }
                let style = style
                    .set(
                        "verticalAlign",
                        if look.name_at == NameAt::BottomLeft {
                            "bottom"
                        } else {
                            "top"
                        },
                    )
                    .set("fontStyle", 1)
                    .set(
                        "fontSize",
                        if look.name_at == NameAt::BottomLeft {
                            16
                        } else {
                            20
                        },
                    );
                cell(
                    e.id.as_str(),
                    &escape_xml(&e.label),
                    &style.build(),
                    &parent,
                    &geometry,
                )
            }
            Shape::Box | Shape::Cloud => {
                let name = format!("<b>{}</b>", escape_xml(&e.label));
                let label = match e.description.as_deref().filter(|d| !d.is_empty()) {
                    Some(d) => format!(
                        r#"{name}<br><span style="font-size: 13px">{}</span>"#,
                        escape_xml(d)
                    ),
                    None => name,
                };
                let inset = round2(e.rect.width * CLOUD_TEXT_INSET);
                let mut style = base_style(colors);
                style = if look.shape == Shape::Cloud {
                    with_spacing(style.set("shape", format!("stencil({CLOUD_STENCIL})")))
                        .num("spacingLeft", inset)
                        .num("spacingRight", inset)
                        .num("spacingTop", round2(6.0 + e.rect.height * CLOUD_DROP * 2.0))
                } else {
                    with_spacing(rounded(style))
                };
                if look.text_at_top {
                    style = style
                        .set("container", 1)
                        .set("collapsible", 0)
                        .set("verticalAlign", "top");
                }
                let style = style
                    .set("fillColor", look.fill.as_deref().unwrap_or(BOX_FILL))
                    .set("fontColor", look.text_color.as_deref().unwrap_or(TEXT))
                    .set("fontSize", 16);
                cell(
                    e.id.as_str(),
                    &escape_xml(&label),
                    &style.build(),
                    &parent,
                    &geometry,
                )
            }
        }
    };

    let mut cells: Vec<String> = Vec::new();
    if let Some((xml, _)) = &heading {
        cells.push(xml.clone());
    }
    cells.extend(elements.iter().filter(|e| !e.look.on_top).map(element_cell));
    for e in elements.iter().filter(|e| e.look.outline_on_top) {
        let (left, top, parent) = place(e);
        let style = if e.look.square {
            Style::new().set("rounded", 0)
        } else {
            rounded(Style::new())
        }
        .set("fillColor", "none")
        .set("strokeColor", &colors.borders)
        .num("strokeWidth", e.look.border_width.unwrap_or(3.0))
        .set("pointerEvents", 0);
        cells.push(cell(
            &format!("{}-outline", e.id),
            "",
            &style.build(),
            &parent,
            &rect_attributes(Rect::new(left, top, e.rect.width, e.rect.height)),
        ));
    }
    cells.extend(elements.iter().filter(|e| e.look.on_top).map(element_cell));

    let box_style = box_on_arrow_style(colors);
    for arrow in arrows {
        let (Some(source), Some(target)) =
            (by_id.get(&arrow.source_id), by_id.get(&arrow.target_id))
        else {
            continue;
        };
        let a = &arrow.look;
        let mut style = Style::new()
            .set("html", 1)
            .set("rounded", 0)
            .set("endArrow", "block")
            .set("endFill", 1)
            .set("strokeColor", &colors.lines)
            .set("strokeWidth", 2);
        if a.dashed {
            style = style.set("dashed", 1).set("dashPattern", "4 2 4 2");
        }
        let exit = constraint(a.from, source.rect);
        let entry = constraint(a.to, a.connector.as_ref().map_or(target.rect, |(_, r)| *r));
        let connector_id = format!("{}-connector", arrow.id);
        let target_id = if a.connector.is_some() {
            connector_id.clone()
        } else {
            target.id.0.clone()
        };
        style = style
            .num("exitX", exit.0)
            .num("exitY", exit.1)
            .num("exitDx", exit.2)
            .num("exitDy", exit.3)
            .set("exitPerimeter", 0)
            .num("entryX", entry.0)
            .num("entryY", entry.1)
            .num("entryDx", entry.2)
            .num("entryDy", entry.3)
            .set("entryPerimeter", 0);
        let mut value = String::new();
        let mut geometry = r#"<mxGeometry relative="1" as="geometry" />"#.to_owned();
        if let Some(label) = a.label.as_ref().filter(|l| !l.is_empty()) {
            let p = label_placement(a.from, a.to);
            style = style
                .set("fontSize", 13)
                .set("fontColor", TEXT)
                .set("labelBackgroundColor", "none")
                .set(
                    "align",
                    if p.align == Align::Start {
                        "left"
                    } else {
                        "right"
                    },
                )
                .set(
                    "verticalAlign",
                    if p.vertical == Vertical::Top {
                        "top"
                    } else {
                        "bottom"
                    },
                );
            value = format!(r#" value="{}""#, escape_xml(&escape_xml(label)));
            geometry = format!(
                r#"<mxGeometry x="-1" relative="1" as="geometry"><mxPoint x="{}" y="{}" as="offset" /></mxGeometry>"#,
                fmt_num(round2(p.anchor.x - a.from.x)),
                fmt_num(round2(p.anchor.y - a.from.y))
            );
        }
        if let Some((label, r)) = &a.connector {
            cells.push(cell(
                &connector_id,
                &escape_xml(&escape_xml(label)),
                &box_style,
                &escape_xml(target.id.as_str()),
                &rect_attributes(Rect::new(
                    r.x - target.rect.x,
                    r.y - target.rect.y,
                    r.width,
                    r.height,
                )),
            ));
        }
        cells.push(format!(
            "        <mxCell id=\"{}\"{value} style=\"{}\" edge=\"1\" parent=\"1\" source=\"{}\" target=\"{}\">\n          {geometry}\n        </mxCell>",
            escape_xml(arrow.id.as_str()),
            style.build(),
            escape_xml(source.id.as_str()),
            escape_xml(&target_id)
        ));
        if a.bidirectional
            && let Some((from, to)) = return_arrow_placement(a.from, a.to)
        {
            let center = Point::new(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y));
            let rotation = round2((a.to.y - a.from.y).atan2(a.to.x - a.from.x).to_degrees());
            let arrow_style = Style::new()
                .set("shape", format!("stencil({RETURN_ARROW_STENCIL})"))
                .num("rotation", rotation)
                .set("strokeColor", &colors.lines)
                .set("fillColor", &colors.lines)
                .num("strokeWidth", 1.5);
            cells.push(format!(
                "        <mxCell id=\"{}-return\" value=\"\" style=\"{}\" vertex=\"1\" connectable=\"0\" parent=\"{}\">\n          <mxGeometry x=\"-1\" width=\"{}\" height=\"{}\" relative=\"1\" as=\"geometry\"><mxPoint x=\"{}\" y=\"{}\" as=\"offset\" /></mxGeometry>\n        </mxCell>",
                escape_xml(arrow.id.as_str()),
                arrow_style.build(),
                escape_xml(arrow.id.as_str()),
                fmt_num(RETURN_ARROW_LENGTH),
                fmt_num(RETURN_ARROW_HEIGHT),
                fmt_num(round2(center.x - RETURN_ARROW_LENGTH / 2.0 - a.from.x)),
                fmt_num(round2(center.y - RETURN_ARROW_HEIGHT / 2.0 - a.from.y))
            ));
        }
        let attached = |id: String, label: &str, style: &str, r: &Rect| {
            format!(
                "        <mxCell id=\"{id}\" value=\"{}\" style=\"{style}\" vertex=\"1\" parent=\"{}\">\n          <mxGeometry x=\"0\" width=\"{}\" height=\"{}\" relative=\"1\" as=\"geometry\"><mxPoint x=\"{}\" y=\"{}\" as=\"offset\" /></mxGeometry>\n        </mxCell>",
                escape_xml(&escape_xml(label)),
                escape_xml(arrow.id.as_str()),
                fmt_num(r.width),
                fmt_num(r.height),
                fmt_num(round2(-r.width / 2.0)),
                fmt_num(round2(-r.height / 2.0))
            )
        };
        if let Some((label, r)) = &a.transformation {
            cells.push(attached(
                format!("{}-transformation", escape_xml(arrow.id.as_str())),
                label,
                &format!("{box_style};connectable=0"),
                r,
            ));
        }
        if let Some(tag) = &a.tag {
            let mut style = rounded(Style::new().set("html", 1))
                .set("arcSize", 6)
                .set("whiteSpace", "nowrap")
                .set("spacing", 0);
            style = match &tag.border {
                Some(b) => style
                    .set("strokeColor", b)
                    .num("strokeWidth", ARROW_TAG_BORDER_WIDTH),
                None => style.set("strokeColor", "none"),
            };
            let style = style
                .set("fillColor", &tag.fill)
                .set("fontColor", &tag.text_color)
                .set("fontStyle", 1)
                .num("fontSize", CONNECTOR_BOX_FONT_SIZE)
                .set("resizable", 0)
                .set("connectable", 0);
            cells.push(attached(
                format!("{}-tag", escape_xml(arrow.id.as_str())),
                &tag.label,
                &style.build(),
                &tag.rect,
            ));
        }
    }
    // Over the arrows, so they don't move to make room.
    for e in elements {
        let (Some(badge), Shape::Physical(figure)) = (&e.look.figure_badge, e.look.shape) else {
            continue;
        };
        let (left, top, parent) = place(e);
        let size = figure.size();
        let style = Style::new()
            .set("text", "")
            .set("html", 1)
            .set("align", "left")
            .set("verticalAlign", "middle")
            .set("fillColor", "none")
            .set("strokeColor", "none")
            .set("fontColor", e.look.text_color.as_deref().unwrap_or(TEXT))
            .set("fontStyle", 1)
            .num("fontSize", FIGURE_BADGE_FONT_SIZE)
            .set("spacing", 0)
            .set("pointerEvents", 0);
        cells.push(cell(
            &format!("{}-badge", e.id),
            &escape_xml(badge),
            &style.build(),
            &parent,
            &rect_attributes(Rect::new(
                left + (e.rect.width + size.width) / 2.0 + FIGURE_BADGE_GAP,
                top + (size.height - FIGURE_BADGE_LINE_HEIGHT) / 2.0,
                48.0,
                FIGURE_BADGE_LINE_HEIGHT,
            )),
        ));
    }
    wrap(title, &cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::Look;

    fn element(id: &str, parent: Option<&str>, shape: Shape, rect: Rect) -> LookElement {
        LookElement {
            id: id.into(),
            parent_id: parent.map(Into::into),
            label: format!("{id} & co"),
            description: None,
            look: Look::new(shape),
            rect,
        }
    }

    #[test]
    fn writes_containers_children_and_attached_arrows() {
        let elements = vec![
            element(
                "d",
                None,
                Shape::Container,
                Rect::new(40.0, 40.0, 480.0, 180.0),
            ),
            element(
                "a",
                Some("d"),
                Shape::Box,
                Rect::new(60.0, 80.0, 200.0, 100.0),
            ),
            element("b", None, Shape::Box, Rect::new(600.0, 80.0, 200.0, 100.0)),
        ];
        let arrows = vec![DrawioArrow {
            id: "c".into(),
            source_id: "a".into(),
            target_id: "b".into(),
            look: ArrowLook {
                from: Point::new(260.0, 130.0),
                to: Point::new(568.0, 130.0),
                label: Some("Orders".into()),
                connector: Some(("API".into(), Rect::new(568.0, 120.0, 32.0, 20.0))),
                ..ArrowLook::default()
            },
        }];
        let heading = Heading {
            name: "Overview".into(),
            description: Some("All of it".into()),
        };
        let xml = diagram_to_drawio(
            "T",
            &elements,
            &arrows,
            Some(&heading),
            &DiagramColors::default(),
        );
        assert!(xml.starts_with(r#"<mxfile host="ArchHiero" type="device">"#));
        // The child is placed relative to its container.
        assert!(xml.contains(r#"id="a" value="&lt;b&gt;a &amp;amp; co&lt;/b&gt;""#));
        assert!(xml.contains(r#"parent="d">"#));
        assert!(xml.contains(r#"x="20" y="40" width="200" height="100""#));
        // The arrow ends at its connector box.
        assert!(xml.contains(r#"source="a" target="c-connector""#));
        assert!(xml.contains("exitX=1;exitY=0;exitDx=0;exitDy=50"));
        assert!(xml.contains(r#"id="archhiero-heading""#));
        // The drawing sits below the heading: 40 + 48 + 20.
        assert!(
            xml.contains(r#"id="d" value="d &amp;amp; co""#)
                || xml.contains(r#"id="d" value="d &amp; co""#)
        );
        assert!(xml.contains(r#"x="40" y="108" width="480" height="180""#));
    }

    #[test]
    fn styles_keep_the_first_place_of_a_key() {
        let s = Style::new().set("a", 1).set("b", 2).set("a", 3).build();
        assert_eq!(s, "a=3;b=2");
    }
}
