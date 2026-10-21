//! How a deployed component diagram is laid out: layers stacked from top to bottom, all as
//! wide as the widest, each holding its components side by side. Where each element sits
//! follows from its order. A port of the prototype's `utils/deployed-component-layout.ts`.

use std::collections::{HashMap, HashSet};

use hiero_core::engine::El;
use hiero_core::geometry::{GRID, Point, Rect, Size};
use hiero_core::model::ElementId;

use crate::model::{DeployedComponent, DeployedKind, LAYER_PADDING};

/// Where the top layer's top-left corner sits.
pub const LAYERS_ORIGIN: Point = Point::new(GRID, GRID);
/// The gap between one layer and the next.
pub const LAYER_GAP: f64 = GRID;
/// The characteristics bar across a layer's top, raised half its height over the edge.
pub const LAYER_BAR_HEIGHT: f64 = 20.0;
pub const LAYER_BAR_LIFT: f64 = LAYER_BAR_HEIGHT / 2.0;
/// The narrowest a layer is: room for three components of one column.
pub const MIN_LAYER_WIDTH: f64 = 860.0;
/// The room an empty layer leaves under its header to drop a component in.
const EMPTY_LAYER_ROOM: f64 = 80.0;
/// The height of a layer's name.
pub const LAYER_NAME_HEIGHT: f64 = 40.0;

/// The parts of a component, top to bottom, and how its description is set out.
pub const COLUMN_WIDTH: f64 = 260.0;
pub const PROTOCOL_BAR: f64 = 20.0;
pub const PROTOCOL_BAR_LIFT: f64 = 10.0;
pub const NAME: f64 = 40.0;
pub const DESCRIPTION_PADDING: f64 = 10.0;
pub const BULLET_LINE: f64 = 20.0;
pub const MIN_ROWS: usize = 4;
pub const FOOTER: f64 = 40.0;
pub const INSET: f64 = 12.0;
pub const SEPARATOR_LEAN: f64 = 5.0;

/// A logo's box, the gap between boxes, and the logo inside.
pub const LOGO_BOX: f64 = 24.0;
pub const LOGO_GAP: f64 = 6.0;
pub const LOGO: f64 = 18.0;

/// The instance count's oval: its height, padding, text size, and a digit's width.
pub const BADGE_HEIGHT: f64 = 24.0;
pub const BADGE_PADDING_X: f64 = 8.0;
pub const BADGE_FONT_SIZE: f64 = 13.0;
pub const BADGE_DIGIT_WIDTH: f64 = 8.0;

/// How far a layer's name is below its top: under its characteristics bar, if any.
pub fn layer_name_top(characteristics: &[String]) -> f64 {
    if characteristics.is_empty() {
        0.0
    } else {
        LAYER_BAR_HEIGHT - LAYER_BAR_LIFT
    }
}

/// The height of a layer's header, above its components.
pub fn layer_header(characteristics: &[String]) -> f64 {
    layer_name_top(characteristics) + LAYER_NAME_HEIGHT
}

/// The height of a layer with no components, leaving room to drop one in.
pub fn empty_layer_height(characteristics: &[String]) -> f64 {
    layer_header(characteristics) + EMPTY_LAYER_ROOM + LAYER_PADDING
}

/// A description's bullets: each line with something on it, trimmed.
pub fn description_rows(description: Option<&str>) -> Vec<String> {
    description
        .unwrap_or_default()
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A description's bullets in columns: one for up to four, else two, the first holding the
/// extra one.
pub fn bullet_columns(rows: &[String]) -> Vec<Vec<String>> {
    if rows.len() <= MIN_ROWS {
        return vec![rows.to_vec()];
    }
    let half = rows.len().div_ceil(2);
    vec![rows[..half].to_vec(), rows[half..].to_vec()]
}

/// The size a component takes for its description.
pub fn component_size(description: Option<&str>) -> Size {
    let columns = bullet_columns(&description_rows(description));
    let rows = columns
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(MIN_ROWS);
    let n = columns.len() as f64;
    Size::new(
        n * COLUMN_WIDTH + (n - 1.0) * LAYER_PADDING,
        bar_inside() + NAME + DESCRIPTION_PADDING * 2.0 + rows as f64 * BULLET_LINE + FOOTER,
    )
}

fn bar_inside() -> f64 {
    PROTOCOL_BAR - PROTOCOL_BAR_LIFT
}

/// How wide the instance count's oval is: a circle for one digit, longer for more.
pub fn badge_width(text: &str) -> f64 {
    BADGE_HEIGHT.max(BADGE_PADDING_X * 2.0 + text.chars().count() as f64 * BADGE_DIGIT_WIDTH)
}

/// A column of bullets: its left edge, its first bullet's top, its width, and its rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub rows: Vec<String>,
}

