//! The flowchart's rules: what a new diagram starts with, where arrows can go, the labels a
//! decision's arrows start with, how connectors are named, how pools and their lanes behave
//! (see `lanes`), and how elements are checked and read to screen readers.

use std::collections::HashMap;

use hiero_core::engine::{
    Conn, Created, DiagramType, Dir, El, Engine, Laid, Placement, ResizeLimits, validate_label,
    validate_length,
};
use hiero_core::file_check::{Field, FieldRule};
use hiero_core::geometry::{Point, Rect};
use hiero_core::layout::snap_to_canvas;
use hiero_core::model::{DiagramTypeId, Element, ElementId, KindInfo, NoColors};

use crate::lanes::{
    constrain_lane, fill_pool, index_at, keep_inside, lane_step, lanes_in, move_in_pool,
    new_lane_rect, pool_of, starting_lanes, step_out_of_lane, turned,
};
use crate::model::{
    CONNECTOR_NAME_MAX, Flowchart, FlowchartArrow, FlowchartKind, FlowchartProps, LABEL_MAX,
    Orientation, PALETTE, TERMINAL_SIZE,
};

type FEngine = Engine<Flowchart>;
type FLaid = Laid<Flowchart>;

const ORIENTATION_FIELD: &[Field] = &[Field {
    key: "orientation",
    rule: FieldRule::OneOf {
        values: &["vertical"],
        default: Some("horizontal"),
    },
}];

const ARROW_FIELDS: &[Field] = &[Field {
    key: "label",
    rule: FieldRule::Text {
        name: "Label",
        max: LABEL_MAX,
    },
}];

/// Where a new diagram's Start and End go: a grid square and more from the corner, side by
/// side with room for a step between them.
const START_AT: (f64, f64) = (80.0, 80.0);
const END_AT: (f64, f64) = (480.0, 80.0);

/// The labels a decision's first arrows out start with.
const DECISION_LABELS: [&str; 2] = ["Yes", "No"];

/// An arrow as stored: its label trimmed, and left out when blank.
pub fn normalized_arrow(arrow: FlowchartArrow) -> FlowchartArrow {
    FlowchartArrow {
        label: arrow
            .label
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty()),
    }
}

/// Such as "To Ship order: Yes", for the list of an element's arrows.
pub fn arrow_title(arrow: &FlowchartArrow, outgoing: bool, other: &str) -> String {
    let mut out = format!("{} {other}", if outgoing { "To" } else { "From" });
    if let Some(label) = &arrow.label {
        out.push_str(&format!(": {label}"));
    }
    out
}

/// The name a new connector takes: the name of the first connector without its other end
/// yet, so the second of a pair matches the first, or else the first letter no connector has.
pub fn next_connector_name(elements: &[El<Flowchart>]) -> String {
    let names: Vec<&str> = elements
        .iter()
        .filter(|e| e.kind == FlowchartKind::Connector)
        .map(|e| e.label.as_str())
        .collect();
    if let Some(alone) = names
        .iter()
        .find(|n| names.iter().filter(|m| m == n).count() == 1)
    {
        return (*alone).to_owned();
    }
    ('A'..='Z')
        .map(String::from)
        .find(|letter| !names.contains(&letter.as_str()))
        .unwrap_or_else(|| "A".into())
}

fn new_element(eng: &mut Engine<Flowchart>, kind: FlowchartKind, at: (f64, f64)) -> El<Flowchart> {
    Element {
        id: ElementId(eng.new_id()),
        kind,
        label: Flowchart::info(kind).entry_label().to_owned(),
        description: None,
        parent_id: None,
        sub_diagram_id: None,
        props: FlowchartProps::default(),
        width: None,
        height: None,
        x: at.0,
        y: at.1,
    }
}

/// An element of `kind` named `label` in `parent`, at `rect`, its size kept.
fn placed(
    eng: &mut FEngine,
    kind: FlowchartKind,
    label: &str,
    parent: Option<ElementId>,
    rect: Rect,
) -> El<Flowchart> {
    Element {
        label: label.to_owned(),
        parent_id: parent,
        width: Some(rect.width),
        height: Some(rect.height),
        ..new_element(eng, kind, (rect.x, rect.y))
    }
}

