//! The high level conceptual type's rules: how big a group's header is for its font size,
//! what arrows can join, what screen readers hear, and how elements are checked.

use hiero_core::engine::{
    DiagramType, El, Engine, Laid, descendant_ids, list_labels, validate_label, validate_length,
};
use hiero_core::file_check::{DescriptionRule, Field, FieldRule};
use hiero_core::model::{DiagramTypeId, ElementId, KindInfo, NoColors};

use crate::model::{
    BlockArrow, DESCRIPTION_MAX, FONT_SIZE_MAX, FONT_SIZE_MIN, HighLevel, HighLevelKind,
    HighLevelProps, PALETTE,
};

/// The space above and below a group's name in its header.
pub const NAME_PADDING: f64 = 8.0;
/// The shortest a group's header is: two grid squares.
pub const HEADER_MIN: f64 = 40.0;
const GRID: f64 = 20.0;

/// The height of a line of text in `size`.
pub fn line_height(size: f64) -> f64 {
    (size * 1.25).round()
}

/// The height of a group's header for a name in `size`: a line with space above and below it,
/// rounded up to the grid so what the group holds stays on it.
pub fn header_for(size: f64) -> f64 {
    let needed = line_height(size) + NAME_PADDING * 2.0;
    ((needed / GRID).ceil() * GRID).max(HEADER_MIN)
}

const APPLICATION_COLOR: Field = Field {
    key: "color",
    rule: FieldRule::OneOf {
        values: &["green", "red", "yellow", "grey"],
        default: Some("blue"),
    },
};

const GROUP_COLOR: Field = Field {
    key: "color",
    rule: FieldRule::OneOf {
        values: &["blue", "green", "red", "yellow"],
        default: Some("grey"),
    },
};

const FONT_SIZE: Field = Field {
    key: "fontSize",
    rule: FieldRule::Number,
};

const DASHED: Field = Field {
    key: "dashed",
    rule: FieldRule::True,
};

const APPLICATION_FIELDS: &[Field] = &[APPLICATION_COLOR, FONT_SIZE, DASHED];
/// Actors are blue unless set otherwise, as applications are, and have no border to dash.
const ACTORS_FIELDS: &[Field] = &[APPLICATION_COLOR, FONT_SIZE];
const GROUP_FIELDS: &[Field] = &[GROUP_COLOR, FONT_SIZE, DASHED];
const ARROW_FIELDS: &[Field] = &[Field {
    key: "twoWay",
    rule: FieldRule::True,
}];

/// The properties with each value left out at its kind's default, so every look has one
/// spelling.
pub fn kept(kind: HighLevelKind, props: HighLevelProps) -> HighLevelProps {
    HighLevelProps {
        color: props.color.filter(|c| *c != kind.default_color()),
        font_size: props.font_size.filter(|s| *s != kind.default_font_size()),
        dashed: props.dashed && kind != HighLevelKind::Actors,
    }
}

