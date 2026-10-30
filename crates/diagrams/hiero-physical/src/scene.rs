//! How a physical diagram is drawn: networks and zones as square regions with their names in
//! their corners, the figures with their names on the zones' colour, and arrows with their
//! protocol and port.

use std::collections::HashMap;

use hiero_core::color::{black_or_white, readable_text};
use hiero_core::engine::{Engine, Laid};
use hiero_core::figures::PhysicalFigure;
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{
    ArrowLook, ArrowTag, DiagramColors, Look, LookElement, NameAt, Shape, draw_diagram,
    draw_element, figure_badge_rect, figure_name_bottom,
};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::routing::{
    CONNECTOR_BOX_FONT_SIZE, ConnectionEnds, Route, Side, route_connections,
    transformation_placement,
};
use hiero_core::scene::Scene;
use hiero_core::text::Weight;

use crate::model::{
    BLUE_ZONE, BORDER_WIDTH, GENERIC_DATABASE, Physical, PhysicalKind, ZoneColorInfo,
};
use crate::rules::{database_of, figure_of, holds_stack, instance_count_of, is_stacked};

/// How far under a figure's name an arrow at its bottom ends, so it almost touches it.
const NAME_CLEARANCE: f64 = 2.0;

/// The colours of a zone outside a network, in its own colour or blue. Unset otherwise.
fn zone_color_of(
    eng: &Engine<Physical>,
    element: &hiero_core::engine::El<Physical>,
) -> Option<ZoneColorInfo> {
    if !element.kind.has_zone_color() || is_stacked(eng, element) {
        return None;
    }
    Some(
        element
            .props
            .zone_color
            .map_or(BLUE_ZONE, crate::model::ZoneColor::info),
    )
}

/// How an element looks.
pub fn look_of(eng: &Engine<Physical>, element: &Laid<Physical>) -> LookElement {
    let colors = eng.colors();
    let kind = element.el.kind;
    let look = if element.is_region {
        if let Some(zone) = zone_color_of(eng, &element.el) {
            // A zone on its own: a rounded box with a dashed border, named top-left.
            Look {
                fill: Some(zone.fill.into()),
                text_color: Some(zone.border.into()),
                border_color: Some(zone.border.into()),
                border_width: Some(BORDER_WIDTH),
                dashed: true,
                square: false,
                name_at: NameAt::TopLeft,
                outline_on_top: holds_stack(eng, &element.el),
                ..Look::new(Shape::Container)
            }
        } else {
            let fill = kind.fill().of(colors).to_owned();
            let text = match kind {
                PhysicalKind::Network => colors.network_names.clone(),
                PhysicalKind::Zone => colors.zone_names.clone(),
                _ => readable_text(&fill).to_owned(),
            };
            Look {
                fill: Some(fill),
                text_color: Some(text),
                border_width: Some(BORDER_WIDTH),
                square: true,
                name_at: if kind == PhysicalKind::Zone {
                    NameAt::BottomLeft
                } else {
                    NameAt::TopLeft
                },
                outline_on_top: holds_stack(eng, &element.el),
                ..Look::new(Shape::Container)
            }
        }
    } else {
        let fill = kind.fill().of(colors).to_owned();
        let figure = figure_of(&element.el);
        let database = kind
            .has_database_type()
            .then(|| database_of(&element.el).unwrap_or(GENERIC_DATABASE));
        // The instance count in a black box beside the figure's bottom-right corner.
        let badge = figure
            .zip(instance_count_of(&element.el))
            .map(|(figure, count)| {
                let label = count.to_string();
                let width = eng
                    .measure()
                    .width(&label, CONNECTOR_BOX_FONT_SIZE, Weight::SemiBold);
                ArrowTag {
                    rect: figure_badge_rect(element.rect, figure, width),
                    label,
                    fill: "#000000".into(),
                    text_color: "#ffffff".into(),
                    border: None,
                }
            });
        Look {
            detail_color: Some(black_or_white(&fill).to_owned()),
            // The name in the labels' colour with no background.
            text_color: Some(colors.labels.clone()),
            on_top: true,
            figure_text: database.map(|d| d.abbreviation.to_owned()),
            figure_badge: badge,
            fill: Some(fill),
            ..Look::new(figure.map_or(Shape::Box, Shape::Physical))
        }
    };
    LookElement {
        id: element.el.id.clone(),
        parent_id: element.el.parent_id.clone(),
        label: element.el.label.clone(),
        description: None,
        look,
        rect: element.rect,
    }
}

