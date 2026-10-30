//! How elements and arrows are drawn, shared by the conceptual and physical diagrams: boxes,
//! clouds, regions, sections, and figures with their names, and arrows with their text and
//! boxes. Builds a [`Scene`]. A port of the prototype's `utils/diagram-svg.ts`.

use serde::{Deserialize, Serialize};

use crate::color::readable_text;
use crate::figures::{
    self, CLOUD_DROP, CLOUD_TEXT_INSET, FIGURE_NAME_GAP, PhysicalFigure, actor, cloud_path,
    component, figure_rect,
};
use crate::geometry::{Point, Rect, fmt_num, round2};
use crate::model::{ColorScheme, ElementId};
use crate::routing::{
    ARROW_TAG_BORDER_WIDTH, Align, CONNECTOR_BOX_FONT_SIZE, CONNECTOR_BOX_HEIGHT,
    connector_text_position, label_placement, label_text_position, return_arrow_placement,
};
use crate::scene::{Anchor, Marker, Node, Paint, Scene};
use crate::text::{MeasureText, TextStyle, Weight, wrap_text};

/// The colours conceptual diagrams are drawn in, also the base the physical diagram's exports
/// draw over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagramColors {
    /// Fill of domains.
    pub domains: String,
    /// Fill of applications and actors that are people.
    pub applications: String,
    /// Fill of components: by default a quarter of the way from `applications` to `processes`.
    pub components: String,
    /// Fill of connector and transformation boxes on arrows, of jobs, and of bots.
    pub connectors: String,
    /// Fill of entities and files, and of databases and storage.
    pub data: String,
    /// Fill of processes.
    pub processes: String,
    /// Borders of elements, figures, and the boxes on arrows.
    pub borders: String,
    /// Arrows: their lines and heads.
    pub lines: String,
    /// Sections' names.
    pub section_names: String,
}

impl Default for DiagramColors {
    fn default() -> Self {
        Self {
            domains: "#f5f5f5".into(),
            applications: "#d0e6fb".into(),
            components: "#a1c6e9".into(),
            connectors: "#cfd4da".into(),
            data: "#ffe599".into(),
            processes: "#1466b3".into(),
            borders: "#1e1e1e".into(),
            lines: "#0066cc".into(),
            section_names: "#0b3d91".into(),
        }
    }
}

impl ColorScheme for DiagramColors {
    const FIELDS: &'static [(&'static str, &'static str)] = &[
        ("domains", "Domains"),
        ("applications", "Applications and Actors"),
        ("components", "Components"),
        ("connectors", "Connectors, Transformations, Jobs, and Bots"),
        ("data", "Entities, Files, Databases, and Storage"),
        ("processes", "Processes"),
        ("borders", "Borders"),
        ("lines", "Lines"),
        ("sectionNames", "Section Names"),
    ];
}

/// The colours of the page behind a drawing, and of text that sets none.
pub const BACKGROUND: &str = "#ffffff";
pub const TEXT: &str = "#1b1f24";
pub const BOX_FILL: &str = "#ffffff";

/// Horizontal padding inside a box.
pub const PADDING_X: f64 = 10.0;

pub const NAME: TextStyle = TextStyle::new(16.0, Weight::Bold, 24.0, 2);
/// The name under a figure sits on a tighter line.
pub const FIGURE_NAME: TextStyle = NAME.with_line_height(20.0);
pub const CONTAINER_NAME: TextStyle = TextStyle::new(20.0, Weight::Bold, 26.0, 1);
/// A name in a container's bottom-left corner, such as a zone's, is smaller.
pub const CORNER_NAME: TextStyle = TextStyle::new(16.0, Weight::Bold, 21.0, 1);
pub const DESCRIPTION: TextStyle = TextStyle::new(13.0, Weight::Regular, 17.0, 3);
pub const SECTION_NAME: TextStyle = CONTAINER_NAME;
/// The gap between a section's name and its line on either side.
pub const SECTION_GAP: f64 = 8.0;
/// The width of a section's lines, as thick as a container's border.
pub const SECTION_LINE_WIDTH: f64 = 3.0;
/// An arrow's text size.
pub const ARROW_LABEL_SIZE: f64 = 13.0;

/// What an element is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Box,
    Cloud,
    /// A filled box with its name as a header.
    Container,
    /// A line across, broken by the name on its middle, with a short line at each end.
    Section,
    /// A person or a bot, with the name underneath.
    Actor,
    /// A cylinder, with the name underneath.
    Database,
    /// A stack of drives, with the name underneath.
    Storage,
    /// A page with a folded corner, with the name underneath.
    File,
    /// One of the physical diagram's figures, with the name underneath.
    Physical(PhysicalFigure),
}