/// A pool at `at`, with its two lanes.
fn new_pool(eng: &mut FEngine, at: Point) -> Created<Flowchart> {
    let size = Flowchart::info(FlowchartKind::Pool).size();
    let at = snap_to_canvas(at, size, eng.canvas());
    let pool = new_element(eng, FlowchartKind::Pool, (at.x, at.y));
    let mut added = vec![pool.clone()];
    for (label, rect) in starting_lanes(at) {
        added.push(placed(
            eng,
            FlowchartKind::Lane,
            label,
            Some(pool.id.clone()),
            rect,
        ));
    }
    Created {
        added,
        existing: None,
    }
}

/// A lane after the last of `pool`'s, named after its place, such as "Lane 3".
fn new_lane(eng: &mut FEngine, pool: &FLaid) -> Created<Flowchart> {
    let rect = new_lane_rect(eng, pool);
    let label = format!("Lane {}", lanes_in(eng, pool.id()).len() + 1);
    let lane = placed(
        eng,
        FlowchartKind::Lane,
        &label,
        Some(pool.id().clone()),
        rect,
    );
    Created {
        added: vec![lane],
        existing: None,
    }
}

/// Removes a lane, giving its steps to the lane before it (or after it), where they keep
/// their place in the lane. Removing a pool's last lane removes the pool, and its steps stay.
fn remove_lane(eng: &mut FEngine, lane: &FLaid) {
    let Some(pool) = pool_of(eng, lane).cloned() else {
        eng.base_remove(lane.id());
        return;
    };
    let lanes: Vec<FLaid> = lanes_in(eng, pool.id()).into_iter().cloned().collect();
    let index = lanes.iter().position(|l| l.id() == lane.id()).unwrap_or(0);
    let heir = if index > 0 {
        lanes.get(index - 1)
    } else {
        lanes.get(1)
    }
    .cloned();
    let Some(heir) = heir else {
        remove_pool(eng, &pool);
        return;
    };
    let (dx, dy) = (heir.rect.x - lane.rect.x, heir.rect.y - lane.rect.y);
    let vertical = crate::lanes::is_vertical(&pool.el);
    let elements = eng
        .elements()
        .iter()
        .filter(|e| e.id != *lane.id())
        .cloned()
        .map(|mut e| {
            if e.parent_id.as_ref() == Some(lane.id()) {
                e.parent_id = Some(heir.id().clone());
                // Along the pool the step moves into the heir; across it, it stays put.
                if vertical {
                    e.x += dx;
                } else {
                    e.y += dy;
                }
            }
            e
        })
        .collect();
    eng.set_elements(elements);
    eng.remove_connections_of(lane.id());
    eng.clear_selection();
    eng.announcement = format!(
        "{} removed; its steps moved to {}.",
        lane.el.label, heir.el.label
    );
}

/// Removes a pool and its lanes. Their steps stay on the diagram, free-form, with their
/// arrows.
fn remove_pool(eng: &mut FEngine, pool: &FLaid) {
    let lanes: Vec<ElementId> = eng
        .elements()
        .iter()
        .filter(|e| e.parent_id.as_ref() == Some(pool.id()))
        .map(|e| e.id.clone())
        .collect();
    let gone = |id: &ElementId| id == pool.id() || lanes.contains(id);
    let elements = eng
        .elements()
        .iter()
        .filter(|e| !gone(&e.id))
        .cloned()
        .map(|e| {
            if e.parent_id.as_ref().is_some_and(gone) {
                e.with_parent(None)
            } else {
                e
            }
        })
        .collect();
    eng.set_elements(elements);
    eng.remove_connections_of(pool.id());
    eng.clear_selection();
    eng.announcement = format!("{} and its lanes removed.", pool.el.label);
}

