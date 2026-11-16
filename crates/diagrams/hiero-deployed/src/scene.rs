//! How a deployed component diagram is drawn: layers with their characteristics bars, and
//! components with their protocol bars, bulleted descriptions, logos, and instance counts. A
//! port of the prototype's `utils/deployed-component-svg.ts`.

use hiero_core::engine::{Engine, Laid};
use hiero_core::geometry::{Point, Rect, round2};
use hiero_core::model::LineWeight;
use hiero_core::scene::{Anchor, Node, Paint, Scene};
use hiero_core::text::{MeasureText, TextStyle, Weight, wrap_text};

use std::collections::HashSet;

use hiero_core::export::drawio::{Legend, LegendBeside, LegendIcon, Stamp, Swatch};
use hiero_core::palette::{
    CORE_COMPONENT, ComponentColors, EXTERNAL_DEPENDENCY, INTERNAL_DEPENDENCY,
};

use crate::layout::{
    BADGE_FONT_SIZE, BULLET_LINE, LAYER_BAR_HEIGHT, LAYER_BAR_LIFT, LAYER_GAP, LAYER_NAME_HEIGHT,
    LOGO, LOGO_BOX, PROTOCOL_BAR_LIFT, SEPARATOR_LEAN, characteristics, component_parts, is_layer,
    layer_name_top, protocol_segments,
};
use crate::model::{DeployedComponent, LAYER_PADDING, colors, component_colors};
use crate::tech_stack::{self, TechItem};

const BAR_TEXT: (f64, Weight) = (11.0, Weight::Bold);
const LAYER_NAME: (f64, Weight) = (16.0, Weight::Bold);
const COMPONENT_NAME: (f64, Weight) = (16.0, Weight::Bold);
const BULLET_TEXT: (f64, Weight) = (13.0, Weight::Regular);
/// How far a bullet's text sits from its column's left edge, after the dot.
const BULLET_INDENT: f64 = 16.0;
const BULLET_DOT_RADIUS: f64 = 2.5;
const BULLET_DOT_X: f64 = 5.0;
/// The rounding of layers' and components' corners.
pub const CORNER_RADIUS: f64 = 6.0;

/// `text` on one line no wider than `width`, ending with "…" when it's cut.
fn fit(text: &str, width: f64, style: (f64, Weight), measure: &dyn MeasureText) -> String {
    wrap_text(
        text,
        width,
        TextStyle::new(style.0, style.1, style.0, 1),
        measure,
    )
    .into_iter()
    .next()
    .unwrap_or_default()
}

/// Text centred from top to bottom in a band `height` high starting at `top`.
fn text(
    value: String,
    x: f64,
    top: f64,
    height: f64,
    style: (f64, Weight),
    anchor: Anchor,
) -> Node {
    let baseline = top + (height - style.0) / 2.0 + style.0 * 0.8;
    Node::text(
        Point::new(round2(x), round2(baseline)),
        anchor,
        style.0,
        style.1,
        value,
    )
}

/// A bar across the top of a layer or component: a pill with `labels` side by side, with a
/// line leaning over between each and the next. Its lines are `width` wide.
fn bar(
    rect: Rect,
    labels: &[String],
    (border, width): (&str, f64),
    measure: &dyn MeasureText,
) -> Vec<Node> {
    let (segments, separators) = protocol_segments(rect, labels);
    let mut nodes = vec![Node::rect(
        rect,
        rect.height / 2.0,
        Paint {
            fill: Some(colors::BAR_FILL.into()),
            stroke: Some(border.into()),
            stroke_width: Some(width),
            ..Paint::default()
        },
    )];
    for (from, to) in separators {
        nodes.push(Node::line(
            Point::new(round2(from.x), from.y),
            Point::new(round2(to.x), to.y),
            Paint {
                stroke: Some(border.into()),
                stroke_width: Some(width),
                ..Paint::default()
            },
        ));
    }
    for s in segments {
        nodes.push(text(
            fit(&s.label, s.width - SEPARATOR_LEAN * 4.0, BAR_TEXT, measure),
            s.x + s.width / 2.0,
            rect.y,
            rect.height,
            BAR_TEXT,
            Anchor::Middle,
        ));
    }
    nodes
}