/// Where a container's name sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameAt {
    TopCenter,
    TopLeft,
    BottomLeft,
}

/// How an element looks, apart from its position, size, and text.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub shape: Shape,
    /// Puts a box's or cloud's text at the top, above what it holds.
    pub text_at_top: bool,
    /// Draws an actor as a bot.
    pub agent: bool,
    pub name_at: NameAt,
    /// Square corners on a container.
    pub square: bool,
    /// A container's border width. Unset means 3.
    pub border_width: Option<f64>,
    /// A container's border colour. Unset means the borders colour.
    pub border_color: Option<String>,
    pub dashed: bool,
    /// Draws a container's border again over what it holds.
    pub outline_on_top: bool,
    /// Draws it after everything else, such as a firewall across two networks.
    pub on_top: bool,
    /// Its fill. Unset means white, or the domain colour for a container.
    pub fill: Option<String>,
    /// The colour of its text. Unset means the usual dark text.
    pub text_color: Option<String>,
    /// The colour of details inside a physical figure. Unset means black.
    pub detail_color: Option<String>,
    /// Text on a physical figure, such as a database's "PSQL".
    pub figure_text: Option<String>,
    /// A box on the middle of a physical figure, such as a server's instance count.
    pub figure_badge: Option<ArrowTag>,
}

impl Look {
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            text_at_top: false,
            agent: false,
            name_at: NameAt::TopCenter,
            square: false,
            border_width: None,
            border_color: None,
            dashed: false,
            outline_on_top: false,
            on_top: false,
            fill: None,
            text_color: None,
            detail_color: None,
            figure_text: None,
            figure_badge: None,
        }
    }
}

/// An element to draw, already laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct LookElement {
    pub id: ElementId,
    pub parent_id: Option<ElementId>,
    pub label: String,
    pub description: Option<String>,
    pub look: Look,
    pub rect: Rect,
}

/// A box of text in its own colours, such as an arrow's protocol and port or a figure's
/// instance count.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrowTag {
    pub label: String,
    pub rect: Rect,
    pub fill: String,
    pub text_color: String,
    pub border: Option<String>,
}

/// An arrow to draw, from one point to another.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArrowLook {
    pub from: Point,
    pub to: Point,
    pub dashed: bool,
    pub label: Option<String>,
    /// A small return arrow drawn beside the line, as a conceptual arrow that goes both ways.
    pub bidirectional: bool,
    /// A full head at each end, as two arrows in opposite directions drawn on one line.
    pub two_heads: bool,
    /// A box between the arrow and its target, such as "API". The arrow ends at it.
    pub connector: Option<(String, Rect)>,
    /// A box on the middle of the line, such as "Map fields".
    pub transformation: Option<(String, Rect)>,
    pub tag: Option<ArrowTag>,
}

/// Where the name under a physical figure ends: the figure, the gap, then the name's lines.
pub fn figure_name_bottom(
    area: Rect,
    label: &str,
    figure: PhysicalFigure,
    measure: &dyn MeasureText,
) -> f64 {
    let lines = wrap_text(label, area.width - PADDING_X * 2.0, FIGURE_NAME, measure).len();
    area.y + figure.size().height + FIGURE_NAME_GAP + lines as f64 * FIGURE_NAME.line_height
}

/// How far below the top of its area an actor's figure sits: where it's centred with a name on
/// one line. It stays there when the name takes two, which go below the area.
const ACTOR_FIGURE_TOP: f64 = 5.0;

/// Where a conceptual figure, such as an actor's person, starts. A database, storage, or file
/// sits at the top of its area, so an arrow from above touches it.
fn component_figure_top(y: f64, shape: Shape) -> f64 {
    if shape == Shape::Actor {
        y + ACTOR_FIGURE_TOP
    } else {
        y
    }
}

/// Where the name under a conceptual figure, such as an actor's, ends: the figure, the gap,
/// then the name's lines. A long name ends below the area.
pub fn component_name_bottom(
    area: Rect,
    label: &str,
    shape: Shape,
    measure: &dyn MeasureText,
) -> f64 {
    let lines = wrap_text(label, area.width - PADDING_X * 2.0, FIGURE_NAME, measure).len();
    component_figure_top(area.y, shape)
        + actor::HEIGHT
        + FIGURE_NAME_GAP
        + lines as f64 * FIGURE_NAME.line_height
}