/// Such as "Group (green, dashed, 20 px)": the kind, and how it differs from the usual look.
fn kind_description(element: &Laid<HighLevel>) -> String {
    let kind = element.el.kind;
    let props = &element.el.props;
    let details: Vec<String> = [
        props.color.map(|c| c.label().to_lowercase()),
        props.dashed.then(|| "dashed".to_owned()),
        props.font_size.map(|s| format!("{s} px")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let label = element.info.label;
    let label = if kind == HighLevelKind::Group && element.child_count == 0 {
        "Empty group"
    } else {
        label
    };
    if details.is_empty() {
        label.to_owned()
    } else {
        format!("{label} ({})", details.join(", "))
    }
}

/// The element's arrows, for screen readers.
fn extra_details(eng: &Engine<HighLevel>, element: &Laid<HighLevel>) -> Vec<String> {
    let label = |id: &ElementId| eng.label_of(id).unwrap_or_default().to_owned();
    let mut to = Vec::new();
    let mut from = Vec::new();
    let mut both = Vec::new();
    for c in eng.connections() {
        let other = if c.source_id == *element.id() {
            &c.target_id
        } else if c.target_id == *element.id() {
            &c.source_id
        } else {
            continue;
        };
        if c.props.two_way {
            both.push(label(other));
        } else if c.source_id == *element.id() {
            to.push(label(other));
        } else {
            from.push(label(other));
        }
    }
    let mut details = Vec::new();
    if !to.is_empty() {
        details.push(format!("arrow to {}", list_labels(&to)));
    }
    if !from.is_empty() {
        details.push(format!("arrow from {}", list_labels(&from)));
    }
    if !both.is_empty() {
        details.push(format!("two-way arrow with {}", list_labels(&both)));
    }
    details
}

/// Applies an edited element with its properties as stored, saying when its colour changed.
fn apply_element(
    eng: &mut Engine<HighLevel>,
    id: &ElementId,
    mut with: El<HighLevel>,
    parent_id: Option<ElementId>,
) {
    let Some(before) = eng.element(id).cloned() else {
        return;
    };
    let kind = before.kind;
    with.props = kept(kind, with.props);
    if with.props.color != before.props.color {
        eng.announcement = format!(
            "{} set to {}.",
            with.label,
            with.props.color_of(kind).label().to_lowercase()
        );
    } else if with.props.dashed != before.props.dashed {
        eng.announcement = format!(
            "{} set to a {} border.",
            with.label,
            if with.props.dashed { "dashed" } else { "solid" }
        );
    }
    eng.apply_changes(id, with, parent_id);
}

/// Checks a font size: a whole number of pixels, from `FONT_SIZE_MIN` to `FONT_SIZE_MAX`.
pub fn validate_font_size(size: u32) -> Result<(), String> {
    if (FONT_SIZE_MIN..=FONT_SIZE_MAX).contains(&size) {
        Ok(())
    } else {
        Err(format!(
            "Font size must be from {FONT_SIZE_MIN} to {FONT_SIZE_MAX}."
        ))
    }
}

impl DiagramType for HighLevel {
    const ID: DiagramTypeId = DiagramTypeId::HighLevelConceptual;
    type Kind = HighLevelKind;
    type Props = HighLevelProps;
    type Arrow = BlockArrow;
    type Colors = NoColors;

    const ARROW_FIELDS: &'static [Field] = ARROW_FIELDS;
    const DESCRIPTION: DescriptionRule = DescriptionRule {
        max: DESCRIPTION_MAX,
        max_lines: None,
    };

    fn file_fields(kind: HighLevelKind) -> &'static [Field] {
        match kind {
            HighLevelKind::Application => APPLICATION_FIELDS,
            HighLevelKind::Group => GROUP_FIELDS,
            HighLevelKind::Actors => ACTORS_FIELDS,
        }
    }

    fn palette() -> &'static [KindInfo<HighLevelKind>] {
        &PALETTE
    }

    /// A group's header fits its name in its font size.
    fn header_height(element: &El<Self>, info: &'static KindInfo<HighLevelKind>) -> f64 {
        if element.kind == HighLevelKind::Group {
            header_for(f64::from(element.props.font_size_of(element.kind)))
        } else {
            info.header()
        }
    }

    fn kind_description(_eng: &Engine<Self>, element: &Laid<Self>) -> String {
        kind_description(element)
    }

    fn extra_details(eng: &Engine<Self>, element: &Laid<Self>) -> Vec<String> {
        extra_details(eng, element)
    }

    /// Any two elements, except a group and what's inside it: an arrow between them would
    /// run inside the group.
    fn can_connect(eng: &Engine<Self>, source: &Laid<Self>, target: &Laid<Self>) -> bool {
        source.id() != target.id()
            && source.info.connects_to.contains(&target.el.kind)
            && !descendant_ids(eng.elements(), source.id()).contains(target.id())
            && !descendant_ids(eng.elements(), target.id()).contains(source.id())
    }

    fn apply_element(
        eng: &mut Engine<Self>,
        id: &ElementId,
        with: El<Self>,
        parent_id: Option<ElementId>,
    ) {
        apply_element(eng, id, with, parent_id);
    }

    fn validate_element(element: &El<Self>) -> Result<(), String> {
        validate_label(&element.label)?;
        validate_length(
            "Description",
            element.description.as_deref().unwrap_or_default(),
            DESCRIPTION_MAX,
        )?;
        validate_font_size(element.props.font_size_of(element.kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;
    use crate::test_engine;
    use hiero_core::geometry::Point;

    fn id(s: &str) -> ElementId {
        s.into()
    }

    /// Applies `change` to element `of`, as its properties panel does.
    fn change(e: &mut Engine<HighLevel>, of: &str, change: impl FnOnce(&mut El<HighLevel>)) {
        let mut with = e.element(&id(of)).unwrap().clone();
        change(&mut with);
        let parent = with.parent_id.clone();
        HighLevel::apply_element(e, &id(of), with, parent);
    }

    #[test]
    fn headers_grow_with_the_font_on_the_grid() {
        assert_eq!(header_for(14.0), 40.0);
        assert_eq!(header_for(16.0), 40.0);
        assert_eq!(header_for(24.0), 60.0);
        assert_eq!(header_for(32.0), 60.0);
        assert_eq!(header_for(48.0), 80.0);
        let mut e = test_engine();
        e.palette_drop(HighLevelKind::Group, Point::new(400.0, 300.0));
        assert_eq!(e.laid(&id("id1")).unwrap().header, 40.0);
        change(&mut e, "id1", |w| w.props.font_size = Some(32));
        assert_eq!(e.laid(&id("id1")).unwrap().header, 60.0);
    }

    #[test]
    fn groups_hold_applications_and_groups() {
        let mut e = test_engine();
        e.palette_drop(HighLevelKind::Group, Point::new(600.0, 400.0));
        e.palette_drop(HighLevelKind::Group, Point::new(560.0, 380.0));
        e.palette_drop(HighLevelKind::Application, Point::new(540.0, 360.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        assert_eq!(e.element(&id("id3")).unwrap().parent_id, Some(id("id2")));
        let outer = e.laid(&id("id1")).unwrap().clone();
        let inner = e.laid(&id("id2")).unwrap().clone();
        assert!(outer.rect.contains_rect(&inner.rect));
        assert_eq!(HighLevel::kind_description(&e, &outer), "Group");
        e.palette_drop(HighLevelKind::Group, Point::new(1600.0, 1200.0));
        let empty = e.laid(&id("id4")).unwrap().clone();
        assert_eq!(HighLevel::kind_description(&e, &empty), "Empty group");
    }

    #[test]
    fn arrows_join_anything_but_a_group_and_what_it_holds() {
        let mut e = test_engine();
        e.palette_drop(HighLevelKind::Group, Point::new(600.0, 400.0));
        e.palette_drop(HighLevelKind::Application, Point::new(560.0, 380.0));
        e.palette_drop(HighLevelKind::Group, Point::new(1600.0, 400.0));
        e.palette_drop(HighLevelKind::Application, Point::new(1600.0, 1400.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        assert!(!e.can_connect(&id("id1"), &id("id2")));
        assert!(!e.can_connect(&id("id2"), &id("id1")));
        assert!(e.can_connect(&id("id1"), &id("id3")));
        assert!(e.can_connect(&id("id2"), &id("id3")));
        assert!(e.can_connect(&id("id4"), &id("id2")));
        let a = e.add_connection(&id("id1"), &id("id3")).unwrap();
        e.set_connection_props(&a, BlockArrow { two_way: true });
        e.add_connection(&id("id4"), &id("id3")).unwrap();
        let group = e.laid(&id("id3")).unwrap().clone();
        assert_eq!(
            HighLevel::extra_details(&e, &group),
            ["arrow from Application", "two-way arrow with Group"]
        );
    }

    #[test]
    fn looks_are_kept_apart_from_the_kinds_usual_one() {
        let mut e = test_engine();
        e.add_at_free_position(HighLevelKind::Application);
        e.add_at_free_position(HighLevelKind::Group);
        change(&mut e, "id1", |w| {
            w.props.color = Some(Color::Blue);
            w.props.font_size = Some(14);
        });
        // Blue and 14 are an application's own, so they're left out.
        assert_eq!(
            e.element(&id("id1")).unwrap().props,
            HighLevelProps::default()
        );
        change(&mut e, "id2", |w| w.props.color = Some(Color::Blue));
        assert_eq!(e.announcement, "Group set to blue.");
        change(&mut e, "id2", |w| w.props.dashed = true);
        assert_eq!(e.announcement, "Group set to a dashed border.");
        let props = &e.element(&id("id2")).unwrap().props;
        assert_eq!(props.color, Some(Color::Blue));
        assert!(props.dashed);
        let laid = e.laid(&id("id2")).unwrap().clone();
        assert_eq!(
            HighLevel::kind_description(&e, &laid),
            "Empty group (blue, dashed)"
        );
        change(&mut e, "id2", |w| w.props.color = Some(Color::Grey));
        assert_eq!(e.element(&id("id2")).unwrap().props.color, None);
    }

    #[test]
    fn font_sizes_are_whole_pixels_in_range() {
        let mut e = test_engine();
        e.add_at_free_position(HighLevelKind::Application);
        let mut with = e.element(&id("id1")).unwrap().clone();
        with.props.font_size = Some(60);
        assert_eq!(
            HighLevel::validate_element(&with).unwrap_err(),
            "Font size must be from 8 to 48."
        );
        with.props.font_size = Some(8);
        assert!(HighLevel::validate_element(&with).is_ok());
    }
}
