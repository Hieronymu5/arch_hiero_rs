//! A deployed component diagram as a draw.io file: each layer a container holding its bar,
//! name, and components, and each component a container holding its parts. A port of the
//! prototype's `utils/deployed-component-drawio.ts`.

use base64::Engine as _;
use hiero_core::engine::{Engine, Laid};
use hiero_core::export::drawio::{
    HEADING_GAP, HEADING_MIN_WIDTH, Heading, ORIGIN, Style, heading_cell, wrap,
};
use hiero_core::export::svg::escape_xml;
use hiero_core::geometry::{Point, Rect, fmt_num, round2};

use crate::layout::{
    BADGE_FONT_SIZE, BULLET_LINE, LAYER_BAR_HEIGHT, LAYER_BAR_LIFT, LAYER_NAME_HEIGHT, LOGO,
    LOGO_BOX, PROTOCOL_BAR_LIFT, SEPARATOR_LEAN, characteristics, component_parts, is_layer,
    layer_name_top, protocol_segments,
};
use crate::model::{DeployedComponent, LAYER_PADDING, colors, component_colors};
use crate::scene::CORNER_RADIUS;
use crate::tech_stack;

const BULLET_INDENT: f64 = 16.0;

/// A shape positioned relative to `parent`. `value` is HTML: escape plain text first.
fn vertex(id: &str, value: &str, style: &str, parent: &str, r: Rect) -> String {
    format!(
        "        <mxCell id=\"{}\" value=\"{}\" style=\"{style}\" vertex=\"1\" parent=\"{}\">\n          <mxGeometry x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" as=\"geometry\" />\n        </mxCell>",
        escape_xml(id),
        escape_xml(value),
        escape_xml(parent),
        fmt_num(round2(r.x)),
        fmt_num(round2(r.y)),
        fmt_num(round2(r.width)),
        fmt_num(round2(r.height))
    )
}

/// A line with no arrowheads, relative to `parent`.
fn line(id: &str, color: &str, parent: &str, from: Point, to: Point) -> String {
    let style = Style::new()
        .set("endArrow", "none")
        .set("html", 1)
        .set("strokeColor", color)
        .build();
    format!(
        "        <mxCell id=\"{}\" value=\"\" style=\"{style}\" edge=\"1\" parent=\"{}\">\n          <mxGeometry relative=\"1\" as=\"geometry\">\n            <mxPoint x=\"{}\" y=\"{}\" as=\"sourcePoint\" />\n            <mxPoint x=\"{}\" y=\"{}\" as=\"targetPoint\" />\n          </mxGeometry>\n        </mxCell>",
        escape_xml(id),
        escape_xml(parent),
        fmt_num(round2(from.x)),
        fmt_num(round2(from.y)),
        fmt_num(round2(to.x)),
        fmt_num(round2(to.y))
    )
}

fn text_style(size: f64, bold: bool, align: &str) -> Style {
    Style::new()
        .set("html", 1)
        .set("fillColor", "none")
        .set("strokeColor", "none")
        .set("whiteSpace", "nowrap")
        .set("overflow", "hidden")
        .set("align", align)
        .set("verticalAlign", "middle")
        .set("spacing", 0)
        .num("fontSize", size)
        .set("fontStyle", u8::from(bold))
        .set("fontColor", colors::TEXT)
}

fn container(style: Style) -> Style {
    style
        .set("container", 1)
        .set("collapsible", 0)
        .set("recursiveResize", 0)
}

fn bar_cells(
    id: &str,
    label_id: &str,
    parent: &str,
    bar: Rect,
    labels: &[String],
    border: &str,
) -> Vec<String> {
    let (segments, separators) = protocol_segments(bar, labels);
    let mut cells = vec![vertex(
        id,
        "",
        &Style::new()
            .set("rounded", 1)
            .set("arcSize", 50)
            .set("html", 1)
            .set("fillColor", colors::BAR_FILL)
            .set("strokeColor", border)
            .build(),
        parent,
        bar,
    )];
    for (i, s) in segments.iter().enumerate() {
        cells.push(vertex(
            &format!("{label_id}-{i}"),
            &escape_xml(&s.label),
            &text_style(11.0, true, "center").build(),
            parent,
            Rect::new(
                s.x + SEPARATOR_LEAN * 2.0,
                bar.y,
                s.width - SEPARATOR_LEAN * 4.0,
                bar.height,
            ),
        ));
    }
    for (i, (from, to)) in separators.into_iter().enumerate() {
        cells.push(line(
            &format!("{label_id}-separator-{i}"),
            border,
            parent,
            from,
            to,
        ));
    }
    cells
}