/// Such as "arrow to Ship order, labelled Yes", for screen readers.
fn arrow_details(eng: &Engine<Flowchart>, element: &Laid<Flowchart>) -> Vec<String> {
    let label = |id: &ElementId| eng.label_of(id).unwrap_or_default().to_owned();
    let labelled = |c: &Conn<Flowchart>| {
        c.props
            .label
            .as_ref()
            .map(|l| format!(", labelled {l}"))
            .unwrap_or_default()
    };
    eng.connections()
        .iter()
        .filter_map(|c| {
            if c.source_id == *element.id() {
                Some(format!("arrow to {}{}", label(&c.target_id), labelled(c)))
            } else if c.target_id == *element.id() {
                Some(format!("arrow from {}{}", label(&c.source_id), labelled(c)))
            } else {
                None
            }
        })
        .collect()
}

impl DiagramType for Flowchart {
    const ID: DiagramTypeId = DiagramTypeId::Flowchart;
    type Kind = FlowchartKind;
    type Props = FlowchartProps;
    type Arrow = FlowchartArrow;
    type Colors = NoColors;

    const ARROW_FIELDS: &'static [Field] = ARROW_FIELDS;

    fn normalized_arrow(arrow: FlowchartArrow) -> FlowchartArrow {
        normalized_arrow(arrow)
    }