/// Text lines anchored at `x`, the first line's box starting at `top`.
pub fn text_lines(
    lines: &[String],
    x: f64,
    top: f64,
    style: TextStyle,
    anchor: Anchor,
) -> Vec<Node> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let baseline = top
                + i as f64 * style.line_height
                + (style.line_height - style.size) / 2.0
                + style.size * 0.8;
            Node::text(
                Point::new(x, round2(baseline)),
                anchor,
                style.size,
                style.weight,
                line.clone(),
            )
        })
        .collect()
}

/// Nodes in `color`, or in the scene's text colour when unset.
fn colored(nodes: Vec<Node>, color: Option<&String>) -> Vec<Node> {
    match color {
        Some(color) if !nodes.is_empty() => {
            vec![Node::group(None, Paint::fill(color.clone()), nodes)]
        }
        _ => nodes,
    }
}

fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}

/// An actor's head and body in its own coordinates.
fn actor_figure(agent: bool) -> Vec<Node> {
    if agent {
        vec![
            Node::rect(actor::BOT_HEAD, actor::BOT_CORNER, Paint::default()),
            Node::line(
                Point::new(actor::ANTENNA_X, actor::ANTENNA_Y1),
                Point::new(actor::ANTENNA_X, actor::ANTENNA_Y2),
                Paint::default(),
            ),
            Node::Circle {
                center: Point::new(actor::ANTENNA_X, actor::ANTENNA_TIP_Y),
                r: actor::ANTENNA_TIP_RADIUS,
                paint: Paint::default(),
            },
            Node::rect(actor::BOT_BODY, actor::BOT_CORNER, Paint::default()),
        ]
    } else {
        vec![
            Node::Circle {
                center: actor::HEAD_CENTER,
                r: actor::HEAD_RADIUS,
                paint: Paint::default(),
            },
            Node::path(actor::SHOULDERS, Paint::default()),
        ]
    }
}

fn component_figure(shape: Shape, agent: bool) -> Vec<Node> {
    match shape {
        Shape::Database => vec![
            Node::path(component::CYLINDER_SIDE, Paint::default()),
            Node::Ellipse {
                center: component::CYLINDER_TOP_CENTER,
                rx: component::CYLINDER_TOP_RX,
                ry: component::CYLINDER_TOP_RY,
                paint: Paint::default(),
            },
        ],
        Shape::Storage => component::DRIVES
            .iter()
            .map(|d| Node::rect(*d, component::DRIVE_CORNER, Paint::default()))
            .collect(),
        Shape::File => {
            [
                Node::path(component::FILE_PAGE, Paint::default()),
                Node::path(component::FILE_FOLD, Paint::default()),
            ]
            .into_iter()
            .chain(component::FILE_LINES.iter().map(|&(x1, x2, y)| {
                Node::line(Point::new(x1, y), Point::new(x2, y), Paint::default())
            }))
            .collect()
        }
        _ => actor_figure(agent),
    }
}