/// An arrow as it's drawn.
#[derive(Debug, Clone)]
pub struct DrawnArrow {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    /// The arrow back the other way, when this line draws it too.
    pub reverse: Option<ConnectionId>,
    pub look: ArrowLook,
    pub route: Route,
}

/// For each arrow drawn as part of an earlier one, that one: an arrow back the other way over
/// the same protocol and port shares its line, which then points both ways.
fn reverses(eng: &Engine<Physical>) -> HashMap<ConnectionId, ConnectionId> {
    let mut drawn_by = HashMap::new();
    let connections = eng.connections();
    for (i, c) in connections.iter().enumerate() {
        let earlier = connections[..i].iter().find(|d| {
            d.source_id == c.target_id
                && d.target_id == c.source_id
                && d.props == c.props
                && !drawn_by.contains_key(&d.id)
        });
        if let Some(d) = earlier {
            drawn_by.insert(c.id.clone(), d.id.clone());
        }
    }
    drawn_by
}

/// Every arrow between evenly spread points on its elements' figures, with its protocol and
/// port in a box on the middle of the line. An end at a figure's bottom moves down to just
/// under its name. Two arrows between the same elements in opposite directions, over the same
/// protocol and port, are drawn as one line pointing both ways.
pub fn drawn_arrows(eng: &Engine<Physical>, layout: &[Laid<Physical>]) -> Vec<DrawnArrow> {
    let rects: HashMap<ElementId, _> = layout
        .iter()
        .map(|e| {
            (
                e.el.id.clone(),
                <Physical as hiero_core::engine::DiagramType>::arrow_rect(eng, e),
            )
        })
        .collect();
    let by_id: HashMap<&ElementId, &Laid<Physical>> =
        layout.iter().map(|e| (&e.el.id, e)).collect();
    let drawn_by = reverses(eng);
    let reverse_of: HashMap<&ConnectionId, &ConnectionId> =
        drawn_by.iter().map(|(c, d)| (d, c)).collect();
    let ends: Vec<ConnectionEnds> = eng
        .connections()
        .iter()
        .filter(|c| !drawn_by.contains_key(&c.id))
        .map(|c| ConnectionEnds {
            id: c.id.clone(),
            source_id: c.source_id.clone(),
            target_id: c.target_id.clone(),
            manual_offset: None,
        })
        .collect();
    let routes = route_connections(&rects, &ends);
    let colors = eng.colors();
    let measure = eng.measure();
    let below = |point: Point, element: &Laid<Physical>| match figure_of(&element.el) {
        Some(figure) => Point::new(
            point.x,
            figure_name_bottom(element.rect, &element.el.label, figure, measure) + NAME_CLEARANCE,
        ),
        None => point,
    };
    let mut arrows: Vec<DrawnArrow> = eng
        .connections()
        .iter()
        .filter_map(|c| {
            let route = *routes.get(&c.id)?;
            let source = by_id.get(&c.source_id)?;
            let target = by_id.get(&c.target_id)?;
            let from = if route.from_side == Side::Bottom {
                below(route.from, source)
            } else {
                route.from
            };
            let to = if route.to_side == Side::Bottom {
                below(route.to, target)
            } else {
                route.to
            };
            let label = c.props.tag();
            let tag = (!label.is_empty()).then(|| {
                let width = measure.width(&label, CONNECTOR_BOX_FONT_SIZE, Weight::SemiBold);
                ArrowTag {
                    rect: transformation_placement(from, to, width).0,
                    label,
                    fill: colors.protocols.clone(),
                    text_color: black_or_white(&colors.protocols).to_owned(),
                    border: Some(colors.lines.clone()),
                }
            });
            let reverse = reverse_of.get(&c.id).map(|r| (*r).clone());
            Some(DrawnArrow {
                id: c.id.clone(),
                source_id: c.source_id.clone(),
                target_id: c.target_id.clone(),
                look: ArrowLook {
                    from,
                    to,
                    tag,
                    two_heads: reverse.is_some(),
                    ..ArrowLook::default()
                },
                reverse,
                route,
            })
        })
        .collect();
    clear_tags(eng, layout, &mut arrows);
    arrows
}