/// The parts of a laid-out component.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentParts {
    pub protocol_bar: Rect,
    pub name: Rect,
    pub name_divider_y: f64,
    pub footer_divider_y: f64,
    pub columns: Vec<Column>,
    /// The logos that fit in its bottom bar, each with its box.
    pub logos: Vec<Rect>,
    /// The instance count in its oval, for more than one instance.
    pub instances: Option<(String, Rect)>,
}

/// Where each part of a component goes in `rect`.
pub fn component_parts(
    rect: Rect,
    description: Option<&str>,
    logo_count: usize,
    instance_count: u32,
) -> ComponentParts {
    let name_top = rect.y + bar_inside();
    let name_divider_y = name_top + NAME;
    let footer_divider_y = rect.y + rect.height - FOOTER;
    let columns = bullet_columns(&description_rows(description));
    let column_width = (rect.width - INSET * 2.0) / columns.len() as f64;
    let text = instance_count.to_string();
    let width = badge_width(&text);
    let instances = (instance_count > 1).then(|| {
        (
            text,
            Rect::new(
                rect.x + rect.width - INSET - width,
                footer_divider_y + (FOOTER - BADGE_HEIGHT) / 2.0,
                width,
                BADGE_HEIGHT,
            ),
        )
    });
    let end = instances
        .as_ref()
        .map_or(rect.x + rect.width - INSET, |(_, b)| b.x - LOGO_GAP);
    let room = end - (rect.x + INSET);
    let fit = ((room + LOGO_GAP) / (LOGO_BOX + LOGO_GAP)).floor().max(0.0) as usize;
    let logos = (0..logo_count.min(fit))
        .map(|i| {
            Rect::new(
                rect.x + INSET + i as f64 * (LOGO_BOX + LOGO_GAP),
                footer_divider_y + (FOOTER - LOGO_BOX) / 2.0,
                LOGO_BOX,
                LOGO_BOX,
            )
        })
        .collect();
    ComponentParts {
        protocol_bar: Rect::new(rect.x, rect.y - PROTOCOL_BAR_LIFT, rect.width, PROTOCOL_BAR),
        name: Rect::new(rect.x + INSET, name_top, rect.width - INSET * 2.0, NAME),
        name_divider_y,
        footer_divider_y,
        columns: columns
            .into_iter()
            .enumerate()
            .map(|(i, rows)| Column {
                x: rect.x + INSET + i as f64 * column_width,
                y: name_divider_y + DESCRIPTION_PADDING,
                width: column_width,
                rows,
            })
            .collect(),
        logos,
        instances,
    }
}

/// A label in a bar, with the width it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub label: String,
    pub x: f64,
    pub width: f64,
}

/// The labels across `bar`, each taking an equal share, with a line leaning over between each
/// and the next.
pub fn protocol_segments(bar: Rect, labels: &[String]) -> (Vec<Segment>, Vec<(Point, Point)>) {
    let width = bar.width / labels.len().max(1) as f64;
    let segments = labels
        .iter()
        .enumerate()
        .map(|(i, label)| Segment {
            label: label.clone(),
            x: bar.x + i as f64 * width,
            width,
        })
        .collect();
    let separators = (1..labels.len())
        .map(|i| {
            let x = bar.x + i as f64 * width;
            (
                Point::new(x - SEPARATOR_LEAN, bar.y + bar.height),
                Point::new(x + SEPARATOR_LEAN, bar.y),
            )
        })
        .collect();
    (segments, separators)
}