fn layer_cells(layer: &Laid<DeployedComponent>, offset: Point) -> Vec<String> {
    let id = layer.id().as_str();
    let r = layer.rect;
    let chars = characteristics(&layer.el);
    let mut cells = vec![vertex(
        id,
        "",
        &container(
            Style::new()
                .set("rounded", 1)
                .set("absoluteArcSize", 1)
                .num("arcSize", CORNER_RADIUS * 2.0)
                .set("html", 1)
                .set("fillColor", colors::LAYER_FILL)
                .set("strokeColor", colors::LAYER_BORDER),
        )
        .build(),
        "1",
        Rect::new(r.x + offset.x, r.y + offset.y, r.width, r.height),
    )];
    if !chars.is_empty() {
        cells.extend(bar_cells(
            &format!("{id}-characteristics"),
            &format!("{id}-characteristic"),
            id,
            Rect::new(0.0, -LAYER_BAR_LIFT, r.width, LAYER_BAR_HEIGHT),
            chars,
            colors::CHARACTERISTICS_BORDER,
        ));
    }
    cells.push(vertex(
        &format!("{id}-name"),
        &escape_xml(&layer.el.label),
        &text_style(16.0, true, "left").build(),
        id,
        Rect::new(
            LAYER_PADDING,
            layer_name_top(chars),
            r.width - LAYER_PADDING * 2.0,
            LAYER_NAME_HEIGHT,
        ),
    ));
    cells
}

fn component_cells(
    component: &Laid<DeployedComponent>,
    parent: &str,
    origin: Point,
) -> Vec<String> {
    let id = component.id().as_str();
    let el = &component.el;
    let r = component.rect;
    let color = component_colors(el.props.dependency);
    let tech = tech_stack::items(el.props.tech_stack.as_deref().unwrap_or_default());
    let local = Rect::new(0.0, 0.0, r.width, r.height);
    let parts = component_parts(
        local,
        el.description.as_deref(),
        tech.len(),
        el.props.instance_count.unwrap_or(1),
    );
    let mut cells = vec![
        vertex(
            id,
            "",
            &container(
                Style::new()
                    .set("rounded", 1)
                    .set("absoluteArcSize", 1)
                    .num("arcSize", CORNER_RADIUS * 2.0)
                    .set("html", 1)
                    .set("fillColor", color.fill)
                    .set("strokeColor", color.border),
            )
            .build(),
            parent,
            Rect::new(r.x - origin.x, r.y - origin.y, r.width, r.height),
        ),
        line(
            &format!("{id}-name-divider"),
            color.border,
            id,
            Point::new(0.0, parts.name_divider_y),
            Point::new(r.width, parts.name_divider_y),
        ),
        line(
            &format!("{id}-footer-divider"),
            color.border,
            id,
            Point::new(0.0, parts.footer_divider_y),
            Point::new(r.width, parts.footer_divider_y),
        ),
    ];
    if let Some(protocols) = el.props.protocols.as_ref().filter(|p| !p.is_empty()) {
        cells.extend(bar_cells(
            &format!("{id}-protocols"),
            &format!("{id}-protocol"),
            id,
            parts.protocol_bar,
            protocols,
            colors::PROTOCOL_BORDER,
        ));
    }
    cells.push(vertex(
        &format!("{id}-name"),
        &escape_xml(&el.label),
        &text_style(16.0, true, "left").build(),
        id,
        parts.name,
    ));
    for (i, column) in parts
        .columns
        .iter()
        .filter(|c| !c.rows.is_empty())
        .enumerate()
    {
        let items: String = column
            .rows
            .iter()
            .map(|row| {
                format!(
                    r#"<li style="line-height:{}px;white-space:nowrap">{}</li>"#,
                    fmt_num(BULLET_LINE),
                    escape_xml(row)
                )
            })
            .collect();
        cells.push(vertex(
            &format!("{id}-description-{i}"),
            &format!(
                r#"<ul style="margin:0;padding-left:{}px;list-style-type:disc">{items}</ul>"#,
                fmt_num(BULLET_INDENT)
            ),
            &text_style(13.0, false, "left")
                .set("whiteSpace", "wrap")
                .set("verticalAlign", "top")
                .build(),
            id,
            Rect::new(
                column.x,
                column.y,
                column.width,
                column.rows.len() as f64 * BULLET_LINE,
            ),
        ));
    }
    let inset = (LOGO_BOX - LOGO) / 2.0;
    for (i, (rect, item)) in parts.logos.iter().zip(&tech).enumerate() {
        let box_id = format!("{id}-logo-{i}");
        cells.push(vertex(
            &box_id,
            "",
            &container(
                Style::new()
                    .set("rounded", 1)
                    .set("absoluteArcSize", 1)
                    .set("arcSize", 8)
                    .set("html", 1)
                    .set("fillColor", colors::LOGO_FILL)
                    .set("strokeColor", color.border),
            )
            .build(),
            id,
            *rect,
        ));
        // draw.io keeps data URLs in styles without ";base64", as ";" separates styles.
        let data = base64::engine::general_purpose::STANDARD.encode(item.png);
        cells.push(vertex(
            &format!("{box_id}-image"),
            "",
            &Style::new()
                .set("shape", "image")
                .set("html", 1)
                .set("aspect", "fixed")
                .set("imageAspect", 0)
                .set("image", format!("data:image/png,{data}"))
                .set("tooltip", escape_xml(item.label))
                .build(),
            &box_id,
            Rect::new(inset, inset, LOGO, LOGO),
        ));
    }
    if let Some((text, badge)) = &parts.instances {
        cells.push(vertex(
            &format!("{id}-instances"),
            text,
            &Style::new()
                .set("rounded", 1)
                .set("arcSize", 50)
                .set("html", 1)
                .set("whiteSpace", "nowrap")
                .set("fillColor", colors::BAR_FILL)
                .set("strokeColor", color.border)
                .set("fontColor", colors::TEXT)
                .num("fontSize", BADGE_FONT_SIZE)
                .set("fontStyle", 1)
                .build(),
            id,
            *badge,
        ));
    }
    cells
}