fn draw_layer(
    layer: &Laid<DeployedComponent>,
    measure: &dyn MeasureText,
    weight: LineWeight,
) -> Vec<Node> {
    let r = layer.rect;
    let chars = characteristics(&layer.el);
    let mut nodes = vec![Node::rect(
        r,
        CORNER_RADIUS,
        Paint::fill_stroke(colors::LAYER_FILL, colors::LAYER_BORDER, weight.width()),
    )];
    if !chars.is_empty() {
        nodes.extend(bar(
            Rect::new(r.x, r.y - LAYER_BAR_LIFT, r.width, LAYER_BAR_HEIGHT),
            chars,
            (colors::LAYER_BORDER, weight.width()),
            measure,
        ));
    }
    nodes.push(text(
        fit(
            &layer.el.label,
            r.width - LAYER_PADDING * 2.0,
            LAYER_NAME,
            measure,
        ),
        r.x + LAYER_PADDING,
        r.y + layer_name_top(chars),
        LAYER_NAME_HEIGHT,
        LAYER_NAME,
        Anchor::Start,
    ));
    nodes
}

fn draw_component(
    component: &Laid<DeployedComponent>,
    measure: &dyn MeasureText,
    width: f64,
) -> Vec<Node> {
    let r = component.rect;
    let el = &component.el;
    let color = component_colors(el.props.dependency);
    let tech = tech_stack::items(el.props.tech_stack.as_deref().unwrap_or_default());
    let parts = component_parts(
        r,
        el.description.as_deref(),
        tech.len(),
        el.props.instance_count,
    );
    let divider = |y: f64| {
        Node::line(
            Point::new(r.x, y),
            Point::new(r.right(), y),
            Paint::stroke(color.border, width),
        )
    };
    let mut nodes = vec![
        Node::rect(
            r,
            CORNER_RADIUS,
            Paint::fill_stroke(color.fill, color.border, width),
        ),
        divider(parts.name_divider_y),
        divider(parts.footer_divider_y),
    ];
    if let Some(protocols) = el.props.protocols.as_ref().filter(|p| !p.is_empty()) {
        nodes.extend(bar(
            parts.protocol_bar,
            protocols,
            (color.border, width),
            measure,
        ));
    }
    nodes.push(text(
        fit(&el.label, parts.name.width, COMPONENT_NAME, measure),
        parts.name.x,
        parts.name.y,
        parts.name.height,
        COMPONENT_NAME,
        Anchor::Start,
    ));
    for column in &parts.columns {
        for (i, row) in column.rows.iter().enumerate() {
            let top = column.y + i as f64 * BULLET_LINE;
            nodes.push(Node::Circle {
                center: Point::new(round2(column.x + BULLET_DOT_X), top + BULLET_LINE / 2.0),
                r: BULLET_DOT_RADIUS,
                paint: Paint::default(),
            });
            nodes.push(text(
                fit(
                    row,
                    column.width - BULLET_INDENT - 4.0,
                    BULLET_TEXT,
                    measure,
                ),
                column.x + BULLET_INDENT,
                top,
                BULLET_LINE,
                BULLET_TEXT,
                Anchor::Start,
            ));
        }
    }
    let inset = (LOGO_BOX - LOGO) / 2.0;
    for (rect, item) in parts.logos.iter().zip(&tech) {
        nodes.push(Node::rect(
            *rect,
            4.0,
            Paint {
                fill: Some(colors::LOGO_FILL.into()),
                stroke: Some(color.border.into()),
                stroke_width: Some(width),
                ..Paint::default()
            },
        ));
        nodes.push(Node::Image {
            rect: Rect::new(rect.x + inset, rect.y + inset, LOGO, LOGO),
            png: item.png,
        });
    }
    if let Some((label, badge)) = &parts.instances {
        nodes.push(Node::rect(
            Rect {
                x: round2(badge.x),
                ..*badge
            },
            badge.height / 2.0,
            Paint {
                fill: Some(colors::BAR_FILL.into()),
                stroke: Some(color.border.into()),
                stroke_width: Some(width),
                ..Paint::default()
            },
        ));
        nodes.push(text(
            label.clone(),
            badge.x + badge.width / 2.0,
            badge.y,
            badge.height,
            (BADGE_FONT_SIZE, Weight::Bold),
            Anchor::Middle,
        ));
    }
    nodes
}