type Dc = El<DeployedComponent>;

pub fn is_layer(element: &Dc) -> bool {
    element.kind == DeployedKind::Layer
}

pub fn characteristics(element: &Dc) -> &[String] {
    element.props.characteristics.as_deref().unwrap_or_default()
}

/// The size an element takes before it's arranged, for working out its middle.
pub fn provisional_size(element: &Dc) -> Size {
    let natural = if is_layer(element) {
        Size::new(
            MIN_LAYER_WIDTH,
            empty_layer_height(characteristics(element)),
        )
    } else {
        component_size(element.description.as_deref())
    };
    Size::new(
        element.width.unwrap_or(natural.width),
        element.height.unwrap_or(natural.height),
    )
}

fn sorted_by(elements: &[Dc], keep: impl Fn(&Dc) -> bool, key: impl Fn(&Dc) -> f64) -> Vec<Dc> {
    let mut indexed: Vec<(usize, &Dc)> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| keep(e))
        .collect();
    indexed.sort_by(|(ai, a), (bi, b)| {
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ai.cmp(bi))
    });
    indexed.into_iter().map(|(_, e)| e.clone()).collect()
}

/// The layers from top to bottom: by their middles, keeping the list's order for ties.
pub fn layers_in_order(elements: &[Dc]) -> Vec<Dc> {
    sorted_by(elements, is_layer, |e| {
        e.y + provisional_size(e).height / 2.0
    })
}

/// The components in a layer from left to right: by their middles.
pub fn components_in_order(elements: &[Dc], layer: &ElementId) -> Vec<Dc> {
    sorted_by(
        elements,
        |e| !is_layer(e) && e.parent_id.as_ref() == Some(layer),
        |e| e.x + provisional_size(e).width / 2.0,
    )
}

fn nearest_layer<'a>(element: &Dc, layers: &'a [Dc]) -> &'a Dc {
    let middle = element.y + provisional_size(element).height / 2.0;
    let distance = |l: &Dc| (l.y + provisional_size(l).height / 2.0 - middle).abs();
    layers
        .iter()
        .reduce(|nearest, l| {
            if distance(l) < distance(nearest) {
                l
            } else {
                nearest
            }
        })
        .expect("there's at least one layer")
}

/// The elements laid out: layers stacked from the top, as wide as the widest needs, each
/// holding its components side by side, all as tall as its tallest. A component outside any
/// layer goes in the nearest one. With no layers, components stay where they are.
pub fn arrange_layers(elements: Vec<Dc>) -> Vec<Dc> {
    let layers = layers_in_order(&elements);
    if layers.is_empty() {
        return elements
            .into_iter()
            .map(|mut e| {
                if !is_layer(&e) {
                    let size = component_size(e.description.as_deref());
                    e.width = Some(size.width);
                    e.height = Some(size.height);
                }
                e
            })
            .collect();
    }
    let layer_ids: HashSet<&ElementId> = layers.iter().map(|l| &l.id).collect();
    let placed_in: Vec<Dc> = elements
        .iter()
        .map(|e| {
            if is_layer(e) || e.parent_id.as_ref().is_some_and(|p| layer_ids.contains(p)) {
                e.clone()
            } else {
                let nearest = nearest_layer(e, &layers).id.clone();
                e.clone().with_parent(Some(nearest))
            }
        })
        .collect();
    struct Row {
        layer: Dc,
        components: Vec<(Dc, Size)>,
        width: f64,
        height: f64,
    }
    let rows: Vec<Row> = layers
        .iter()
        .map(|layer| {
            let components: Vec<(Dc, Size)> = components_in_order(&placed_in, &layer.id)
                .into_iter()
                .map(|c| {
                    let size = component_size(c.description.as_deref());
                    (c, size)
                })
                .collect();
            let width = components
                .iter()
                .enumerate()
                .map(|(i, (_, s))| s.width + if i > 0 { LAYER_PADDING } else { 0.0 })
                .sum();
            let height = components.iter().map(|(_, s)| s.height).fold(0.0, f64::max);
            Row {
                layer: layer.clone(),
                components,
                width,
                height,
            }
        })
        .collect();
    let layer_width = rows
        .iter()
        .map(|r| r.width + LAYER_PADDING * 2.0)
        .fold(MIN_LAYER_WIDTH, f64::max);
    let mut placed: HashMap<ElementId, Rect> = HashMap::new();
    let mut y = LAYERS_ORIGIN.y;
    for row in &rows {
        let header = layer_header(characteristics(&row.layer));
        let height = if row.components.is_empty() {
            empty_layer_height(characteristics(&row.layer))
        } else {
            header + row.height + LAYER_PADDING
        };
        placed.insert(
            row.layer.id.clone(),
            Rect::new(LAYERS_ORIGIN.x, y, layer_width, height),
        );
        let mut x = LAYERS_ORIGIN.x + LAYER_PADDING;
        for (component, size) in &row.components {
            placed.insert(
                component.id.clone(),
                Rect::new(x, y + header, size.width, row.height),
            );
            x += size.width + LAYER_PADDING;
        }
        y += height + LAYER_GAP;
    }
    placed_in
        .into_iter()
        .map(|mut e| {
            if let Some(r) = placed.get(&e.id) {
                e.x = r.x;
                e.y = r.y;
                e.width = Some(r.width);
                e.height = Some(r.height);
                if is_layer(&e) {
                    e.parent_id = None;
                }
            }
            e
        })
        .collect()
}

