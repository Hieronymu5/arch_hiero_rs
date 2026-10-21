//! Running conceptual arrows straight: sliding them sideways by hand, straightening them, and
//! returning them to automatic placement.

use hiero_core::engine::{Dir, Engine};
use hiero_core::geometry::{GRID, Point, js_round};
use hiero_core::model::ConnectionId;
use hiero_core::routing::{StraightRange, straight_range};

use crate::model::Conceptual;
use crate::scene::drawn_arrows;

/// Straight arrows snap to half a grid step.
pub const ARROW_STEP: f64 = GRID / 2.0;

/// Where a straight arrow can run between the arrow's elements, if anywhere.
pub fn range_for(eng: &Engine<Conceptual>, id: &ConnectionId) -> Option<StraightRange> {
    let connection = eng.connection(id)?;
    let source = eng.laid(&connection.source_id)?;
    let target = eng.laid(&connection.target_id)?;
    straight_range(&source.rect, &target.rect)
}

/// A position along the sides, kept inside the range, as an offset from the source.
pub fn offset_within(range: StraightRange, at: f64) -> f64 {
    at.max(range.start).min(range.end) - range.source_start
}

/// Sets or clears an arrow's hand-placed offset, saying what changed.
pub fn set_manual_offset(
    eng: &mut Engine<Conceptual>,
    id: &ConnectionId,
    offset: Option<f64>,
    change: &str,
) {
    let Some(connection) = eng.connection(id).cloned() else {
        return;
    };
    if connection.props.manual_offset == offset {
        return;
    }
    let mut props = connection.props.clone();
    props.manual_offset = offset;
    eng.set_connection_props(id, props);
    eng.announcement = format!(
        "{} {change}.",
        eng.arrow_name(&connection.source_id, &connection.target_id)
    );
}

/// Runs the arrow straight across the middle of where its elements' sides overlap.
pub fn straighten(eng: &mut Engine<Conceptual>, id: &ConnectionId) {
    if let Some(range) = range_for(eng, id) {
        let middle = js_round(f64::midpoint(range.start, range.end) / ARROW_STEP) * ARROW_STEP;
        set_manual_offset(eng, id, Some(offset_within(range, middle)), "straightened");
    }
}

/// Returns the arrow to automatic, evenly spaced placement.
pub fn place_automatically(eng: &mut Engine<Conceptual>, id: &ConnectionId) {
    set_manual_offset(eng, id, None, "placed automatically");
}

/// The offset an arrow being dragged to `pointer` takes, snapped to half a grid step.
pub fn offset_at(eng: &Engine<Conceptual>, id: &ConnectionId, pointer: Point) -> Option<f64> {
    let range = range_for(eng, id)?;
    let along = if range.side.is_vertical() {
        pointer.y
    } else {
        pointer.x
    };
    let at = js_round(along / ARROW_STEP) * ARROW_STEP;
    Some(offset_within(range, at))
}

/// The arrow keys slide a selected arrow along the sides it joins: up and down for arrows
/// between left and right sides, else left and right. Returns whether the key was used.
pub fn arrow_key(eng: &mut Engine<Conceptual>, dir: Dir, shift: bool) -> bool {
    let Some(id) = eng.selected_connection.clone() else {
        return false;
    };
    let drawn = drawn_arrows(eng, eng.layout(), None);
    let Some(arrow) = drawn.into_iter().find(|a| a.id == id) else {
        return false;
    };
    if arrow.across_y == dir.is_across() {
        return false;
    }
    let Some(range) = range_for(eng, &id) else {
        return true;
    };
    let delta = f64::from(dir.sign()) * ARROW_STEP * if shift { 5.0 } else { 1.0 };
    let current = if arrow.across_y {
        arrow.look.from.y
    } else {
        arrow.look.from.x
    };
    set_manual_offset(
        eng,
        &id,
        Some(offset_within(range, current + delta)),
        "moved",
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ConceptualKind;
    use hiero_core::look::DiagramColors;
    use hiero_core::model::ElementId;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    #[test]
    fn straightening_and_nudging() {
        let mut n = 0;
        let mut e = Engine::<Conceptual>::new(Vec::new(), Vec::new(), DiagramColors::default())
            .with_measure(&ESTIMATE)
            .with_ids(move || {
                n += 1;
                format!("id{n}")
            });
        e.add_at_free_position(ConceptualKind::Application);
        e.add_at_free_position(ConceptualKind::Application);
        let (a, b): (ElementId, ElementId) = ("id1".into(), "id2".into());
        let cid = e.add_connection(&a, &b).unwrap();
        straighten(&mut e, &cid);
        // Both are 100 tall at y=20: overlap 30..110, middle 70, offset 50 from the top.
        assert_eq!(e.connection(&cid).unwrap().props.manual_offset, Some(50.0));
        e.select_connection(&cid);
        assert!(arrow_key(&mut e, Dir::Down, false));
        assert_eq!(e.connection(&cid).unwrap().props.manual_offset, Some(60.0));
        // Left and right don't apply to an arrow between left and right sides.
        assert!(!arrow_key(&mut e, Dir::Left, false));
        place_automatically(&mut e, &cid);
        assert_eq!(e.connection(&cid).unwrap().props.manual_offset, None);
        assert_eq!(
            e.announcement,
            "Arrow from Application to Application placed automatically."
        );
    }
}