/// A physical diagram's figure in its own coordinates, with its details in `detail`.
pub fn physical_figure(figure: PhysicalFigure, detail: &str) -> Vec<Node> {
    use figures::{
        container_stack, database, firewall, gateway, load_balancer, server, storage_array, user,
        virtual_machine,
    };
    let line = |s: &figures::Seg| {
        Node::line(
            Point::new(round2(s.from.x), round2(s.from.y)),
            Point::new(round2(s.to.x), round2(s.to.y)),
            Paint::default(),
        )
    };
    let circle = |c: Point, radius: f64| Node::Circle {
        center: c,
        r: radius,
        paint: Paint::default(),
    };
    match figure {
        PhysicalFigure::User => vec![
            circle(user::HEAD_CENTER, user::HEAD_RADIUS),
            Node::path(user::SHOULDERS, Paint::default()),
        ],
        PhysicalFigure::Server => vec![
            Node::rect(server::BODY, server::CORNER, Paint::default()),
            Node::group(
                None,
                Paint {
                    stroke: Some(detail.into()),
                    ..Paint::default()
                },
                server::LINES.iter().map(line).collect(),
            ),
        ],
        PhysicalFigure::Firewall => vec![
            Node::rect(firewall::BODY, 0.0, Paint::default()),
            Node::group(
                None,
                Paint::stroke(detail, firewall::MORTAR_WIDTH),
                firewall::mortar().iter().map(line).collect(),
            ),
        ],
        PhysicalFigure::LoadBalancer => {
            let mut dots = vec![circle(load_balancer::INPUT, load_balancer::DOT_RADIUS)];
            dots.extend(
                load_balancer::OUTPUTS
                    .iter()
                    .map(|o| circle(*o, load_balancer::DOT_RADIUS)),
            );
            let mut links: Vec<Node> = load_balancer::LINKS.iter().map(line).collect();
            links.push(Node::group(None, Paint::fill(detail), dots));
            vec![
                circle(load_balancer::BODY_CENTER, load_balancer::BODY_RADIUS),
                Node::group(
                    None,
                    Paint::stroke(detail, load_balancer::DETAIL_WIDTH),
                    links,
                ),
            ]
        }
        PhysicalFigure::Gateway => vec![
            Node::rect(gateway::BODY, gateway::CORNER, Paint::default()),
            Node::group(
                None,
                Paint::fill_stroke(detail, detail, gateway::DETAIL_WIDTH),
                gateway::ports()
                    .into_iter()
                    .map(|d| Node::path(d, Paint::default()))
                    .collect(),
            ),
        ],
        PhysicalFigure::DatabaseCylinder => vec![
            Node::path(database::CYLINDER_SIDE, Paint::default()),
            Node::Ellipse {
                center: database::CYLINDER_TOP_CENTER,
                rx: database::CYLINDER_TOP_RX,
                ry: database::CYLINDER_TOP_RY,
                paint: Paint::default(),
            },
        ],
        PhysicalFigure::StorageArray => {
            let mut nodes: Vec<Node> = storage_array::DISKS
                .iter()
                .map(|d| Node::rect(*d, storage_array::CORNER, Paint::default()))
                .collect();
            nodes.extend(storage_array::CONNECTION.iter().map(line));
            nodes
        }
        PhysicalFigure::VirtualMachine => virtual_machine::FACES
            .iter()
            .map(|d| Node::path(*d, Paint::default()))
            .collect(),
        PhysicalFigure::ContainerStack => {
            let mut nodes = vec![Node::path(container_stack::HULL, Paint::default())];
            nodes.extend(
                container_stack::SQUARES
                    .iter()
                    .map(|s| Node::rect(*s, 0.0, Paint::default())),
            );
            nodes
        }
    }
}

/// Text on a physical figure, such as a database's abbreviation on its cylinder.
fn figure_text(figure: PhysicalFigure, text: Option<&String>, color: &str) -> Vec<Node> {
    let Some(text) = text.filter(|t| !t.is_empty()) else {
        return Vec::new();
    };
    if figure != PhysicalFigure::DatabaseCylinder {
        return Vec::new();
    }
    vec![Node::Text {
        at: figures::database::TEXT_AT,
        anchor: Anchor::Middle,
        size: figures::database::TEXT_SIZE,
        weight: Weight::Bold,
        text: text.clone(),
        fill: Some(color.into()),
        halo: None,
    }]
}

/// A figure's badge: the room left and right of its text, its narrowest, and the gap between
/// the figure's right edge and it. Its ends are round, so one digit makes a circle.
pub const FIGURE_BADGE_PADDING_X: f64 = 6.0;
pub const FIGURE_BADGE_MIN_WIDTH: f64 = 20.0;
pub const FIGURE_BADGE_GAP: f64 = 4.0;

/// Where a box of text `text_width` wide goes on a physical figure in `area`: just right of its
/// bottom-right corner, level with its bottom edge so it stays clear of the name, as tall as an
/// arrow's tag.
pub fn figure_badge_rect(area: Rect, figure: PhysicalFigure, text_width: f64) -> Rect {
    let width = FIGURE_BADGE_MIN_WIDTH.max(text_width.ceil() + FIGURE_BADGE_PADDING_X * 2.0);
    let figure = figure_rect(area, figure);
    Rect::new(
        round2(figure.right() + FIGURE_BADGE_GAP),
        round2(figure.bottom() - CONNECTOR_BOX_HEIGHT),
        width,
        CONNECTOR_BOX_HEIGHT,
    )
}

/// A box of text with its label centred, as an arrow's tag, with corners of `radius`.
fn tag_nodes(tag: &ArrowTag, radius: f64) -> [Node; 2] {
    let paint = match &tag.border {
        Some(border) => {
            Paint::fill_stroke(tag.fill.clone(), border.clone(), ARROW_TAG_BORDER_WIDTH)
        }
        None => Paint::fill(tag.fill.clone()),
    };
    [
        Node::rect(tag.rect, radius, paint),
        Node::Text {
            at: connector_text_position(&tag.rect),
            anchor: Anchor::Middle,
            size: CONNECTOR_BOX_FONT_SIZE,
            weight: Weight::SemiBold,
            text: tag.label.clone(),
            fill: Some(tag.text_color.clone()),
            halo: None,
        },
    ]
}