/// The diagram as a draw.io file, with `heading` above it.
pub fn to_drawio(
    eng: &Engine<DeployedComponent>,
    title: &str,
    heading: Option<&Heading>,
) -> String {
    let layout = eng.layout();
    let left = layout
        .iter()
        .map(|e| e.rect.x)
        .fold(f64::INFINITY, f64::min);
    let right = layout
        .iter()
        .map(|e| e.rect.right())
        .fold(f64::NEG_INFINITY, f64::max);
    let mut tops: Vec<f64> = layout.iter().map(|e| e.rect.y).collect();
    tops.extend(
        layout
            .iter()
            .filter(|e| !is_layer(&e.el))
            .map(|c| c.rect.y - PROTOCOL_BAR_LIFT),
    );
    tops.extend(
        layout
            .iter()
            .filter(|e| is_layer(&e.el) && !characteristics(&e.el).is_empty())
            .map(|l| l.rect.y - LAYER_BAR_LIFT),
    );
    let (left, right) = if layout.is_empty() {
        (0.0, 0.0)
    } else {
        (left, right)
    };
    let top = tops.iter().copied().fold(f64::INFINITY, f64::min);
    let top = if tops.is_empty() { 0.0 } else { top };
    let heading = heading.map(|h| heading_cell(h, (right - left).max(HEADING_MIN_WIDTH)));
    let offset = Point::new(
        ORIGIN - left,
        ORIGIN + heading.as_ref().map_or(0.0, |(_, h)| h + HEADING_GAP) - top,
    );
    let mut cells: Vec<String> = Vec::new();
    if let Some((xml, _)) = &heading {
        cells.push(xml.clone());
    }
    for layer in layout.iter().filter(|e| is_layer(&e.el)) {
        cells.extend(layer_cells(layer, offset));
    }
    for component in layout.iter().filter(|e| !is_layer(&e.el)) {
        let layer = component.el.parent_id.as_ref().and_then(|p| eng.laid(p));
        cells.extend(match layer {
            Some(layer) => component_cells(component, layer.id().as_str(), layer.rect.position()),
            None => component_cells(component, "1", Point::new(-offset.x, -offset.y)),
        });
    }
    wrap(title, &cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DeployedKind;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    #[test]
    fn layers_hold_components_with_their_parts() {
        let mut e = Engine::<DeployedComponent>::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE);
        e.add_at_free_position(DeployedKind::Component);
        let id = e.selected_id().unwrap().clone();
        e.replace(&id, |mut c| {
            c.props.protocols = Some(vec!["HTTPS".into()]);
            c.props.tech_stack = Some(vec!["aws".into()]);
            c.props.instance_count = Some(2);
            c.description = Some("One\nTwo".into());
            c
        });
        let xml = to_drawio(&e, "D", None);
        assert!(xml.contains("container=1;collapsible=0;recursiveResize=0"));
        assert!(xml.contains(&format!("{id}-protocol-0")));
        assert!(xml.contains("data:image/png,"));
        assert!(xml.contains("&lt;li style"));
        assert!(xml.contains(&format!(r#"id="{id}-instances" value="2""#)));
    }
}
