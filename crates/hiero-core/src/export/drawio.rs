//! Renders a diagram as an uncompressed draw.io (diagrams.net) file. Containers become draw.io
//! containers holding their children, and arrows become connectors attached to their elements
//! at the same points as in the editor. A port of the prototype's `utils/diagram-drawio.ts`.

use base64::Engine as _;
use std::collections::HashMap;
use std::fmt::Write as _;

use crate::color::readable_text;
use crate::figures::{CLOUD_DROP, CLOUD_TEXT_INSET, FIGURE_NAME_GAP, PhysicalFigure, database};
use crate::geometry::{Point, Rect, Size, fmt_num, round2};
use crate::legend::{
    LEGEND_BAND, LEGEND_BORDER, LEGEND_COLUMN, LEGEND_FOOTER, LEGEND_GAP, LEGEND_HEADER,
    LEGEND_MUTED, LEGEND_PADDING, LEGEND_ROW, LEGEND_WIDTH,
};
pub use crate::legend::{Legend, LegendLine, Stamp};
use crate::look::{ArrowLook, DiagramColors, LookElement, NameAt, Shape};
use crate::model::{ConnectionId, ElementId, LineWeight};
use crate::routing::{
    Align, CONNECTOR_BOX_FONT_SIZE, RETURN_ARROW_LENGTH, Vertical, label_placement,
    return_arrow_placement,
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
/// Space above an actor's name: the figure, a little way down the area, and the gap under it,
/// so the name sits under the figure and a second line goes below the first.
const ACTOR_NAME_TOP: f64 = 55.0;
/// Space above a database's, storage's, or file's name. Their figures sit at the top of the
/// area, so the name sits right under the figure.
const COMPONENT_NAME_TOP: f64 = 50.0;
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

impl Legend {
    /// The legend's cells, with its top-left corner at `at`, its lines drawn in `line_color`.
    pub fn cells(&self, at: Point, line_color: &str) -> Vec<String> {
        let (x, y, w, h) = (at.x, at.y, LEGEND_WIDTH, self.height());
        let text =
            |id: &str, r: Rect, value: &str, size: f64, align: &str, color: &str, bold: bool| {
                let style = Style::new()
                    .set("text", 1)
                    .set("html", 1)
                    .set("whiteSpace", "wrap")
                    .set("align", align)
                    .set("verticalAlign", "middle")
                    .set("spacing", 0)
                    .set("strokeColor", "none")
                    .set("fillColor", "none")
                    .set("fontColor", color)
                    .num("fontSize", size)
                    .set("fontStyle", u8::from(bold));
                cell(
                    id,
                    &escape_xml(&escape_xml(value)),
                    &style.build(),
                    "1",
                    &rect_attributes(r),
                )
            };
        let band = |id: &str, top: f64, height: f64, value: &str| {
            let style = Style::new()
                .set("html", 1)
                .set("rounded", 0)
                .set("fillColor", LEGEND_BAND)
                .set("strokeColor", "none")
                .set("fontColor", TEXT)
                .set("fontSize", 14)
                .set("fontStyle", 1);
            cell(
                id,
                &escape_xml(value),
                &style.build(),
                "1",
                &rect_attributes(Rect::new(x + 1.0, top, w - 2.0, height)),
            )
        };
        let frame = rounded(Style::new().set("html", 1))
            .set("fillColor", BACKGROUND)
            .set("strokeColor", LEGEND_BORDER)
            .set("strokeWidth", 1);
        let mut cells = vec![
            cell(
                "legend",
                "",
                &frame.build(),
                "1",
                &rect_attributes(Rect::new(x, y, w, h)),
            ),
            band("legend-title", y + 1.0, LEGEND_HEADER - 1.0, "Legend"),
            band(
                "legend-footer",
                y + h - LEGEND_FOOTER,
                LEGEND_FOOTER - 1.0,
                "",
            ),
        ];
        let mut top = y + LEGEND_HEADER + 12.0;
        for (i, l) in self.lines.iter().enumerate() {
            let left = x + LEGEND_PADDING + i as f64 * LEGEND_COLUMN;
            let mut style = Style::new()
                .set("html", 1)
                .set("rounded", 0)
                .set("endArrow", "block")
                .set("endFill", 1)
                .set("endSize", 5)
                .set("strokeColor", line_color)
                .set("strokeWidth", 2);
            if l.dashed {
                style = style.set("dashed", 1).set("dashPattern", "4 2 4 2");
            }
            let middle = fmt_num(round2(top + 9.0));
            cells.push(format!(
                "        <mxCell id=\"legend-line-{i}\" value=\"\" style=\"{}\" edge=\"1\" parent=\"1\">\n          <mxGeometry relative=\"1\" as=\"geometry\">\n            <mxPoint x=\"{}\" y=\"{middle}\" as=\"sourcePoint\" />\n            <mxPoint x=\"{}\" y=\"{middle}\" as=\"targetPoint\" />\n          </mxGeometry>\n        </mxCell>",
                style.build(),
                fmt_num(round2(left)),
                fmt_num(round2(left + 36.0)),
            ));
            cells.push(text(
                &format!("legend-line-{i}-label"),
                Rect::new(left + 44.0, top, LEGEND_COLUMN - 46.0, 18.0),
                &l.label,
                11.0,
                "left",
                TEXT,
                false,
            ));
        }
        if !self.lines.is_empty() {
            top += 32.0;
        }
        for (i, (label, fill)) in self.swatches.iter().enumerate() {
            let left = x + LEGEND_PADDING + (i % 2) as f64 * LEGEND_COLUMN;
            let row = top + (i / 2) as f64 * LEGEND_ROW;
            let style = Style::new()
                .set("html", 1)
                .set("rounded", 0)
                .set("fillColor", fill)
                .set("strokeColor", crate::color::border_for(fill))
                .set("strokeWidth", 1);
            cells.push(cell(
                &format!("legend-swatch-{i}"),
                "",
                &style.build(),
                "1",
                &rect_attributes(Rect::new(left, row, 14.0, 14.0)),
            ));
            cells.push(text(
                &format!("legend-swatch-{i}-label"),
                Rect::new(left + 22.0, row - 2.0, LEGEND_COLUMN - 24.0, 18.0),
                label,
                11.0,
                "left",
                TEXT,
                false,
            ));
        }
        let footer = y + h - LEGEND_FOOTER + 7.0;
        let half = w / 2.0 - LEGEND_PADDING;
        cells.push(text(
            "legend-generated",
            Rect::new(x + LEGEND_PADDING, footer, half, 16.0),
            &format!("Generated {}", self.stamp.generated),
            10.0,
            "left",
            LEGEND_MUTED,
            false,
        ));
        cells.push(text(
            "legend-version",
            Rect::new(x + w / 2.0, footer, half, 16.0),
            &self.stamp.version,
            10.0,
            "right",
            TEXT,
            true,
        ));
        cells
    }
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

fn base_style(colors: &DiagramColors, weight: LineWeight) -> Style {
    Style::new()
        .set("html", 1)
        .set("whiteSpace", "wrap")
        .set("strokeColor", &colors.borders)
        .num("strokeWidth", weight.width())
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

/// The style of an actor, database, storage, or file figure with its name under it, in
/// `look`'s fill and border, as both the diagram export and a sequence diagram's use.
pub fn figure_style(look: &crate::look::Look, colors: &DiagramColors, weight: LineWeight) -> Style {
    let stencil = match look.shape {
        Shape::Actor if look.agent => AGENT_STENCIL,
        Shape::Actor => ACTOR_STENCIL,
        Shape::Database => DATABASE_STENCIL,
        Shape::File => FILE_STENCIL,
        _ => STORAGE_STENCIL,
    };
    let style = base_style(colors, weight);
    let style = match &look.border_color {
        Some(border) => style.set("strokeColor", border),
        None => style,
    };
    style
        .set("shape", format!("stencil({stencil})"))
        .set("fillColor", look.fill.as_deref().unwrap_or(BOX_FILL))
        .set("align", "center")
        .set("verticalAlign", "top")
        .num(
            "spacingTop",
            if look.shape == Shape::Actor {
                ACTOR_NAME_TOP
            } else {
                COMPONENT_NAME_TOP
            },
        )
        .set("fontStyle", 1)
        .set("fontSize", 16)
}

/// A person on their own, without the room for a name the actor stencil has: the actor figure
/// in its own 48 × 46 box, `figures::actor`, with its proportions kept. Compressed as draw.io
/// stores stencils, from:
///
/// ```xml
/// <shape name="ArchHiero person" w="48" h="46" aspect="fixed" strokewidth="inherit">
///   <connections/><background><ellipse x="14" y="2" w="20" h="20"/></background>
///   <foreground><fillstroke/><path><move x="4" y="45"/><arc rx="20" ry="20"
///   x-axis-rotation="0" large-arc-flag="0" sweep-flag="1" x="44" y="45"/><close/></path>
///   <fillstroke/></foreground></shape>
/// ```
const PERSON_STENCIL: &str = "fZLBbsMgEES/hqOlBNyq1ypNld+geG2jEBYttCZ/X9Ci1kRVbzszZnisLNQprjqAkAevbyDUm5Dylcx6sUBY3AAU0RezzBvH4wvLtclnljoGMIm92WaY2I6J8AqbnVL73voVyKaaqrNQJ4Pel4MWfRTynb0Pba4L4aefWINzNsRKmbnkOHL7naXsAOWhA2yyVZfhsX1Ggk5b5xj751TQFb9ON/zacfQY49P+Jk2mRJR7KLr3Og862zgQJl13wGHLnKYFhtIzzE4vXRQ3gLCzj62tgfzHZRxG2O/j93F/vLwMj/spFv806vwN";

/// The draw.io shape of a person drawn in a cell the size of `figures::actor`, as `shape=`
/// takes it, such as each of a high level diagram's actors.
pub fn person_shape() -> String {
    format!("stencil({PERSON_STENCIL})")
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

/// The file around `cells`, a drawing `size` big starting at the origin. draw.io's page view is
/// on, with one page holding the drawing and a margin as wide as the origin all round, so
/// zooming out shows the page rather than an endless canvas.
pub fn wrap(title: &str, size: Size, cells: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, r#"<mxfile host="ArchHiero" type="device">"#);
    let _ = writeln!(
        out,
        r#"  <diagram id="{FIRST_PAGE_ID}" name="{}">"#,
        escape_xml(title)
    );
    let _ = writeln!(
        out,
        r#"    <mxGraphModel grid="1" gridSize="20" guides="1" tooltips="1" connect="1" arrows="1" fold="1" page="1" pageScale="1" pageWidth="{}" pageHeight="{}" math="0" shadow="0">"#,
        fmt_num((size.width + ORIGIN * 2.0).ceil()),
        fmt_num((size.height + ORIGIN * 2.0).ceil()),
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

/// The id of a single diagram's page, and of the first page of a file with several.
pub const FIRST_PAGE_ID: &str = "archhiero-diagram";

/// A page of a draw.io file with several: a whole file from [`wrap`], its page's id, and the
/// elements that link to another page, with that page's id.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawioPage {
    pub id: String,
    pub file: String,
    pub links: Vec<(ElementId, String)>,
}

/// One draw.io file holding each page in turn, with the links made: each linked element
/// links to its page.
pub fn combine_pages(pages: &[DrawioPage]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, r#"<mxfile host="ArchHiero" type="device">"#);
    for page in pages {
        let start = page.file.find("  <diagram ").unwrap_or(0);
        let end = page
            .file
            .rfind("  </diagram>")
            .map_or(page.file.len(), |i| i + "  </diagram>\n".len());
        let mut body = page.file[start..end.min(page.file.len())].replacen(
            &format!(r#"id="{FIRST_PAGE_ID}""#),
            &format!(r#"id="{}""#, escape_xml(&page.id)),
            1,
        );
        for (element, target) in &page.links {
            body = link_cell(&body, element, target);
        }
        out.push_str(&body);
    }
    let _ = writeln!(out, "</mxfile>");
    out
}

/// Links an element to the page `target`: its cell becomes a draw.io `UserObject` with the
/// link. Its text stays as it is, so draw.io offers the one link.
fn link_cell(body: &str, element: &ElementId, target: &str) -> String {
    let own = format!(r#"<mxCell id="{}" value=""#, escape_xml(element.as_str()));
    let Some(start) = body.find(&own) else {
        return body.to_owned();
    };
    let Some(end) = body[start..].find("</mxCell>").map(|i| start + i) else {
        return body.to_owned();
    };
    let cell = &body[start..end];
    let attribute = |name: &str| -> Option<(usize, usize)> {
        let key = format!(r#" {name}=""#);
        let from = cell.find(&key)? + key.len();
        Some((from, from + cell[from..].find('"')?))
    };
    let (Some((id_from, id_to)), Some((value_from, value_to)), Some((style_from, style_to))) =
        (attribute("id"), attribute("value"), attribute("style"))
    else {
        return body.to_owned();
    };
    let link = format!("data:page/id,{target}");
    let label = &cell[value_from..value_to];
    let style = &cell[style_from..style_to];
    let rest = &cell[style_to + 1..];
    format!(
        r#"{}<UserObject id="{}" label="{label}" link="{}">
          <mxCell style="{style}"{rest}</mxCell>
        </UserObject>{}"#,
        &body[..start],
        &cell[id_from..id_to],
        escape_xml(&link),
        &body[end + "</mxCell>".len()..]
    )
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

/// Where an arrow meets an element, as draw.io's connection constraint: `(x, y, dx, dy)`, for
/// `exitX`, `exitY`, `exitDx`, and `exitDy` (or the `entry` ones). A point off the element's
/// sides is kept as an offset from its top-left corner.
pub fn constraint(point: Point, element: Rect) -> (f64, f64, f64, f64) {
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
        PhysicalFigure::LowFirewall => pick(LOW_FIREWALL_STENCIL_BLACK, LOW_FIREWALL_STENCIL_WHITE),
        PhysicalFigure::LoadBalancer => {
            pick(LOAD_BALANCER_STENCIL_BLACK, LOAD_BALANCER_STENCIL_WHITE)
        }
        PhysicalFigure::Gateway => pick(GATEWAY_STENCIL_BLACK, GATEWAY_STENCIL_WHITE),
        PhysicalFigure::StorageArray => STORAGE_ARRAY_STENCIL,
        PhysicalFigure::VirtualMachine => VIRTUAL_MACHINE_STENCIL,
        PhysicalFigure::ContainerStack => CONTAINER_STENCIL,
    }
}

/// `stencil`, a draw.io stencil with black details, with its details in `color` instead. A
/// stencil is its shape's XML, URL-encoded, deflated, and in base64.
fn recolored_stencil(stencil: &str, color: &str) -> String {
    use std::io::{Read as _, Write as _};
    let engine = &base64::engine::general_purpose::STANDARD;
    let Ok(packed) = engine.decode(stencil) else {
        return stencil.to_owned();
    };
    let mut xml = String::new();
    if flate2::read::DeflateDecoder::new(packed.as_slice())
        .read_to_string(&mut xml)
        .is_err()
    {
        return stencil.to_owned();
    }
    // `#` is `%23` in the URL-encoded XML.
    let xml = xml.replace(
        "%23000000",
        &format!("%23{}", color.trim_start_matches('#')),
    );
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    match encoder
        .write_all(xml.as_bytes())
        .and_then(|()| encoder.finish())
    {
        Ok(packed) => engine.encode(packed),
        Err(_) => stencil.to_owned(),
    }
}

/// A physical figure's stencil: its details in `border` when it has one, else black or white
/// by `detail`.
fn physical_figure_stencil(
    figure: PhysicalFigure,
    detail: Option<&str>,
    border: Option<&str>,
) -> String {
    let black = physical_stencil(figure, None);
    match border {
        Some(border) if black != physical_stencil(figure, Some("#ffffff")) => {
            recolored_stencil(black, border)
        }
        _ => physical_stencil(figure, detail).to_owned(),
    }
}

fn box_on_arrow_style(colors: &DiagramColors, weight: LineWeight) -> String {
    rounded(base_style(colors, weight))
        .set("arcSize", 6)
        .set("whiteSpace", "nowrap")
        .set("spacing", 0)
        .num("strokeWidth", weight.width())
        .set("fillColor", &colors.connectors)
        .set("fontColor", readable_text(&colors.connectors))
        .set("fontStyle", 1)
        .num("fontSize", CONNECTOR_BOX_FONT_SIZE)
        .set("resizable", 0)
        .build()
}

/// Renders the diagram, with elements' lines as thick as `weight`. Elements should come before
/// the elements they contain. A `heading`, when given, goes in the top-left corner with the
/// drawing below it.
pub fn diagram_to_drawio(
    title: &str,
    elements: &[LookElement],
    arrows: &[DrawioArrow],
    heading: Option<&Heading>,
    legend: Option<&Legend>,
    colors: &DiagramColors,
    weight: LineWeight,
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
    let bottom = top_level
        .iter()
        .map(|e| e.rect.bottom())
        .fold(f64::NEG_INFINITY, f64::max);
    let (min_x, max_x, min_y, bottom) = if top_level.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (min_x, max_x, min_y, bottom)
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
        // The diagram's base style, with the element's own border colour when it has one.
        let own_border = || {
            let style = base_style(colors, weight);
            match &look.border_color {
                Some(border) => style.set("strokeColor", border),
                None => style,
            }
        };
        match look.shape {
            Shape::Actor | Shape::Database | Shape::Storage | Shape::File => {
                let style = figure_style(look, colors, weight);
                cell(
                    e.id.as_str(),
                    &escape_xml(&e.label),
                    &style.build(),
                    &parent,
                    &geometry,
                )
            }
            Shape::Physical(figure) => {
                let style = own_border()
                    .set(
                        "shape",
                        format!(
                            "stencil({})",
                            physical_figure_stencil(
                                figure,
                                look.detail_color.as_deref(),
                                look.border_color.as_deref()
                            )
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
            Shape::C4(shape) => {
                crate::c4::drawio_cells(e, shape, (left, top), &parent, &colors.borders, weight)
                    .join("\n")
            }
            Shape::Flow(shape) => crate::flowchart::drawio_cell(
                e,
                shape,
                (left, top),
                &parent,
                &colors.borders,
                weight,
            ),
            Shape::Section => {
                let style = base_style(colors, weight)
                    .set("shape", format!("stencil({SECTION_STENCIL})"))
                    .num("strokeWidth", weight.width())
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
                let mut style = base_style(colors, weight);
                style = if look.square {
                    style.set("rounded", 0)
                } else {
                    rounded(style)
                };
                style = style
                    .set("container", 1)
                    .set("collapsible", 0)
                    .num("strokeWidth", weight.width());
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
                // A bold name, with any description under it in a smaller regular font, keeping
                // its line breaks.
                let name = format!("<b>{}</b>", escape_xml(&e.label));
                let description = e.description.as_deref().map(str::trim).unwrap_or_default();
                let label = if description.is_empty() {
                    name
                } else {
                    let lines: Vec<String> = description.lines().map(escape_xml).collect();
                    format!(
                        r#"{name}<br><span style="font-size: 13px">{}</span>"#,
                        lines.join("<br>")
                    )
                };
                let inset = round2(e.rect.width * CLOUD_TEXT_INSET);
                let mut style = own_border();
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
        .num("strokeWidth", weight.width())
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

    let box_style = box_on_arrow_style(colors, weight);
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
        if a.annotation {
            style = style
                .set("dashed", 1)
                .set("dashPattern", "1 2")
                .set("endArrow", "none");
        }
        if a.two_heads {
            style = style.set("startArrow", "block").set("startFill", 1);
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
        if let Some(caption) = a.caption.as_ref().filter(|c| !c.is_empty()) {
            let mut html: Vec<String> = caption.lines.iter().map(|l| escape_xml(l)).collect();
            if let Some(t) = &caption.technology {
                html.push(format!(
                    r#"<span style="font-size: 11px">{}</span>"#,
                    escape_xml(t)
                ));
            }
            style = style
                .set("fontSize", 13)
                .set("fontColor", TEXT)
                .set("labelBackgroundColor", BACKGROUND);
            value = format!(r#" value="{}""#, escape_xml(&html.join("<br>")));
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
                    .num("strokeWidth", weight.width()),
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
    // Over the arrows, so they don't move to make room. A box on the figure, as an arrow's tag
    // but with round ends.
    for e in elements {
        let (Some(badge), Shape::Physical(_)) = (&e.look.figure_badge, e.look.shape) else {
            continue;
        };
        let (left, top, parent) = place(e);
        let mut style = rounded(Style::new().set("html", 1))
            .set("arcSize", 50)
            .set("whiteSpace", "nowrap")
            .set("spacing", 0);
        style = match &badge.border {
            Some(b) => style
                .set("strokeColor", b)
                .num("strokeWidth", weight.width()),
            None => style.set("strokeColor", "none"),
        };
        let style = style
            .set("fillColor", &badge.fill)
            .set("fontColor", &badge.text_color)
            .set("fontStyle", 1)
            .num("fontSize", CONNECTOR_BOX_FONT_SIZE)
            .set("resizable", 0)
            .set("connectable", 0);
        cells.push(cell(
            &format!("{}-badge", e.id),
            &escape_xml(&badge.label),
            &style.build(),
            &parent,
            &rect_attributes(Rect::new(
                left + badge.rect.x - e.rect.x,
                top + badge.rect.y - e.rect.y,
                badge.rect.width,
                badge.rect.height,
            )),
        ));
    }
    let mut page = Size::new(
        (max_x - min_x).max(if heading.is_some() {
            HEADING_MIN_WIDTH
        } else {
            0.0
        }),
        bottom + offset_y - ORIGIN,
    );
    // Under the drawing, lined up with its right edge, and as wide as the page at least.
    if let Some(legend) = legend {
        page.width = page.width.max(LEGEND_WIDTH);
        let at = Point::new(
            ORIGIN + page.width - LEGEND_WIDTH,
            ORIGIN + page.height + LEGEND_GAP,
        );
        cells.extend(legend.cells(at, &colors.lines));
        page.height += LEGEND_GAP + legend.height();
    }
    wrap(title, page, &cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::Look;

    #[test]
    fn figures_details_take_their_border_colour() {
        use std::io::Read as _;
        let stencil = physical_figure_stencil(
            PhysicalFigure::LoadBalancer,
            Some("#000000"),
            Some("#9c3530"),
        );
        let packed = base64::engine::general_purpose::STANDARD
            .decode(&stencil)
            .unwrap();
        let mut xml = String::new();
        flate2::read::DeflateDecoder::new(packed.as_slice())
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("%239c3530"));
        assert!(!xml.contains("%23000000"));
        // A figure with no details, or no border, keeps its stencil.
        assert_eq!(
            physical_figure_stencil(PhysicalFigure::User, None, Some("#9c3530")),
            USER_STENCIL
        );
        assert_eq!(
            physical_figure_stencil(PhysicalFigure::Server, Some("#ffffff"), None),
            SERVER_STENCIL_WHITE
        );
    }

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
            None,
            &DiagramColors::default(),
            LineWeight::Thick,
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
    fn turns_on_page_view_with_one_page_fitting_the_drawing_and_its_margin() {
        let a = element("a", None, Shape::Box, Rect::new(100.0, 100.0, 200.0, 100.0));
        let b = element("b", None, Shape::Box, Rect::new(780.0, 400.0, 120.0, 80.0));
        let xml = |elements: &[LookElement], heading: Option<&Heading>| {
            diagram_to_drawio(
                "Page",
                elements,
                &[],
                heading,
                None,
                &DiagramColors::default(),
                LineWeight::Thin,
            )
        };
        // 100..900 by 100..480, with 40px all round.
        assert!(
            xml(&[a.clone(), b], None)
                .contains(r#"page="1" pageScale="1" pageWidth="880" pageHeight="460""#)
        );
        // 200px of drawing, under a heading at least 400px wide: 28 + 20 + 100 high.
        let heading = Heading {
            name: "Page".into(),
            description: None,
        };
        assert!(xml(&[a], Some(&heading)).contains(r#"pageWidth="480" pageHeight="228""#));
    }

    #[test]
    fn pages_link_elements_to_the_pages_of_their_diagrams() {
        let page = |id: &str, title: &str, links: Vec<(ElementId, String)>| DrawioPage {
            id: id.into(),
            file: wrap(
                title,
                Size::default(),
                &[cell(
                    "e1",
                    "Web &amp; API",
                    "rounded=1;",
                    "1",
                    r#"x="0" y="0""#,
                )],
            ),
            links,
        };
        let xml = combine_pages(&[
            page(FIRST_PAGE_ID, "Overview", vec![("e1".into(), "d2".into())]),
            page("d2", "Detail", Vec::new()),
        ]);
        assert_eq!(xml.matches("<mxfile ").count(), 1);
        assert!(xml.contains(r#"<diagram id="archhiero-diagram" name="Overview">"#));
        assert!(xml.contains(r#"<diagram id="d2" name="Detail">"#));
        assert_eq!(xml.matches("<UserObject ").count(), 1);
        assert!(
            xml.contains(r#"<UserObject id="e1" label="Web &amp; API" link="data:page/id,d2">"#)
        );
        assert!(!xml.contains("&lt;a "));
        assert!(xml.contains(r#"<mxCell style="rounded=1;" vertex="1" parent="1">"#));
        assert!(xml.trim_end().ends_with("</mxfile>"));
    }

    #[test]
    fn keeps_the_line_breaks_in_a_description() {
        let mut crm = element("a", None, Shape::Box, Rect::new(40.0, 40.0, 200.0, 100.0));
        crm.description = Some("Leads\nAccounts & more\n".into());
        let xml = diagram_to_drawio(
            "T",
            &[crm],
            &[],
            None,
            None,
            &DiagramColors::default(),
            LineWeight::Thick,
        );
        // Escaped once for the HTML and once more for the XML attribute.
        assert!(
            xml.contains(
                "&lt;span style=&quot;font-size: 13px&quot;&gt;Leads&lt;br&gt;Accounts &amp;amp; more&lt;/span&gt;"
            ),
            "{xml}"
        );
        // A description of only a line break shows nothing.
        let mut blank = element("b", None, Shape::Box, Rect::new(40.0, 40.0, 200.0, 100.0));
        blank.description = Some("\n".into());
        let xml = diagram_to_drawio(
            "T",
            &[blank],
            &[],
            None,
            None,
            &DiagramColors::default(),
            LineWeight::Thick,
        );
        assert!(xml.contains(r#"id="b" value="&lt;b&gt;b &amp;amp; co&lt;/b&gt;""#));
    }

    #[test]
    fn figure_names_grow_down_from_under_the_figure() {
        let elements = vec![
            element("u", None, Shape::Actor, Rect::new(40.0, 40.0, 120.0, 80.0)),
            element(
                "d",
                None,
                Shape::Database,
                Rect::new(200.0, 40.0, 120.0, 80.0),
            ),
        ];
        let xml = diagram_to_drawio(
            "T",
            &elements,
            &[],
            None,
            None,
            &DiagramColors::default(),
            LineWeight::Thick,
        );
        assert!(xml.contains("verticalAlign=top;spacingTop=55;"), "{xml}");
        assert!(xml.contains("verticalAlign=top;spacingTop=50;"), "{xml}");
        assert!(!xml.contains("verticalAlign=bottom"));
    }

    #[test]
    fn styles_keep_the_first_place_of_a_key() {
        let s = Style::new().set("a", 1).set("b", 2).set("a", 3).build();
        assert_eq!(s, "a=3;b=2");
    }
}