/// The nodes that draw one element.
pub fn draw_element(
    element: &LookElement,
    measure: &dyn MeasureText,
    colors: &DiagramColors,
) -> Vec<Node> {
    let Rect {
        x,
        y,
        width,
        height,
    } = element.rect;
    let look = &element.look;
    let center_x = x + width / 2.0;
    let text_width = width - PADDING_X * 2.0;

    match look.shape {
        Shape::Container => {
            let style = if look.name_at == NameAt::BottomLeft {
                CORNER_NAME
            } else {
                CONTAINER_NAME
            };
            let name = wrap_text(&element.label, text_width, style, measure);
            let text = match look.name_at {
                NameAt::TopCenter => text_lines(&name, center_x, y + 8.0, style, Anchor::Middle),
                NameAt::TopLeft => text_lines(&name, x + PADDING_X, y + 4.0, style, Anchor::Start),
                NameAt::BottomLeft => text_lines(
                    &name,
                    x + PADDING_X,
                    y + height - 8.0 - name.len() as f64 * style.line_height,
                    style,
                    Anchor::Start,
                ),
            };
            let mut nodes = vec![Node::rect(
                element.rect,
                if look.square { 0.0 } else { 6.0 },
                Paint::fill_stroke(
                    look.fill.clone().unwrap_or_else(|| colors.domains.clone()),
                    look.border_color
                        .clone()
                        .unwrap_or_else(|| colors.borders.clone()),
                    look.border_width.unwrap_or(3.0),
                )
                .dashed(look.dashed.then_some((8.0, 4.0))),
            )];
            nodes.extend(colored(text, look.text_color.as_ref()));
            nodes
        }
        Shape::Section => draw_section(element, measure, colors),
        Shape::Physical(figure) => {
            let size = figure.size();
            let name = wrap_text(&element.label, text_width, FIGURE_NAME, measure);
            let name_top = y + size.height + FIGURE_NAME_GAP;
            let detail = look
                .detail_color
                .clone()
                .unwrap_or_else(|| "#000000".into());
            let mut children = physical_figure(figure, &detail);
            children.extend(figure_text(figure, look.figure_text.as_ref(), &detail));
            let mut nodes = vec![Node::group(
                Some(Point::new(round2(center_x - size.width / 2.0), round2(y))),
                Paint::fill_stroke(
                    look.fill.clone().unwrap_or_else(|| BOX_FILL.into()),
                    colors.borders.clone(),
                    2.0,
                ),
                children,
            )];
            // No background, so whatever is behind the name, such as its zone, shows through.
            nodes.extend(colored(
                text_lines(&name, center_x, name_top, FIGURE_NAME, Anchor::Middle),
                look.text_color.as_ref(),
            ));
            nodes
        }
        Shape::Actor | Shape::Database | Shape::Storage | Shape::File => {
            // The figure stays put and the name grows down, below the area when it takes two lines.
            let name = wrap_text(&element.label, text_width, FIGURE_NAME, measure);
            let top = component_figure_top(y, look.shape);
            let mut nodes = vec![Node::group(
                Some(Point::new(
                    round2(center_x - actor::WIDTH / 2.0),
                    round2(top),
                )),
                Paint::fill_stroke(
                    look.fill.clone().unwrap_or_else(|| BOX_FILL.into()),
                    colors.borders.clone(),
                    2.0,
                ),
                component_figure(look.shape, look.agent),
            )];
            nodes.extend(text_lines(
                &name,
                center_x,
                top + actor::HEIGHT + FIGURE_NAME_GAP,
                FIGURE_NAME,
                Anchor::Middle,
            ));
            nodes
        }
        Shape::Box | Shape::Cloud => {
            let cloud = look.shape == Shape::Cloud;
            let inner = if cloud {
                width * (1.0 - CLOUD_TEXT_INSET * 2.0)
            } else {
                text_width
            };
            let name = wrap_text(&element.label, inner, NAME, measure);
            let description = match element.description.as_deref() {
                Some(d) if !d.is_empty() => wrap_text(
                    d,
                    inner,
                    if look.text_at_top {
                        DESCRIPTION.with_max_lines(1)
                    } else {
                        DESCRIPTION
                    },
                    measure,
                ),
                _ => Vec::new(),
            };
            let gap = if description.is_empty() { 0.0 } else { 2.0 };
            let content_height = name.len() as f64 * NAME.line_height
                + gap
                + description.len() as f64 * DESCRIPTION.line_height;
            let top = if look.text_at_top {
                y + if cloud {
                    12.0 + height * CLOUD_DROP * 2.0
                } else {
                    6.0
                }
            } else {
                y + (height - content_height) / 2.0 + if cloud { height * CLOUD_DROP } else { 0.0 }
            };
            let paint = Paint::fill_stroke(
                look.fill.clone().unwrap_or_else(|| BOX_FILL.into()),
                colors.borders.clone(),
                2.0,
            );
            let outline = if cloud {
                Node::path(
                    cloud_path(r(x + 1.0, y + 1.0, width - 2.0, height - 2.0)),
                    paint,
                )
            } else {
                Node::rect(element.rect, 6.0, paint)
            };
            let mut text = text_lines(&name, center_x, top, NAME, Anchor::Middle);
            text.extend(text_lines(
                &description,
                center_x,
                top + name.len() as f64 * NAME.line_height + gap,
                DESCRIPTION,
                Anchor::Middle,
            ));
            let mut nodes = vec![outline];
            nodes.extend(colored(text, look.text_color.as_ref()));
            nodes
        }
    }
}