/// The diagram drawn from `layout`: its layers, then the components over them.
pub fn scene_of(eng: &Engine<DeployedComponent>, layout: &[Laid<DeployedComponent>]) -> Scene {
    let measure = eng.measure();
    let (layers, components): (Vec<_>, Vec<_>) = layout.iter().partition(|e| is_layer(&e.el));
    let weight = eng.line_weight();
    let mut nodes = Vec::new();
    for layer in &layers {
        nodes.extend(draw_layer(layer, measure, weight));
    }
    for component in &components {
        nodes.extend(draw_component(component, measure, weight.width()));
    }
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for e in layout {
        xs.extend([e.rect.x, e.rect.right()]);
        ys.extend([e.rect.y, e.rect.bottom()]);
    }
    ys.extend(components.iter().map(|c| c.rect.y - PROTOCOL_BAR_LIFT));
    ys.extend(
        layers
            .iter()
            .filter(|l| !characteristics(&l.el).is_empty())
            .map(|l| l.rect.y - LAYER_BAR_LIFT),
    );
    let bounds = if xs.is_empty() {
        Rect::default()
    } else {
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Rect::new(min(&xs), min(&ys), max(&xs) - min(&xs), max(&ys) - min(&ys))
    };
    Scene {
        nodes,
        bounds,
        text_color: colors::TEXT.into(),
        marker_color: colors::TEXT.into(),
        background: "#ffffff".into(),
    }
}

/// The tech stack logos the diagram shows, each once, in the order they're offered: those that
/// fit in their components' bottom bars, beside any instance count.
pub fn logos_shown(layout: &[Laid<DeployedComponent>]) -> Vec<&'static TechItem> {
    let shown: HashSet<&str> = layout
        .iter()
        .filter(|e| !is_layer(&e.el))
        .flat_map(|c| {
            let tech = tech_stack::items(c.el.props.tech_stack.as_deref().unwrap_or_default());
            let fits = component_parts(
                c.rect,
                c.el.description.as_deref(),
                tech.len(),
                c.el.props.instance_count,
            )
            .logos
            .len();
            tech.into_iter().take(fits).map(|item| item.id)
        })
        .collect();
    tech_stack::TECH_STACK
        .iter()
        .filter(|item| shown.contains(item.id))
        .collect()
}