/// Whether `id` is `zone` or inside it.
fn within(eng: &Engine<Physical>, id: &ElementId, zone: &ElementId) -> bool {
    let mut current = Some(id);
    while let Some(id) = current {
        if id == zone {
            return true;
        }
        current = eng.element(id).and_then(|e| e.parent_id.as_ref());
    }
    false
}

/// Where the line from `outside` to `inside` enters `rect`.
fn entry_point(rect: Rect, outside: Point, inside: Point) -> Option<Point> {
    if rect.contains_point(outside) || !rect.contains_point(inside) {
        return None;
    }
    let (dx, dy) = (inside.x - outside.x, inside.y - outside.y);
    // The latest of the times the line passes each pair of edges' lines going in.
    let enter = |from: f64, d: f64, low: f64, high: f64| {
        if d == 0.0 {
            0.0
        } else {
            ((low - from) / d).min((high - from) / d)
        }
    };
    let t = enter(outside.x, dx, rect.x, rect.right())
        .max(enter(outside.y, dy, rect.y, rect.bottom()))
        .clamp(0.0, 1.0);
    Some(Point::new(outside.x + dx * t, outside.y + dy * t))
}

/// Where arrows cross into zones that have a firewall: each arrow from outside such a zone,
/// including a line that points both ways, at the point it enters. A line from one such zone
/// into another crosses both at the same point.
fn firewall_points(
    eng: &Engine<Physical>,
    layout: &[Laid<Physical>],
    arrows: &[DrawnArrow],
) -> Vec<(ConnectionId, Point)> {
    let mut points = Vec::new();
    for zone in layout
        .iter()
        .filter(|z| z.el.kind.has_firewall() && z.el.props.firewall)
    {
        let zone_id = &zone.el.id;
        for arrow in arrows {
            let source_in = within(eng, &arrow.source_id, zone_id);
            let target_in = within(eng, &arrow.target_id, zone_id);
            let ends = match (source_in, target_in) {
                (false, true) => Some((arrow.look.from, arrow.look.to)),
                (true, false) if arrow.reverse.is_some() => Some((arrow.look.to, arrow.look.from)),
                _ => None,
            };
            if let Some(at) =
                ends.and_then(|(outside, inside)| entry_point(zone.rect, outside, inside))
            {
                points.push((arrow.id.clone(), at));
            }
        }
    }
    points
}

/// The area of a zone's firewall, two bricks tall, centred on `at`.
fn firewall_rect(at: Point) -> Rect {
    let size = PhysicalFigure::LowFirewall.size();
    Rect::new(
        at.x - size.width / 2.0,
        at.y - size.height / 2.0,
        size.width,
        size.height,
    )
}

/// Moves an arrow's protocol and port off a zone's firewall that would hide them, to the middle
/// of the longest stretch of the line between its ends and its firewalls.
fn clear_tags(eng: &Engine<Physical>, layout: &[Laid<Physical>], arrows: &mut [DrawnArrow]) {
    let points = firewall_points(eng, layout, arrows);
    for arrow in arrows.iter_mut() {
        let on_line: Vec<Point> = points
            .iter()
            .filter(|(id, _)| *id == arrow.id)
            .map(|(_, p)| *p)
            .collect();
        let (from, to) = (arrow.look.from, arrow.look.to);
        let Some(tag) = arrow.look.tag.as_mut() else {
            continue;
        };
        if !on_line
            .iter()
            .any(|p| firewall_rect(*p).overlaps(&tag.rect))
        {
            continue;
        }
        let length = from.distance(to);
        if length == 0.0 {
            continue;
        }
        let mut stops: Vec<f64> = on_line.iter().map(|p| from.distance(*p) / length).collect();
        stops.extend([0.0, 1.0]);
        stops.sort_by(f64::total_cmp);
        let (a, b) = stops
            .windows(2)
            .map(|w| (w[0], w[1]))
            .max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0)))
            .expect("a line has a start and an end");
        let middle = (a + b) / 2.0;
        let center = tag.rect.center();
        tag.rect.x += from.x + (to.x - from.x) * middle - center.x;
        tag.rect.y += from.y + (to.y - from.y) * middle - center.y;
    }
}