fn draw_section(
    element: &LookElement,
    measure: &dyn MeasureText,
    colors: &DiagramColors,
) -> Vec<Node> {
    let Rect {
        x,
        y,
        width,
        height,
    } = element.rect;
    let center_x = x + width / 2.0;
    let middle = y + height / 2.0;
    let left = x + SECTION_LINE_WIDTH / 2.0;
    let right = x + width - SECTION_LINE_WIDTH / 2.0;
    let name = wrap_text(
        &element.label,
        (width - SECTION_GAP * 4.0).max(0.0),
        SECTION_NAME,
        measure,
    )
    .into_iter()
    .next()
    .unwrap_or_default();
    let half = if name.is_empty() {
        0.0
    } else {
        measure.width(&name, SECTION_NAME.size, SECTION_NAME.weight) / 2.0 + SECTION_GAP
    };
    let f = |v: f64| fmt_num(round2(v));
    let d = format!(
        "M{} {}V{}M{} {}V{}M{} {}H{}M{} {}H{}",
        f(left),
        f(y + height / 4.0),
        f(y + height * 3.0 / 4.0),
        f(right),
        f(y + height / 4.0),
        f(y + height * 3.0 / 4.0),
        f(left),
        f(middle),
        f(center_x - half),
        f(center_x + half),
        f(middle),
        f(right)
    );
    let mut nodes = vec![Node::path(
        d,
        Paint {
            fill: Some("none".into()),
            stroke: Some(colors.borders.clone()),
            stroke_width: Some(SECTION_LINE_WIDTH),
            dash: None,
        },
    )];
    let lines: Vec<String> = if name.is_empty() {
        Vec::new()
    } else {
        vec![name]
    };
    nodes.extend(colored(
        text_lines(
            &lines,
            center_x,
            middle - SECTION_NAME.line_height / 2.0,
            SECTION_NAME,
            Anchor::Middle,
        ),
        element.look.text_color.as_ref(),
    ));
    nodes
}