/// The legend beside a deployed component diagram: the colours of its core components and its
/// internal and external dependencies, one under another, each with its own border, then,
/// under a Technology Stack heading, each of the tech stack `logos` the diagram shows, two to a
/// row. It's wider than a conceptual diagram's, for their longer names, such as "Microsoft SQL
/// Server".
pub fn legend(logos: &[&'static TechItem], stamp: Stamp) -> Legend {
    let swatch = |c: ComponentColors| Swatch {
        label: c.label.into(),
        fill: c.fill.into(),
        border: Some(c.border.into()),
    };
    Legend {
        lines: Vec::new(),
        swatches: vec![
            swatch(CORE_COMPONENT),
            swatch(INTERNAL_DEPENDENCY),
            swatch(EXTERNAL_DEPENDENCY),
        ],
        swatch_columns: 1,
        icons: logos
            .iter()
            .map(|item| LegendIcon {
                label: item.label.into(),
                png: item.png,
            })
            .collect(),
        icons_title: Some("Technology Stack".into()),
        width: LEGEND_WIDTH,
        stamp,
    }
}

/// The legend's width: wider than the usual, for tech stack names.
const LEGEND_WIDTH: f64 = 320.0;

/// The diagram's legend, stamped with `stamp`, and how it goes beside the drawing: as far from
/// the layers as they are from each other, with its border as wide as their lines.
pub fn legend_beside(eng: &Engine<DeployedComponent>, stamp: Stamp) -> LegendBeside {
    LegendBeside {
        legend: legend(&logos_shown(eng.layout()), stamp),
        gap: LAYER_GAP,
        line_color: colors::LAYER_BORDER.into(),
        border_width: eng.line_weight().width(),
    }
}

pub fn scene(eng: &Engine<DeployedComponent>) -> Scene {
    scene_of(eng, eng.layout())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DeployedKind;
    use hiero_core::count::InstanceCount;
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    #[test]
    fn the_legend_names_the_colours_and_each_logo_shown_once() {
        let mut e = Engine::<DeployedComponent>::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE);
        let tech = |e: &mut Engine<DeployedComponent>, ids: &[&str]| {
            e.add_at_free_position(DeployedKind::Component);
            let id = e.selected_id().unwrap().clone();
            let ids: Vec<String> = ids.iter().map(|&i| i.into()).collect();
            e.replace(&id, |mut c| {
                c.props.tech_stack = Some(ids);
                c
            });
        };
        tech(&mut e, &["postgresql", "aws"]);
        tech(&mut e, &["aws", "linux"]);
        // Each once, in the order the catalogue offers them.
        let labels: Vec<&str> = logos_shown(e.layout()).iter().map(|i| i.label).collect();
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0], "AWS");
        let stamp = Stamp {
            generated: "4 Nov 2026".into(),
            version: "Version 3".into(),
        };
        let placed = legend_beside(&e, stamp);
        assert_eq!(placed.gap, LAYER_GAP);
        assert_eq!(placed.border_width, e.line_weight().width());
        let legend = &placed.legend;
        assert_eq!(
            legend
                .swatches
                .iter()
                .map(|s| (s.label.as_str(), s.border()))
                .collect::<Vec<_>>(),
            [
                ("Core Component", "#82b366".to_owned()),
                ("Internal Dependency", "#d6b656".to_owned()),
                ("External Dependency", "#b85450".to_owned()),
            ]
        );
        assert_eq!(legend.icons.len(), 3);
        // Only the logos that fit are named: a component shows as many as its bottom bar holds.
        let mut crowded = Engine::<DeployedComponent>::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE);
        let all: Vec<&str> = tech_stack::TECH_STACK.iter().map(|t| t.id).collect();
        tech(&mut crowded, &all);
        let fits = logos_shown(crowded.layout()).len();
        assert!(fits > 0 && fits < all.len());
        // The SVG draws it beside the layers, with its heading and each logo.
        let mut scene = scene(&e);
        let right = scene.bounds.right();
        placed.add_to(&mut scene);
        let svg = scene_to_svg("Deploy", &scene);
        assert!(svg.contains(">Technology Stack</text>"));
        assert!(svg.contains(">Core Component</text>"));
        assert!(svg.contains(">AWS</text>"));
        assert!(scene.bounds.right() > right + LAYER_GAP + 300.0);
    }

    #[test]
    fn draws_layers_components_bars_and_logos() {
        let mut e = Engine::<DeployedComponent>::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE);
        e.add_at_free_position(DeployedKind::Component);
        let id = e.selected_id().unwrap().clone();
        e.replace(&id, |mut c| {
            c.props.protocols = Some(vec!["HTTPS".into(), "JWT".into()]);
            c.props.tech_stack = Some(vec!["aws".into()]);
            c.props.instance_count = Some(InstanceCount::AtLeast(50));
            c.description = Some("Serves pages".into());
            c
        });
        let svg = scene_to_svg("Deploy", &scene(&e));
        assert!(svg.contains(r##"fill="#e6e6e6" stroke="#b3b3b3" stroke-width="1""##));
        assert!(svg.contains(r##"fill="#d5e8d4" stroke="#82b366" stroke-width="1""##));
        assert!(svg.contains(">HTTPS</text>"));
        assert!(svg.contains(r#"href="data:image/png;base64,"#));
        assert!(svg.contains(">Serves pages</text>"));
        assert!(svg.contains(">50+</text>"));
        // The layer, 10 taller for the row moved down under the protocol bar.
        assert!(svg.contains(r#"viewBox="0 0 900 300""#));
        e.set_line_weight(LineWeight::Thick);
        let svg = scene_to_svg("Deploy", &scene(&e));
        assert!(svg.contains(r##"fill="#d5e8d4" stroke="#82b366" stroke-width="2""##));
        // The protocol bar has the component's border, as thick, and so does its separator.
        assert!(svg.contains(r##"fill="#f5f5f5" stroke="#82b366" stroke-width="2""##));
        // The component, its two dividers, the bar and its separator, the logo box, and the
        // instance oval.
        assert_eq!(
            svg.matches(r##"stroke="#82b366" stroke-width="2""##)
                .count(),
            7,
            "{svg}"
        );
    }

    #[test]
    fn characteristics_bars_look_like_their_layer() {
        let mut e = Engine::<DeployedComponent>::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE);
        e.add_at_free_position(DeployedKind::Layer);
        let id = e.selected_id().unwrap().clone();
        e.replace(&id, |mut l| {
            l.props.characteristics = Some(vec!["DNS".into(), "VPN".into()]);
            l
        });
        // The layer's border, the bar's, and the separator between its two labels: all in the
        // layer's border colour and width.
        let border = |width: &str| format!(r##"stroke="#b3b3b3" stroke-width="{width}""##);
        let svg = scene_to_svg("Deploy", &scene(&e));
        assert_eq!(svg.matches(&border("1")).count(), 3, "{svg}");
        e.set_line_weight(LineWeight::Thick);
        let svg = scene_to_svg("Deploy", &scene(&e));
        assert_eq!(svg.matches(&border("2")).count(), 3, "{svg}");
    }
}