    fn palette() -> &'static [KindInfo<FlowchartKind>] {
        &PALETTE
    }

    /// A Start and an End, side by side, with nothing between them yet.
    fn starting_elements(eng: &mut Engine<Self>) -> Vec<El<Self>> {
        let start = new_element(eng, FlowchartKind::Start, START_AT);
        let end = new_element(eng, FlowchartKind::End, END_AT);
        debug_assert!(START_AT.0 + TERMINAL_SIZE.width < END_AT.0);
        vec![start, end]
    }

    fn file_fields(kind: FlowchartKind) -> &'static [Field] {
        if kind == FlowchartKind::Pool {
            ORIENTATION_FIELD
        } else {
            &[]
        }
    }

    /// A pool starts with two lanes. A lane goes after the last lane of the pool it's dropped
    /// on, or of the first pool; with no pool, a new pool is added with its two lanes. A
    /// connector is named to pair up with the last one, or with a new letter.
    fn created(
        eng: &mut Engine<Self>,
        kind: &'static KindInfo<FlowchartKind>,
        placement: Placement,
    ) -> Created<Self> {
        match kind.kind {
            FlowchartKind::Pool => new_pool(eng, placement.position),
            FlowchartKind::Lane => {
                let pool = placement
                    .parent_id
                    .as_ref()
                    .and_then(|p| eng.laid(p))
                    .filter(|p| p.el.kind == FlowchartKind::Pool)
                    .or_else(|| {
                        eng.layout()
                            .iter()
                            .find(|e| e.el.kind == FlowchartKind::Pool)
                    })
                    .cloned();
                match pool {
                    Some(pool) => new_lane(eng, &pool),
                    None => new_pool(eng, placement.position),
                }
            }
            _ => {
                let name = next_connector_name(eng.elements());
                let mut created = eng.base_created(kind, placement);
                if let Some(first) = created
                    .added
                    .first_mut()
                    .filter(|e| e.kind == FlowchartKind::Connector)
                {
                    first.label = name;
                }
                created
            }
        }
    }

    /// An element in a container its kind can't be in, such as a step saved in a pool rather
    /// than a lane, is shown on its own. Lanes fill their pools.
    fn arrange(_eng: &FEngine, elements: Vec<El<Self>>) -> Vec<El<Self>> {
        let kinds: HashMap<ElementId, FlowchartKind> =
            elements.iter().map(|e| (e.id.clone(), e.kind)).collect();
        let held: Vec<El<Self>> = elements
            .into_iter()
            .map(|e| {
                let can_hold = e
                    .parent_id
                    .as_ref()
                    .and_then(|p| kinds.get(p))
                    .is_none_or(|parent| Self::info(*parent).can_contain.contains(&e.kind));
                if can_hold { e } else { e.with_parent(None) }
            })
            .collect();
        let pools: Vec<ElementId> = held
            .iter()
            .filter(|e| e.kind == FlowchartKind::Pool)
            .map(|e| e.id.clone())
            .collect();
        pools.iter().fold(held, fill_pool)
    }

    /// Resizing a pool shares its new size among its lanes, in proportion; resizing a lane
    /// resizes its pool to match.
    fn resized(
        eng: &FEngine,
        elements: Vec<El<Self>>,
        id: &ElementId,
        rect: Rect,
    ) -> Vec<El<Self>> {
        match crate::lanes::resized(eng, &elements, id, rect) {
            Some(resized) => resized,
            None => eng.base_resized(elements, id, rect),
        }
    }

    /// A pool can't shrink past what its lanes need to hold their steps.
    fn resize_limits(eng: &FEngine, element: &FLaid) -> ResizeLimits {
        (element.el.kind == FlowchartKind::Pool)
            .then(|| crate::lanes::resize_limits(eng, element))
            .flatten()
            .unwrap_or_else(|| eng.base_resize_limits(element))
    }

    /// A lane stays in its pool, moving only along it. A step in a lane stays within it.
    fn constrain_move(eng: &FEngine, element: &FLaid, placement: Placement) -> Placement {
        if let Some(pool) = pool_of(eng, element) {
            return constrain_lane(pool, element, &placement);
        }
        match placement.parent_id.as_ref().and_then(|p| eng.laid(p)) {
            Some(parent) => Placement {
                position: keep_inside(placement.position, element.rect.size(), parent),
                ..placement
            },
            None => placement,
        }
    }

    /// A lane dragged in its pool takes the place where its middle lands.
    fn commit_move(
        eng: &mut FEngine,
        id: &ElementId,
        placement: Placement,
        with: Option<El<Self>>,
    ) {
        let lane = eng.laid(id).cloned();
        let pool = lane.as_ref().and_then(|l| pool_of(eng, l)).cloned();
        match (lane, pool) {
            (Some(lane), Some(pool))
                if placement.parent_id.as_ref() == Some(pool.id()) && with.is_none() =>
            {
                let index = index_at(eng, &pool, &lane, placement.position);
                move_in_pool(eng, id, pool.id(), index);
            }
            _ => eng.base_commit_move(id, placement, with),
        }
    }

    /// The arrow keys along a pool swap a lane with its neighbour; the others leave it. A step
    /// against its lane's edge steps across it. Groups move as usual.
    fn element_key(eng: &mut FEngine, id: &ElementId, dir: Dir, _shift: bool) -> bool {
        if eng.in_group(id) {
            return false;
        }
        let Some(element) = eng.laid(id).cloned() else {
            return false;
        };
        let Some(pool) = pool_of(eng, &element).cloned() else {
            return step_out_of_lane(eng, &element, dir);
        };
        let lanes = lanes_in(eng, pool.id());
        let Some(position) = lanes.iter().position(|e| e.id() == id) else {
            return true;
        };
        let step = lane_step(&pool, dir);
        let index = position as i64 + step;
        if step != 0 && index >= 0 && (index as usize) < lanes.len() {
            move_in_pool(eng, id, pool.id(), index as usize);
        }
        true
    }

    /// A removed lane's steps go to the lane before it, or the one after; removing a pool's
    /// last lane removes the pool. A removed pool's steps stay on the diagram, free-form.
    fn remove(eng: &mut FEngine, id: &ElementId) {
        let Some(element) = eng.laid(id).cloned() else {
            return;
        };
        match element.el.kind {
            FlowchartKind::Lane => remove_lane(eng, &element),
            FlowchartKind::Pool => remove_pool(eng, &element),
            _ => eng.base_remove(id),
        }
    }

    /// A step dropped in a lane sits within it.
    fn drop_position(
        eng: &FEngine,
        kind: &'static KindInfo<FlowchartKind>,
        position: Point,
        parent: &FLaid,
    ) -> Point {
        let size = Self::new_size_of(eng, kind);
        keep_inside(
            snap_to_canvas(keep_inside(position, size, parent), size, eng.canvas()),
            size,
            parent,
        )
    }

    /// Turning a pool's lanes the other way keeps each step's place along the flow.
    fn apply_element(
        eng: &mut FEngine,
        id: &ElementId,
        with: El<Self>,
        parent_id: Option<ElementId>,
    ) {
        let turn = eng
            .element(id)
            .is_some_and(|e| e.kind == FlowchartKind::Pool && e.props != with.props);
        let label = with.label.clone();
        let orientation = with.props.orientation;
        if !turn {
            eng.apply_changes(id, with, parent_id);
            return;
        }
        // Turned as it was laid out, then given its new orientation.
        let elements = turned(eng, id)
            .into_iter()
            .map(|mut e| {
                if e.id == *id {
                    e.label.clone_from(&label);
                    e.props.orientation = orientation;
                }
                e
            })
            .collect();
        eng.set_elements(elements);
        eng.announcement = format!(
            "{label}'s lanes now run {}.",
            Orientation::label(orientation).to_lowercase()
        );
    }

    /// Such as "Pool (vertical)".
    fn kind_description(_eng: &FEngine, element: &FLaid) -> String {
        match element.el.props.orientation {
            Some(o) if element.el.kind == FlowchartKind::Pool => {
                format!("Pool ({})", Orientation::label(Some(o)).to_lowercase())
            }
            _ => element.info.label.to_owned(),
        }
    }

    /// Nothing leads into a start or out of an end, and a note is about one element only.
    fn can_connect(eng: &Engine<Self>, source: &Laid<Self>, target: &Laid<Self>) -> bool {
        let note_taken = source.el.kind == FlowchartKind::Note
            && eng
                .connections()
                .iter()
                .any(|c| c.source_id == *source.id() && c.target_id != *target.id());
        source.id() != target.id()
            && source.info.connects_to.contains(&target.el.kind)
            && !note_taken
    }

    /// A decision's first two arrows out of it are labelled Yes and No.
    fn new_arrow(eng: &Engine<Self>, source: &ElementId, target: &ElementId) -> FlowchartArrow {
        let is =
            |id: &ElementId, kind: FlowchartKind| eng.element(id).is_some_and(|e| e.kind == kind);
        if !is(source, FlowchartKind::Decision)
            || eng.element(target).is_some_and(|e| e.kind.is_data())
        {
            return FlowchartArrow::default();
        }
        let out = eng
            .connections()
            .iter()
            .filter(|c| c.source_id == *source)
            .filter(|c| eng.element(&c.target_id).is_some_and(|e| !e.kind.is_data()))
            .count();
        FlowchartArrow {
            label: DECISION_LABELS.get(out).map(|l| (*l).to_owned()),
        }
    }

    fn extra_details(eng: &Engine<Self>, element: &Laid<Self>) -> Vec<String> {
        let mut details = arrow_details(eng, element);
        if element.el.kind == FlowchartKind::Pool {
            let lanes: Vec<String> = lanes_in(eng, element.id())
                .iter()
                .map(|l| l.el.label.clone())
                .collect();
            if !lanes.is_empty() {
                details.push(format!("lanes {}", hiero_core::engine::list_labels(&lanes)));
            }
        }
        details
    }

    fn validate_element(element: &El<Self>) -> Result<(), String> {
        validate_label(&element.label)?;
        if element.kind == FlowchartKind::Connector {
            validate_length("A connector's name", &element.label, CONNECTOR_NAME_MAX)?;
        }
        Ok(())
    }

    fn validate_arrow(arrow: &FlowchartArrow) -> Result<(), String> {
        validate_length(
            "Label",
            arrow.label.as_deref().unwrap_or_default(),
            LABEL_MAX,
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use hiero_core::geometry::Size;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    pub(crate) fn engine() -> Engine<Flowchart> {
        let mut n = 0;
        Engine::new(Vec::new(), Vec::new(), NoColors {})
            .with_measure(&ESTIMATE)
            .with_ids(move || {
                n += 1;
                format!("id{n}")
            })
    }

    fn id(s: &str) -> ElementId {
        s.into()
    }

    #[test]
    fn a_new_diagram_starts_with_a_start_and_an_end() {
        let e = Engine::<Flowchart>::new_diagram(NoColors {});
        let kinds: Vec<(FlowchartKind, &str)> = e
            .elements()
            .iter()
            .map(|el| (el.kind, el.label.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [(FlowchartKind::Start, "Start"), (FlowchartKind::End, "End")]
        );
        assert!(e.connections().is_empty());
    }

    #[test]
    fn nothing_leads_into_a_start_or_out_of_an_end() {
        let mut e = engine();
        e.add_at_free_position(FlowchartKind::Start);
        e.add_at_free_position(FlowchartKind::Process);
        e.add_at_free_position(FlowchartKind::End);
        let can = |e: &Engine<Flowchart>, a: &str, b: &str| e.can_connect(&id(a), &id(b));
        assert!(can(&e, "id1", "id2"));
        assert!(can(&e, "id2", "id3"));
        assert!(!can(&e, "id2", "id1"));
        assert!(!can(&e, "id3", "id2"));
        assert!(!can(&e, "id2", "id2"));
    }

    #[test]
    fn a_note_is_about_one_element() {
        let mut e = engine();
        e.add_at_free_position(FlowchartKind::Note);
        e.add_at_free_position(FlowchartKind::Process);
        e.add_at_free_position(FlowchartKind::Decision);
        assert!(e.can_connect(&id("id1"), &id("id2")));
        e.add_connection(&id("id1"), &id("id2")).unwrap();
        assert!(!e.can_connect(&id("id1"), &id("id3")));
        // Nothing points to a note.
        assert!(!e.can_connect(&id("id2"), &id("id1")));
    }

    #[test]
    fn a_decisions_first_arrows_are_labelled_yes_and_no() {
        let mut e = engine();
        e.add_at_free_position(FlowchartKind::Decision);
        e.add_at_free_position(FlowchartKind::DataStore);
        for _ in 0..3 {
            e.add_at_free_position(FlowchartKind::Process);
        }
        let label = |e: &Engine<Flowchart>, c| {
            e.connections()
                .iter()
                .find(|x| x.id == c)
                .unwrap()
                .props
                .label
                .clone()
        };
        // An arrow to data isn't one of the ways out.
        let data = e.add_connection(&id("id1"), &id("id2")).unwrap();
        let yes = e.add_connection(&id("id1"), &id("id3")).unwrap();
        let no = e.add_connection(&id("id1"), &id("id4")).unwrap();
        let third = e.add_connection(&id("id1"), &id("id5")).unwrap();
        assert_eq!(label(&e, data), None);
        assert_eq!(label(&e, yes).as_deref(), Some("Yes"));
        assert_eq!(label(&e, no).as_deref(), Some("No"));
        assert_eq!(label(&e, third), None);
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert!(
            e.accessible_label(&laid)
                .contains("arrow to Process, labelled Yes")
        );
    }

    #[test]
    fn connectors_are_named_in_pairs() {
        let mut e = engine();
        let names = |e: &Engine<Flowchart>| -> Vec<String> {
            e.elements().iter().map(|x| x.label.clone()).collect()
        };
        for _ in 0..3 {
            e.add_at_free_position(FlowchartKind::Connector);
        }
        assert_eq!(names(&e), ["A", "A", "B"]);
        let mut long = e.element(&id("id1")).unwrap().clone();
        long.label = "ABCD".into();
        assert!(Flowchart::validate_element(&long).is_err());
        long.kind = FlowchartKind::Process;
        assert!(Flowchart::validate_element(&long).is_ok());
    }

    #[test]
    fn arrows_are_trimmed_and_titled() {
        let arrow = normalized_arrow(FlowchartArrow {
            label: Some(" Yes ".into()),
        });
        assert_eq!(arrow.label.as_deref(), Some("Yes"));
        assert_eq!(arrow_title(&arrow, true, "Ship"), "To Ship: Yes");
        assert_eq!(
            normalized_arrow(FlowchartArrow {
                label: Some("  ".into())
            }),
            FlowchartArrow::default()
        );
        assert!(
            Flowchart::validate_arrow(&FlowchartArrow {
                label: Some("x".repeat(41))
            })
            .is_err()
        );
    }

    /// A pool added at (40, 400), with its two lanes; returns the pool's and lanes' ids.
    fn pool(e: &mut Engine<Flowchart>) -> (String, String, String) {
        let before = e.elements().len();
        e.palette_drop(FlowchartKind::Pool, Point::new(640.0, 620.0));
        let ids: Vec<String> = e.elements()[before..]
            .iter()
            .map(|x| x.id.0.clone())
            .collect();
        (ids[0].clone(), ids[1].clone(), ids[2].clone())
    }

    fn rect_of(e: &Engine<Flowchart>, of: &str) -> Rect {
        e.laid(&id(of)).unwrap().rect
    }

    #[test]
    fn a_pool_starts_with_two_lanes_that_fill_it() {
        let mut e = engine();
        let (p, l1, l2) = pool(&mut e);
        let pr = rect_of(&e, &p);
        assert_eq!(pr.size(), crate::model::POOL_SIZE);
        let (a, b) = (rect_of(&e, &l1), rect_of(&e, &l2));
        assert_eq!(e.element(&id(&l1)).unwrap().label, "Lane 1");
        assert_eq!(a, Rect::new(pr.x, pr.y + 40.0, 1200.0, 200.0));
        assert_eq!(b, Rect::new(pr.x, pr.y + 240.0, 1200.0, 200.0));
        // Lanes take no arrows.
        e.add_at_free_position(FlowchartKind::Process);
        assert!(!e.can_connect(&id(&l1), &id("id4")));
        assert!(!e.can_connect(&id("id4"), &id(&l1)));
        let laid = e.laid(&id(&p)).unwrap().clone();
        assert!(
            e.accessible_label(&laid)
                .contains("lanes Lane 1 and Lane 2")
        );
    }

    #[test]
    fn steps_dropped_in_a_lane_stay_in_it() {
        let mut e = engine();
        let (_, _, l2) = pool(&mut e);
        let lane = rect_of(&e, &l2);
        e.palette_drop(FlowchartKind::Process, lane.center());
        let step = e.elements().last().unwrap().clone();
        assert_eq!(step.parent_id, Some(id(&l2)));
        // Dropped on the lane's bottom edge, it's kept inside it.
        e.palette_drop(
            FlowchartKind::Decision,
            Point::new(lane.center().x, lane.bottom() - 4.0),
        );
        let edge = e.elements().last().unwrap().clone();
        assert_eq!(edge.parent_id, Some(id(&l2)));
        assert!(lane.contains_rect(&rect_of(&e, &edge.id.0)));
    }

    #[test]
    fn a_lane_goes_after_the_last_in_its_pool_or_starts_one() {
        let mut e = engine();
        // With no pool, a lane comes with a pool of its own.
        e.add_at_free_position(FlowchartKind::Lane);
        let kinds: Vec<FlowchartKind> = e.elements().iter().map(|x| x.kind).collect();
        assert_eq!(
            kinds,
            [
                FlowchartKind::Pool,
                FlowchartKind::Lane,
                FlowchartKind::Lane
            ]
        );
        e.add_at_free_position(FlowchartKind::Lane);
        let lane = e.elements().last().unwrap().clone();
        assert_eq!(lane.label, "Lane 3");
        assert_eq!(lane.parent_id, Some(id("id1")));
        let pr = rect_of(&e, "id1");
        assert_eq!(
            rect_of(&e, &lane.id.0),
            Rect::new(pr.x, pr.y + 440.0, 1200.0, 200.0)
        );
        assert_eq!(pr.height, 640.0);
    }

    #[test]
    fn resizing_a_pool_shares_it_among_its_lanes() {
        let mut e = engine();
        let (p, l1, l2) = pool(&mut e);
        let pr = rect_of(&e, &p);
        e.commit_resize(&id(&p), Rect::new(pr.x, pr.y, 1400.0, 840.0));
        assert_eq!(rect_of(&e, &l1).size(), Size::new(1400.0, 400.0));
        assert_eq!(rect_of(&e, &l2).size(), Size::new(1400.0, 400.0));
        // A lane made taller makes its pool taller.
        let lr = rect_of(&e, &l1);
        e.commit_resize(&id(&l1), Rect::new(lr.x, lr.y, lr.width, 600.0));
        assert_eq!(rect_of(&e, &p).height, 1040.0);
        assert_eq!(rect_of(&e, &l2).y, lr.y + 600.0);
    }

    #[test]
    fn lanes_reorder_with_the_arrow_keys_and_take_their_steps() {
        let mut e = engine();
        let (_, l1, l2) = pool(&mut e);
        e.palette_drop(FlowchartKind::Process, rect_of(&e, &l1).center());
        let step = e.elements().last().unwrap().id.0.clone();
        let before = rect_of(&e, &step);
        e.arrow_key(&id(&l1), Dir::Down, false);
        assert!(rect_of(&e, &l1).y > rect_of(&e, &l2).y);
        assert_eq!(rect_of(&e, &step).y, before.y + 200.0);
        assert_eq!(e.announcement, "Lane 1 moved to 2 of 2 in Pool.");
        // Across the pool, nothing happens.
        e.arrow_key(&id(&l1), Dir::Left, false);
        assert!(rect_of(&e, &l1).y > rect_of(&e, &l2).y);
    }

    #[test]
    fn a_removed_lanes_steps_go_to_its_neighbour() {
        let mut e = engine();
        let (p, l1, l2) = pool(&mut e);
        e.palette_drop(FlowchartKind::Process, rect_of(&e, &l1).center());
        let first = e.elements().last().unwrap().id.0.clone();
        e.palette_drop(FlowchartKind::Decision, rect_of(&e, &l2).center());
        let second = e.elements().last().unwrap().id.0.clone();
        let lane_x = rect_of(&e, &second).x;
        Flowchart::remove(&mut e, &id(&l2));
        assert_eq!(e.element(&id(&second)).unwrap().parent_id, Some(id(&l1)));
        assert_eq!(rect_of(&e, &second).x, lane_x);
        assert!(rect_of(&e, &l1).contains_rect(&rect_of(&e, &second)));
        assert_eq!(e.announcement, "Lane 2 removed; its steps moved to Lane 1.");
        // The last lane takes its pool with it; the steps stay, free-form.
        Flowchart::remove(&mut e, &id(&l1));
        assert!(e.element(&id(&p)).is_none());
        assert_eq!(e.element(&id(&first)).unwrap().parent_id, None);
        assert_eq!(e.element(&id(&second)).unwrap().parent_id, None);
    }

    #[test]
    fn turning_a_pool_keeps_each_steps_place_along_the_flow() {
        let mut e = engine();
        let (p, l1, l2) = pool(&mut e);
        let a = rect_of(&e, &l1);
        e.palette_drop(FlowchartKind::Process, Point::new(a.x + 500.0, a.y + 100.0));
        let step = e.elements().last().unwrap().id.0.clone();
        let s = rect_of(&e, &step);
        let (along, across) = (s.x - (a.x + 20.0), s.y - (a.y + 40.0));
        let mut with = e.element(&id(&p)).unwrap().clone();
        with.props.orientation = Some(Orientation::Vertical);
        Flowchart::apply_element(&mut e, &id(&p), with, None);
        assert_eq!(e.announcement, "Pool's lanes now run vertical.");
        let (a, b) = (rect_of(&e, &l1), rect_of(&e, &l2));
        // Side by side now, the first on the left.
        assert_eq!(a.y, b.y);
        assert_eq!(b.x, a.right());
        assert!(a.height > a.width);
        // What was along the lane is now down it.
        let s = rect_of(&e, &step);
        assert_eq!(s.y - (a.y + 40.0), along);
        assert_eq!(s.x - (a.x + 20.0), across);
        assert_eq!(e.element(&id(&step)).unwrap().parent_id, Some(id(&l1)));
        let pool_rect = rect_of(&e, &p);
        assert!(pool_rect.contains_rect(&b));
        // A lane added now goes on the right, and lanes reorder left and right.
        e.add_at_free_position(FlowchartKind::Lane);
        let third = e.elements().last().unwrap().id.0.clone();
        assert_eq!(rect_of(&e, &third).x, rect_of(&e, &l2).right());
        e.arrow_key(&id(&l1), Dir::Right, false);
        assert!(rect_of(&e, &l1).x > rect_of(&e, &l2).x);
        // And back: rows again, the step where it started.
        let mut with = e.element(&id(&p)).unwrap().clone();
        with.props.orientation = None;
        Flowchart::apply_element(&mut e, &id(&p), with, None);
        let (a, b) = (rect_of(&e, &l1), rect_of(&e, &l2));
        assert_eq!(a.x, b.x);
        let s = rect_of(&e, &step);
        assert_eq!(s.x - (a.x + 20.0), along);
        assert_eq!(s.y - (a.y + 40.0), across);
    }
}
