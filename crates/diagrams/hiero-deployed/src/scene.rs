//! How a deployed component diagram is drawn: layers with their characteristics bars, and
//! components with their protocol bars, bulleted descriptions, logos, and instance counts. A
//! port of the prototype's `utils/deployed-component-svg.ts`.

use hiero_core::engine::{Engine, Laid};
use hiero_core::geometry::{Point, Rect, round2};
use hiero_core::scene::{Anchor, Node, Paint, Scene};
use hiero_core::text::{MeasureText, TextStyle, Weight, wrap_text};

use crate::layout::{
    BADGE_FONT_SIZE, BULLET_LINE, LAYER_BAR_HEIGHT, LAYER_BAR_LIFT, LAYER_NAME_HEIGHT, LOGO,
    LOGO_BOX, PROTOCOL_BAR_LIFT, SEPARATOR_LEAN, characteristics, component_parts, is_layer,
    layer_name_top, protocol_segments,
};
use crate::model::{DeployedComponent, LAYER_PADDING, colors, component_colors};
use crate::tech_stack;

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
/// line leaning over between each and the next.
fn bar(rect: Rect, labels: &[String], border: &str, measure: &dyn MeasureText) -> Vec<Node> {
    let (segments, separators) = protocol_segments(rect, labels);
    let mut nodes = vec![Node::rect(
        rect,
        rect.height / 2.0,
        Paint {
            fill: Some(colors::BAR_FILL.into()),
            stroke: Some(border.into()),
            ..Paint::default()
        },
    )];
    for (from, to) in separators {
        nodes.push(Node::line(
            Point::new(round2(from.x), from.y),
            Point::new(round2(to.x), to.y),
            Paint {
                stroke: Some(border.into()),
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

fn draw_layer(layer: &Laid<DeployedComponent>, measure: &dyn MeasureText) -> Vec<Node> {
    let r = layer.rect;
    let chars = characteristics(&layer.el);
    let mut nodes = vec![Node::rect(
        r,
        CORNER_RADIUS,
        Paint::fill_stroke(colors::LAYER_FILL, colors::LAYER_BORDER, 1.5),
    )];
    if !chars.is_empty() {
        nodes.extend(bar(
            Rect::new(r.x, r.y - LAYER_BAR_LIFT, r.width, LAYER_BAR_HEIGHT),
            chars,
            colors::CHARACTERISTICS_BORDER,
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

fn draw_component(component: &Laid<DeployedComponent>, measure: &dyn MeasureText) -> Vec<Node> {
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
            Paint::stroke(color.border, 1.5),
        )
    };
    let mut nodes = vec![
        Node::rect(
            r,
            CORNER_RADIUS,
            Paint::fill_stroke(color.fill, color.border, 1.5),
        ),
        divider(parts.name_divider_y),
        divider(parts.footer_divider_y),
    ];
    if let Some(protocols) = el.props.protocols.as_ref().filter(|p| !p.is_empty()) {
        nodes.extend(bar(
            parts.protocol_bar,
            protocols,
            colors::PROTOCOL_BORDER,
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
    let mut nodes = Vec::new();
    for layer in &layers {
        nodes.extend(draw_layer(layer, measure));
    }
    for component in &components {
        nodes.extend(draw_component(component, measure));
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
        assert!(svg.contains(r##"fill="#e6e6e6" stroke="#b3b3b3" stroke-width="1.5""##));
        assert!(svg.contains(r##"fill="#d5e8d4" stroke="#82b366" stroke-width="1.5""##));
        assert!(svg.contains(">HTTPS</text>"));
        assert!(svg.contains(r#"href="data:image/png;base64,"#));
        assert!(svg.contains(">Serves pages</text>"));
        assert!(svg.contains(">50+</text>"));
        // Cropped to include the protocol bar raised over the component's top.
        assert!(svg.contains(r#"viewBox="0 0 900 290""#));
    }
}