/// The firewalls on the edges of zones that have one, where arrows come in
/// ([`firewall_points`]), one at each point. They aren't elements, so they can't be selected;
/// they come and go with the zone's Firewall property.
pub fn edge_firewalls(
    eng: &Engine<Physical>,
    layout: &[Laid<Physical>],
    arrows: &[DrawnArrow],
) -> Vec<LookElement> {
    let fill = PhysicalKind::Firewall.fill().of(eng.colors()).to_owned();
    let mut points: Vec<(ConnectionId, Point)> = Vec::new();
    for (id, at) in firewall_points(eng, layout, arrows) {
        if !points.iter().any(|(_, p)| p.distance(at) < 1.0) {
            points.push((id, at));
        }
    }
    points
        .into_iter()
        .map(|(id, at)| LookElement {
            id: ElementId::from(format!("{id}-firewall-{}-{}", at.x.round(), at.y.round())),
            parent_id: None,
            label: String::new(),
            description: None,
            look: Look {
                detail_color: Some(black_or_white(&fill).to_owned()),
                fill: Some(fill.clone()),
                on_top: true,
                ..Look::new(Shape::Physical(PhysicalFigure::LowFirewall))
            },
            rect: firewall_rect(at),
        })
        .collect()
}

/// The base colours the shared drawing uses: outlines in the lines colour, arrows in theirs.
pub fn base_colors(eng: &Engine<Physical>) -> DiagramColors {
    DiagramColors {
        borders: eng.colors().lines.clone(),
        lines: eng.colors().arrows.clone(),
        ..DiagramColors::default()
    }
}

/// The diagram drawn from `layout`. Zones' firewalls go over the arrows that pass them.
pub fn scene_of(eng: &Engine<Physical>, layout: &[Laid<Physical>]) -> Scene {
    let elements: Vec<LookElement> = layout.iter().map(|e| look_of(eng, e)).collect();
    let drawn = drawn_arrows(eng, layout);
    let firewalls = edge_firewalls(eng, layout, &drawn);
    let arrows: Vec<ArrowLook> = drawn.into_iter().map(|a| a.look).collect();
    let colors = base_colors(eng);
    let mut scene = draw_diagram(&elements, &arrows, eng.measure(), &colors);
    for firewall in &firewalls {
        scene
            .nodes
            .extend(draw_element(firewall, eng.measure(), &colors));
        let (b, r) = (scene.bounds, firewall.rect);
        let (x, y) = (b.x.min(r.x), b.y.min(r.y));
        scene.bounds = Rect::new(
            x,
            y,
            b.right().max(r.right()) - x,
            b.bottom().max(r.bottom()) - y,
        );
    }
    scene
}

pub fn scene(eng: &Engine<Physical>) -> Scene {
    scene_of(eng, eng.layout())
}