/// The elements with the layers in `layers` from top to bottom, and the components given for
/// any layer in `components` from left to right, once arranged.
pub fn in_order(
    elements: Vec<Dc>,
    layers: &[ElementId],
    components: &[(ElementId, Vec<ElementId>)],
) -> Vec<Dc> {
    let mut moved_y: HashMap<ElementId, f64> = HashMap::new();
    let mut moved_x: HashMap<ElementId, (f64, ElementId)> = HashMap::new();
    let mut y = LAYERS_ORIGIN.y;
    for id in layers {
        if let Some(layer) = elements.iter().find(|e| e.id == *id) {
            moved_y.insert(id.clone(), y);
            y += provisional_size(layer).height + LAYER_GAP;
        }
    }
    for (layer, ids) in components {
        let mut x = LAYERS_ORIGIN.x + LAYER_PADDING;
        for id in ids {
            if let Some(component) = elements.iter().find(|e| e.id == *id) {
                moved_x.insert(id.clone(), (x, layer.clone()));
                x += provisional_size(component).width + LAYER_PADDING;
            }
        }
    }
    elements
        .into_iter()
        .map(|mut e| {
            if let Some(y) = moved_y.get(&e.id) {
                e.y = *y;
            }
            if let Some((x, layer)) = moved_x.get(&e.id) {
                e.x = *x;
                e.parent_id = Some(layer.clone());
            }
            e
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_description() {
        assert_eq!(component_size(None), Size::new(260.0, 190.0));
        let five = "a\nb\nc\nd\ne";
        assert_eq!(component_size(Some(five)), Size::new(540.0, 190.0));
        let columns = bullet_columns(&description_rows(Some(five)));
        assert_eq!(columns[0].len(), 3);
        let ten = (0..10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(component_size(Some(&ten)).height, 210.0);
    }

    #[test]
    fn logos_stop_short_of_the_count() {
        let parts = component_parts(Rect::new(0.0, 0.0, 260.0, 190.0), None, 20, 3);
        let (text, badge) = parts.instances.unwrap();
        assert_eq!(text, "3");
        assert_eq!(badge, Rect::new(224.0, 158.0, 24.0, 24.0));
        // (218 - 12 + 6) / 30 = 7 logos.
        assert_eq!(parts.logos.len(), 7);
    }

    #[test]
    fn segments_share_the_bar() {
        let (segments, separators) = protocol_segments(
            Rect::new(0.0, 0.0, 300.0, 20.0),
            &["HTTPS".into(), "JWT".into(), "XSRF".into()],
        );
        assert_eq!(segments[1].x, 100.0);
        assert_eq!(
            separators[0],
            (Point::new(95.0, 20.0), Point::new(105.0, 0.0))
        );
    }
}