/// The area elements and arrows cover.
pub fn bounds_of(elements: &[LookElement], arrows: &[ArrowLook]) -> Rect {
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut add = |rect: &Rect| {
        xs.extend([rect.x, rect.right()]);
        ys.extend([rect.y, rect.bottom()]);
    };
    for e in elements {
        add(&e.rect);
    }
    for a in arrows {
        for rect in [
            a.connector.as_ref().map(|c| c.1),
            a.transformation.as_ref().map(|t| t.1),
            a.tag.as_ref().map(|t| t.rect),
        ]
        .into_iter()
        .flatten()
        {
            add(&rect);
        }
    }
    for a in arrows {
        xs.extend([a.from.x, a.to.x]);
        ys.extend([a.from.y, a.to.y]);
    }
    if xs.is_empty() {
        return Rect::default();
    }
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Draws a diagram: elements in the order given (containers before their children), then
/// arrows with their text and boxes on top.
pub fn draw_diagram(
    elements: &[LookElement],
    arrows: &[ArrowLook],
    measure: &dyn MeasureText,
    colors: &DiagramColors,
) -> Scene {
    let mut nodes: Vec<Node> = Vec::new();
    for e in elements.iter().filter(|e| !e.look.on_top) {
        nodes.extend(draw_element(e, measure, colors));
    }
    for e in elements.iter().filter(|e| e.look.outline_on_top) {
        nodes.push(Node::rect(
            e.rect,
            if e.look.square { 0.0 } else { 6.0 },
            Paint {
                fill: Some("none".into()),
                stroke: Some(colors.borders.clone()),
                stroke_width: Some(e.look.border_width.unwrap_or(3.0)),
                dash: None,
            },
        ));
    }
    for e in elements.iter().filter(|e| e.look.on_top) {
        nodes.extend(draw_element(e, measure, colors));
    }
    nodes.extend(draw_arrows(elements, arrows, colors));
    Scene {
        nodes,
        bounds: bounds_of(elements, arrows),
        text_color: TEXT.into(),
        marker_color: colors.lines.clone(),
        background: BACKGROUND.into(),
    }
}

/// The arrows' lines, return arrows, figure badges, boxes, tags, and text.
pub fn draw_arrows(
    elements: &[LookElement],
    arrows: &[ArrowLook],
    colors: &DiagramColors,
) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    for a in arrows {
        let paint = Paint::stroke(colors.lines.clone(), 2.0).dashed(a.dashed.then_some((8.0, 4.0)));
        let line = |from: Point, to: Point| Node::Line {
            from: Point::new(round2(from.x), round2(from.y)),
            to: Point::new(round2(to.x), round2(to.y)),
            paint: paint.clone(),
            marker: Some(Marker::Arrowhead),
        };
        if a.two_heads {
            // Two halves from the middle, each with its head.
            let middle = Point::new(
                f64::midpoint(a.from.x, a.to.x),
                f64::midpoint(a.from.y, a.to.y),
            );
            nodes.push(line(middle, a.to));
            nodes.push(line(middle, a.from));
        } else {
            nodes.push(line(a.from, a.to));
        }
    }
    for a in arrows.iter().filter(|a| a.bidirectional) {
        if let Some((from, to)) = return_arrow_placement(a.from, a.to) {
            nodes.push(Node::Line {
                from,
                to,
                paint: Paint::stroke(colors.lines.clone(), 1.5),
                marker: Some(Marker::ReturnArrowhead),
            });
        }
    }
    // Over the arrows, so they don't move to make room.
    for e in elements {
        if let (Some(badge), Shape::Physical(_)) = (&e.look.figure_badge, e.look.shape) {
            nodes.extend(tag_nodes(badge, badge.rect.height / 2.0));
        }
    }
    let connector_text = readable_text(&colors.connectors);
    for (label, rect) in arrows
        .iter()
        .flat_map(|a| [a.connector.as_ref(), a.transformation.as_ref()])
        .flatten()
    {
        let text = connector_text_position(rect);
        nodes.push(Node::rect(
            *rect,
            3.0,
            Paint::fill_stroke(colors.connectors.clone(), colors.borders.clone(), 1.5),
        ));
        nodes.push(Node::Text {
            at: text,
            anchor: Anchor::Middle,
            size: CONNECTOR_BOX_FONT_SIZE,
            weight: Weight::SemiBold,
            text: label.clone(),
            fill: Some(connector_text.into()),
            halo: None,
        });
    }
    for tag in arrows.iter().filter_map(|a| a.tag.as_ref()) {
        nodes.extend(tag_nodes(tag, 3.0));
    }
    for a in arrows {
        let Some(label) = a.label.as_ref().filter(|l| !l.is_empty()) else {
            continue;
        };
        let (at, align) = label_text_position(label_placement(a.from, a.to), ARROW_LABEL_SIZE);
        nodes.push(Node::Text {
            at: Point::new(round2(at.x), round2(at.y)),
            anchor: match align {
                Align::Start => Anchor::Start,
                Align::End => Anchor::End,
            },
            size: ARROW_LABEL_SIZE,
            weight: Weight::Regular,
            text: label.clone(),
            fill: None,
            halo: Some((BACKGROUND.into(), 3.0)),
        });
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::svg::scene_to_svg;
    use crate::text::EstimateText;

    fn element(shape: Shape, rect: Rect) -> LookElement {
        LookElement {
            id: "e".into(),
            parent_id: None,
            label: "Customer Portal".into(),
            description: Some("Self-service website".into()),
            look: Look::new(shape),
            rect,
        }
    }

    #[test]
    fn boxes_centre_their_text() {
        let nodes = draw_element(
            &element(Shape::Box, Rect::new(40.0, 40.0, 200.0, 100.0)),
            &EstimateText,
            &DiagramColors::default(),
        );
        let svg_scene = Scene {
            nodes,
            bounds: Rect::new(40.0, 40.0, 200.0, 100.0),
            text_color: TEXT.into(),
            marker_color: "#0066cc".into(),
            background: BACKGROUND.into(),
        };
        let svg = scene_to_svg("t", &svg_scene);
        assert!(svg.contains(r##"<rect x="40" y="40" width="200" height="100" rx="6" fill="#ffffff" stroke="#1e1e1e" stroke-width="2"/>"##));
        // Name on one line (15 chars × 8.8 = 132 ≤ 180), description under it.
        assert!(svg.contains(r#"<text x="140" y="85.3" text-anchor="middle" font-size="16" font-weight="700">Customer Portal</text>"#));
        assert!(svg.contains(r#"font-size="13" font-weight="400">Self-service website</text>"#));
    }

    #[test]
    fn descriptions_keep_their_line_breaks() {
        let mut e = element(Shape::Box, Rect::new(40.0, 40.0, 200.0, 140.0));
        e.description = Some("Leads\nAccounts".into());
        let scene = draw_diagram(&[e], &[], &EstimateText, &DiagramColors::default());
        let svg = scene_to_svg("t", &scene);
        // Each line of the description on a line of its own, though both would fit on one.
        assert!(svg.contains(r#"font-size="13" font-weight="400">Leads</text>"#));
        assert!(svg.contains(r#"font-size="13" font-weight="400">Accounts</text>"#));
    }

    #[test]
    fn arrows_get_heads_labels_and_boxes() {
        let arrows = [ArrowLook {
            from: Point::new(100.0, 50.0),
            to: Point::new(300.0, 50.0),
            label: Some("Orders".into()),
            bidirectional: true,
            transformation: Some(("Map".into(), Rect::new(184.0, 40.0, 32.0, 20.0))),
            ..ArrowLook::default()
        }];
        let scene = draw_diagram(&[], &arrows, &EstimateText, &DiagramColors::default());
        let svg = scene_to_svg("t", &scene);
        assert!(svg.contains(r##"<line x1="100" y1="50" x2="300" y2="50" stroke="#0066cc" stroke-width="2" marker-end="url(#arrowhead)"/>"##));
        assert!(svg.contains(r#"marker-end="url(#return-arrowhead)""#));
        assert!(svg.contains(r##"stroke="#ffffff" stroke-width="3" stroke-linejoin="round" paint-order="stroke">Orders</text>"##));
        assert!(svg.contains(">Map</text>"));
    }

    #[test]
    fn two_headed_arrows_have_a_full_head_at_each_end() {
        let arrows = [ArrowLook {
            from: Point::new(100.0, 50.0),
            to: Point::new(300.0, 50.0),
            two_heads: true,
            ..ArrowLook::default()
        }];
        let scene = draw_diagram(&[], &arrows, &EstimateText, &DiagramColors::default());
        let svg = scene_to_svg("t", &scene);
        assert!(svg.contains(r#"<line x1="200" y1="50" x2="300" y2="50""#));
        assert!(svg.contains(r#"<line x1="200" y1="50" x2="100" y2="50""#));
        assert_eq!(svg.matches(r#"marker-end="url(#arrowhead)""#).count(), 2);
        assert!(!svg.contains("url(#return-arrowhead)"));
    }

    #[test]
    fn actors_stay_put_when_their_name_takes_two_lines() {
        let area = Rect::new(40.0, 40.0, 120.0, 80.0);
        let svg_of = |label: &str| {
            let mut e = element(Shape::Actor, area);
            e.label = label.into();
            let scene = draw_diagram(&[e], &[], &EstimateText, &DiagramColors::default());
            scene_to_svg("t", &scene)
        };
        // The figure starts 5px down the area, whatever the name's length.
        for label in ["Rep", "Customer Service Representative"] {
            assert!(
                svg_of(label).contains(r#"<g transform="translate(76 45)""#),
                "{label}"
            );
        }
        // A name on one line ends inside the area, one on two lines below it.
        let bottom = |label| component_name_bottom(area, label, Shape::Actor, &EstimateText);
        assert_eq!(bottom("Rep"), 115.0);
        assert_eq!(bottom("Customer Service Representative"), 135.0);
        // A database sits at the very top.
        assert_eq!(
            component_name_bottom(area, "Orders", Shape::Database, &EstimateText),
            110.0
        );
    }

    #[test]
    fn physical_figures_have_names_with_no_background() {
        let mut e = element(
            Shape::Physical(PhysicalFigure::DatabaseCylinder),
            Rect::new(40.0, 40.0, 120.0, 100.0),
        );
        e.label = "Orders".into();
        e.look.figure_text = Some("PSQL".into());
        let scene = draw_diagram(
            std::slice::from_ref(&e),
            &[],
            &EstimateText,
            &DiagramColors::default(),
        );
        let svg = scene_to_svg("t", &scene);
        assert!(svg.contains(r#"<g transform="translate(76 40)""#));
        assert!(svg.contains(">PSQL</text>"));
        // The figure is drawn in its group; there's no box behind the name.
        assert!(!scene.nodes.iter().any(|n| matches!(n, Node::Rect { .. })));
    }
}