/// The diagram as a draw.io file, with `heading` above it.
pub fn to_drawio(
    eng: &Engine<Physical>,
    title: &str,
    heading: Option<&hiero_core::export::drawio::Heading>,
) -> String {
    let mut elements: Vec<LookElement> = eng.layout().iter().map(|e| look_of(eng, e)).collect();
    let drawn = drawn_arrows(eng, eng.layout());
    elements.extend(edge_firewalls(eng, eng.layout(), &drawn));
    let arrows: Vec<hiero_core::export::drawio::DrawioArrow> = drawn
        .into_iter()
        .map(|a| hiero_core::export::drawio::DrawioArrow {
            id: a.id,
            source_id: a.source_id,
            target_id: a.target_id,
            look: a.look,
        })
        .collect();
    hiero_core::export::drawio::diagram_to_drawio(
        title,
        &elements,
        &arrows,
        heading,
        &base_colors(eng),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PhysicalColors;
    use hiero_core::export::svg::scene_to_svg;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    #[test]
    fn draws_zones_networks_and_figures() {
        let mut e =
            Engine::<Physical>::new_diagram(PhysicalColors::default()).with_measure(&ESTIMATE);
        let zone = e.elements()[2].id.clone();
        e.select(&zone);
        e.add_at_free_position(PhysicalKind::Database);
        let svg = scene_to_svg("Infra", &scene(&e));
        // The End User zone on its own: blue and dashed.
        assert!(svg.contains(
            r##"fill="#dae8fc" stroke="#2d5f9e" stroke-width="2" stroke-dasharray="8 4""##
        ));
        // The network's outline drawn over its zones.
        assert!(svg.contains(r##"fill="none" stroke="#333333" stroke-width="2""##));
        // The database's abbreviation.
        assert!(svg.contains(">DB</text>"));
    }

    #[test]
    fn arrows_back_and_forth_over_one_port_share_a_line() {
        let mut e =
            Engine::<Physical>::new_diagram(PhysicalColors::default()).with_measure(&ESTIMATE);
        let zone = e.elements()[2].id.clone();
        let mut add = || {
            e.select(&zone);
            e.add_at_free_position(PhysicalKind::Server);
            e.selection[0].clone()
        };
        let (a, b) = (add(), add());
        let there = e.add_connection(&a, &b).unwrap();
        let back = e.add_connection(&b, &a).unwrap();
        let drawn = drawn_arrows(&e, e.layout());
        assert_eq!(drawn.len(), 1);
        assert_eq!(drawn[0].id, there);
        assert_eq!(drawn[0].reverse, Some(back.clone()));
        assert!(drawn[0].look.two_heads && !drawn[0].look.bidirectional);
        // In draw.io, one edge with a head at each end.
        let xml = to_drawio(&e, "Infra", None);
        assert_eq!(xml.matches("startArrow=block").count(), 1);
        assert!(!xml.contains("-return\""));

        // Over another port, they're two arrows again.
        let mut props = e.connection(&back).unwrap().props.clone();
        props.port = "8443".into();
        e.set_connection_props(&back, props);
        let drawn = drawn_arrows(&e, e.layout());
        assert_eq!(drawn.len(), 2);
        assert!(
            drawn
                .iter()
                .all(|d| !d.look.two_heads && d.reverse.is_none())
        );
    }

    #[test]
    fn instance_counts_are_boxes_on_the_figure() {
        let mut e =
            Engine::<Physical>::new_diagram(PhysicalColors::default()).with_measure(&ESTIMATE);
        let zone = e.elements()[2].id.clone();
        e.select(&zone);
        e.add_at_free_position(PhysicalKind::Server);
        let server = e.selection[0].clone();
        e.replace(&server, |mut el| {
            el.props.instance_count = Some(hiero_core::count::InstanceCount::Between(50, 100));
            el
        });
        let laid = e.laid(&server).unwrap().clone();
        let look = look_of(&e, &laid);
        let badge = look.look.figure_badge.expect("a count over one has a box");
        assert_eq!(badge.label, "50-100");
        // Beside the figure's bottom-right corner, black with white text.
        let figure = hiero_core::figures::figure_rect(laid.rect, figure_of(&laid.el).unwrap());
        assert_eq!(
            badge.rect.x,
            figure.right() + hiero_core::look::FIGURE_BADGE_GAP
        );
        assert_eq!(badge.rect.bottom(), figure.bottom());
        assert_eq!(
            (badge.fill.as_str(), badge.text_color.as_str()),
            ("#000000", "#ffffff")
        );
        let svg = scene_to_svg("Infra", &scene(&e));
        assert!(svg.contains(">50-100</text>"));
        // Its ends are round, so a single digit is a circle.
        e.replace(&server, |mut el| {
            el.props.instance_count = Some(hiero_core::count::InstanceCount::Exactly(3));
            el
        });
        let badge = look_of(&e, &e.laid(&server).unwrap().clone())
            .look
            .figure_badge
            .unwrap();
        assert_eq!(badge.rect.width, badge.rect.height);
        let svg = scene_to_svg("Infra", &scene(&e));
        assert!(
            svg.contains(&format!(r#"rx="{}""#, badge.rect.height / 2.0)),
            "{svg}"
        );
        assert!(!svg.contains('×'));
    }

    #[test]
    fn a_zone_with_a_firewall_shows_one_where_each_arrow_comes_in() {
        let mut e =
            Engine::<Physical>::new_diagram(PhysicalColors::default()).with_measure(&ESTIMATE);
        let zone = |e: &Engine<Physical>, label: &str| {
            e.elements()
                .iter()
                .find(|z| z.label == label)
                .unwrap()
                .id
                .clone()
        };
        let (end_user, business) = (zone(&e, "End User"), zone(&e, "Business"));
        let add = |e: &mut Engine<Physical>, zone: &ElementId, kind| {
            e.select(zone);
            e.add_at_free_position(kind);
            e.selection[0].clone()
        };
        let user = add(&mut e, &end_user, PhysicalKind::User);
        let server = add(&mut e, &business, PhysicalKind::Server);
        let other = add(&mut e, &business, PhysicalKind::Server);
        let incoming = e.add_connection(&user, &server).unwrap();
        e.add_connection(&server, &other).unwrap();
        let outgoing = e.add_connection(&other, &user).unwrap();
        let firewalls = |e: &Engine<Physical>| {
            let drawn = drawn_arrows(e, e.layout());
            (edge_firewalls(e, e.layout(), &drawn), drawn)
        };
        // Without the flag there are none.
        assert!(firewalls(&e).0.is_empty());
        let plain = scene_to_svg("Infra", &scene(&e));

        e.replace(&business, |mut z| {
            z.props.firewall = true;
            z
        });
        // Only the arrow from outside gets one, not the one inside or the one going out.
        let (walls, drawn) = firewalls(&e);
        assert_eq!(walls.len(), 1);
        assert!(walls[0].id.to_string().starts_with(&incoming.to_string()));
        // Centred on the zone's edge, on the arrow's line.
        let rect = e.laid(&business).unwrap().rect;
        let arrow = drawn
            .iter()
            .find(|a| a.id == incoming)
            .unwrap()
            .look
            .clone();
        let at = walls[0].rect.center();
        assert!((at.y - rect.y).abs() < 1e-6, "{at:?} on {rect:?}");
        let cross = (arrow.to.x - arrow.from.x) * (at.y - arrow.from.y)
            - (arrow.to.y - arrow.from.y) * (at.x - arrow.from.x);
        assert!(cross.abs() < 1e-6);
        // Drawn in the SVG and the draw.io file.
        assert!(scene_to_svg("Infra", &scene(&e)).len() > plain.len());
        // Two bricks tall, rather than a firewall element's four, in draw.io too.
        assert_eq!(
            walls[0].look.shape,
            Shape::Physical(PhysicalFigure::LowFirewall)
        );
        assert_eq!(walls[0].rect.height, 21.0);
        let xml = to_drawio(&e, "Infra", None);
        assert!(xml.contains(&walls[0].id.to_string()));
        let cell = xml
            .lines()
            .skip_while(|l| !l.contains(&walls[0].id.to_string()))
            .nth(1)
            .unwrap();
        assert!(cell.contains(r#"width="58" height="21""#), "{cell}");

        // The arrow going out, drawn both ways, now comes in too.
        let props = e.connection(&outgoing).unwrap().props.clone();
        let back = e.add_connection(&user, &other).unwrap();
        e.set_connection_props(&back, props);
        assert_eq!(firewalls(&e).0.len(), 2);

        // A line both ways between two such zones that touch crosses both at one point, so it
        // gets one firewall.
        let perimeter = zone(&e, "Perimeter");
        e.replace(&perimeter, |mut z| {
            z.props.firewall = true;
            z
        });
        let front = add(&mut e, &perimeter, PhysicalKind::Server);
        e.add_connection(&front, &server).unwrap();
        e.add_connection(&server, &front).unwrap();
        let (walls, drawn) = firewalls(&e);
        assert_eq!(walls.len(), 3);
        // No protocol and port is hidden under a firewall.
        for tag in drawn.iter().filter_map(|a| a.look.tag.as_ref()) {
            assert!(walls.iter().all(|w| !w.rect.overlaps(&tag.rect)), "{tag:?}");
        }
    }
}
