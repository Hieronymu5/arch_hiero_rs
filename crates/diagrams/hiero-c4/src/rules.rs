//! The C4 types' rules: what elements and relationships can hold, what screen readers hear
//! about them, and how they're checked. The three types share them; each fills in its kinds.

use hiero_core::engine::{
    DiagramType, El, Engine, Laid, list_labels, validate_label, validate_length,
};
use hiero_core::file_check::{DescriptionRule, Field, FieldRule};
use hiero_core::model::{DiagramTypeId, ElementId, KindInfo, NoColors};

use crate::model::{
    ARROW_DESCRIPTION_MAX, ARROW_TECHNOLOGY_MAX, C4Arrow, C4Component, C4Container, C4Context,
    C4Props, C4Type, COMPONENT_PALETTE, CONTAINER_PALETTE, CONTEXT_PALETTE, Color, ComponentKind,
    ContainerKind, ContextKind, DESCRIPTION_MAX, TECHNOLOGY_MAX, TYPE_NAME_MAX,
};

const COLOR_FIELD: Field = Field {
    key: "color",
    rule: FieldRule::OneOf {
        values: &["green", "orange", "red"],
        default: Some("blue"),
    },
};

const TECHNOLOGY_FIELD: Field = Field {
    key: "technology",
    rule: FieldRule::Text {
        name: "Technology",
        max: TECHNOLOGY_MAX,
    },
};

const TYPE_NAME_FIELD: Field = Field {
    key: "typeName",
    rule: FieldRule::Text {
        name: "Type",
        max: TYPE_NAME_MAX,
    },
};

/// Every element has a colour; containers and components also have a technology, and people
/// and software systems a type of their own.
const COLOR_FIELDS: &[Field] = &[COLOR_FIELD];
const COLOR_AND_TECHNOLOGY_FIELDS: &[Field] = &[COLOR_FIELD, TECHNOLOGY_FIELD];
const COLOR_AND_TYPE_FIELDS: &[Field] = &[COLOR_FIELD, TYPE_NAME_FIELD];

const DESCRIPTION_FIELD: Field = Field {
    key: "description",
    rule: FieldRule::Text {
        name: "Description",
        max: ARROW_DESCRIPTION_MAX,
    },
};

const ARROW_TECHNOLOGY_FIELD: Field = Field {
    key: "technology",
    rule: FieldRule::Text {
        name: "Technology",
        max: ARROW_TECHNOLOGY_MAX,
    },
};

/// A system context diagram's relationships have a description only.
const CONTEXT_ARROW_FIELDS: &[Field] = &[DESCRIPTION_FIELD];
/// Container and component diagrams' relationships have a technology too.
const ARROW_FIELDS: &[Field] = &[DESCRIPTION_FIELD, ARROW_TECHNOLOGY_FIELD];

/// The fields an element of `kind` has beyond the common ones.
fn file_fields<T: C4Type>(kind: T::Kind) -> &'static [Field] {
    let role = T::role(kind);
    if role.has_technology() {
        COLOR_AND_TECHNOLOGY_FIELDS
    } else if role.has_custom_type() {
        COLOR_AND_TYPE_FIELDS
    } else {
        COLOR_FIELDS
    }
}

/// Text as stored: trimmed, and unset when blank.
fn clean(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// A relationship as stored: its text trimmed, blank text left out, and no technology on a
/// system context diagram.
pub fn normalized_arrow<T: C4Type>(arrow: C4Arrow) -> C4Arrow {
    C4Arrow {
        description: clean(arrow.description),
        technology: clean(arrow.technology).filter(|_| T::ARROW_TECHNOLOGY),
    }
}

/// The type line an element shows, such as "[Container: Java and Spring Boot]", "[Person]", or
/// a person's or software system's own type, such as "[Customer]".
pub fn type_line<T: C4Type>(element: &El<T>) -> Option<String> {
    let role = T::role(element.kind);
    if let Some(own) = element
        .props
        .type_name
        .as_deref()
        .filter(|_| role.has_custom_type())
    {
        return Some(format!("[{own}]"));
    }
    let name = role.type_name()?;
    Some(
        match element
            .props
            .technology
            .as_deref()
            .filter(|_| role.has_technology())
        {
            Some(technology) => format!("[{name}: {technology}]"),
            None => format!("[{name}]"),
        },
    )
}

/// Such as "Software System (red)", "Database (MySQL)", or "Queue (Kafka, green)": its own
/// type, the technology, and the colour unless it's blue.
fn kind_description<T: C4Type>(element: &Laid<T>) -> String {
    let role = T::role(element.el.kind);
    let props = &element.el.props;
    let details: Vec<String> = [
        props.type_name.clone().filter(|_| role.has_custom_type()),
        props.technology.clone().filter(|_| role.has_technology()),
        props.color.map(|c| Color::label(Some(c)).to_lowercase()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let label = element.info.label;
    if details.is_empty() {
        label.to_owned()
    } else {
        format!("{label} ({})", details.join(", "))
    }
}

/// Such as " described as Makes API requests to, using JSON/HTTPS".
fn saying(arrow: &C4Arrow) -> String {
    let mut out = String::new();
    if let Some(d) = &arrow.description {
        out.push_str(&format!(" described as {d}"));
    }
    if let Some(t) = &arrow.technology {
        out.push_str(&format!(", using {t}"));
    }
    out
}

/// Such as "To Database [MySQL protocol]: Reads from and writes to", for the list of an
/// element's relationships.
pub fn arrow_title(arrow: &C4Arrow, outgoing: bool, other: &str) -> String {
    let mut out = format!("{} {other}", if outgoing { "To" } else { "From" });
    if let Some(t) = &arrow.technology {
        out.push_str(&format!(" [{t}]"));
    }
    if let Some(d) = &arrow.description {
        out.push_str(&format!(": {d}"));
    }
    out
}

/// The element's relationships, for screen readers.
fn extra_details<T: C4Type>(eng: &Engine<T>, element: &Laid<T>) -> Vec<String> {
    let label = |id: &ElementId| eng.label_of(id).unwrap_or_default().to_owned();
    let mut details: Vec<String> = eng
        .connections()
        .iter()
        .filter_map(|c| {
            if c.source_id == *element.id() {
                Some(format!(
                    "relationship to {}{}",
                    label(&c.target_id),
                    saying(&c.props)
                ))
            } else if c.target_id == *element.id() {
                Some(format!(
                    "relationship from {}{}",
                    label(&c.source_id),
                    saying(&c.props)
                ))
            } else {
                None
            }
        })
        .collect();
    if T::role(element.el.kind).is_boundary() {
        let held: Vec<String> = eng
            .elements()
            .iter()
            .filter(|e| e.parent_id.as_ref() == Some(element.id()))
            .map(|e| e.label.clone())
            .collect();
        if !held.is_empty() {
            details.push(format!("around {}", list_labels(&held)));
        }
    }
    details
}

/// Applies an edited element, keeping only the properties its kind has, and saying when its
/// colour changed.
fn apply_element<T: C4Type>(
    eng: &mut Engine<T>,
    id: &ElementId,
    mut with: El<T>,
    parent_id: Option<ElementId>,
) {
    let Some(selected) = eng.laid(id).cloned() else {
        return;
    };
    let role = T::role(selected.el.kind);
    if !selected.info.has_description {
        with.description.clone_from(&selected.el.description);
    }
    with.props.technology = clean(with.props.technology).filter(|_| role.has_technology());
    with.props.type_name = clean(with.props.type_name).filter(|_| role.has_custom_type());
    if with.props.color != selected.el.props.color {
        eng.announcement = format!(
            "{} set to {}.",
            with.label,
            Color::label(with.props.color).to_lowercase()
        );
    }
    eng.apply_changes(id, with, parent_id);
}

fn validate_element<T: C4Type>(element: &El<T>) -> Result<(), String> {
    validate_label(&element.label)?;
    validate_length(
        "Description",
        element.description.as_deref().unwrap_or_default(),
        DESCRIPTION_MAX,
    )?;
    validate_length(
        "Technology",
        element.props.technology.as_deref().unwrap_or_default(),
        TECHNOLOGY_MAX,
    )?;
    validate_length(
        "Type",
        element.props.type_name.as_deref().unwrap_or_default(),
        TYPE_NAME_MAX,
    )
}

fn validate_arrow(arrow: &C4Arrow) -> Result<(), String> {
    validate_length(
        "Description",
        arrow.description.as_deref().unwrap_or_default(),
        ARROW_DESCRIPTION_MAX,
    )?;
    validate_length(
        "Technology",
        arrow.technology.as_deref().unwrap_or_default(),
        ARROW_TECHNOLOGY_MAX,
    )
}

/// Implements `DiagramType` for a C4 type with the shared rules.
macro_rules! c4_type {
    ($ty:ty, $id:expr, $kind:ty, $palette:expr, $arrow_fields:expr) => {
        impl DiagramType for $ty {
            const ID: DiagramTypeId = $id;
            type Kind = $kind;
            type Props = C4Props;
            type Arrow = C4Arrow;
            type Colors = NoColors;

            const ARROW_FIELDS: &'static [Field] = $arrow_fields;
            const DESCRIPTION: DescriptionRule = DescriptionRule {
                max: DESCRIPTION_MAX,
                max_lines: None,
            };

            fn file_fields(kind: $kind) -> &'static [Field] {
                file_fields::<Self>(kind)
            }

            fn normalized_arrow(arrow: C4Arrow) -> C4Arrow {
                normalized_arrow::<Self>(arrow)
            }

            fn palette() -> &'static [KindInfo<$kind>] {
                &$palette
            }

            fn kind_description(_eng: &Engine<Self>, element: &Laid<Self>) -> String {
                kind_description(element)
            }

            fn extra_details(eng: &Engine<Self>, element: &Laid<Self>) -> Vec<String> {
                extra_details(eng, element)
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
                validate_element::<Self>(element)
            }

            fn validate_arrow(arrow: &C4Arrow) -> Result<(), String> {
                validate_arrow(arrow)
            }
        }
    };
}

c4_type!(
    C4Context,
    DiagramTypeId::C4Context,
    ContextKind,
    CONTEXT_PALETTE,
    CONTEXT_ARROW_FIELDS
);
c4_type!(
    C4Container,
    DiagramTypeId::C4Container,
    ContainerKind,
    CONTAINER_PALETTE,
    ARROW_FIELDS
);
c4_type!(
    C4Component,
    DiagramTypeId::C4Component,
    ComponentKind,
    COMPONENT_PALETTE,
    ARROW_FIELDS
);

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use hiero_core::geometry::Point;
    use hiero_core::model::NoColors;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    pub(crate) fn engine<T: C4Type>() -> Engine<T> {
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
    fn people_and_systems_relate_to_each_other() {
        let mut e = engine::<C4Context>();
        e.add_at_free_position(ContextKind::Person);
        e.add_at_free_position(ContextKind::SoftwareSystem);
        e.add_at_free_position(ContextKind::SoftwareSystem);
        assert!(e.can_connect(&id("id1"), &id("id2")));
        assert!(e.can_connect(&id("id2"), &id("id1")));
        assert!(e.can_connect(&id("id2"), &id("id3")));
        let system = e.laid(&id("id2")).unwrap().clone();
        assert_eq!(C4Context::kind_description(&e, &system), "Software System");
    }

    #[test]
    fn containers_go_in_a_system_boundary_which_takes_no_arrows() {
        let mut e = engine::<C4Container>();
        e.palette_drop(ContainerKind::SystemBoundary, Point::new(400.0, 300.0));
        e.palette_drop(ContainerKind::Backend, Point::new(300.0, 200.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        e.add_at_free_position(ContainerKind::Person);
        assert!(e.can_connect(&id("id3"), &id("id2")));
        assert!(!e.can_connect(&id("id3"), &id("id1")));
        let boundary = e.laid(&id("id1")).unwrap();
        assert!(boundary.is_region);
        // The backend sits clear of the boundary's text at its bottom.
        let backend = e.laid(&id("id2")).unwrap().rect;
        assert!(backend.bottom() + 60.0 <= boundary.rect.bottom());
    }

    #[test]
    fn a_system_boundary_holds_a_container_boundary_which_holds_components() {
        let mut e = engine::<C4Component>();
        e.palette_drop(ComponentKind::SystemBoundary, Point::new(600.0, 400.0));
        e.palette_drop(ComponentKind::ContainerBoundary, Point::new(500.0, 350.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        e.palette_drop(ComponentKind::Component, Point::new(400.0, 300.0));
        assert_eq!(e.element(&id("id3")).unwrap().parent_id, Some(id("id2")));
        let boundary = e.laid(&id("id1")).unwrap().clone();
        assert!(e.accessible_label(&boundary).contains("around Container"));
    }

    #[test]
    fn technology_shows_in_the_type_line_and_colour_changes_are_announced() {
        let mut e = engine::<C4Container>();
        e.add_at_free_position(ContainerKind::Database);
        e.add_at_free_position(ContainerKind::SoftwareSystem);
        e.select(&id("id1"));
        let mut with = e.element(&id("id1")).unwrap().clone();
        with.props.technology = Some("  MySQL  ".into());
        with.props.color = Some(Color::Green);
        C4Container::apply_element(&mut e, &id("id1"), with, None);
        assert_eq!(e.announcement, "Database set to green.");
        let database = e.element(&id("id1")).unwrap();
        assert_eq!(database.props.technology.as_deref(), Some("MySQL"));
        assert_eq!(database.props.color, Some(Color::Green));
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(
            C4Container::kind_description(&e, &laid),
            "Database (MySQL, green)"
        );
        assert_eq!(
            type_line::<C4Container>(database).as_deref(),
            Some("[Container: MySQL]")
        );
        let mut with = e.element(&id("id2")).unwrap().clone();
        with.props.color = Some(Color::Red);
        // A software system has no technology.
        with.props.technology = Some("Java".into());
        C4Container::apply_element(&mut e, &id("id2"), with, None);
        assert_eq!(e.announcement, "Software System set to red.");
        assert_eq!(e.element(&id("id2")).unwrap().props.technology, None);
        assert_eq!(
            type_line::<C4Container>(e.element(&id("id2")).unwrap()).as_deref(),
            Some("[Software System]")
        );
    }

    #[test]
    fn people_and_software_systems_can_name_their_own_type() {
        let mut e = engine::<C4Context>();
        e.add_at_free_position(ContextKind::Person);
        e.add_at_free_position(ContextKind::SoftwareSystem);
        let type_of = |e: &Engine<C4Context>, id: &str| {
            type_line::<C4Context>(e.element(&id.into()).unwrap())
        };
        // [Person] and [Software System] unless they say otherwise.
        assert_eq!(type_of(&e, "id1").as_deref(), Some("[Person]"));
        assert_eq!(type_of(&e, "id2").as_deref(), Some("[Software System]"));
        for (id, own) in [("id1", " Customer "), ("id2", "External System")] {
            let mut with = e.element(&id.into()).unwrap().clone();
            with.props.type_name = Some(own.into());
            C4Context::apply_element(&mut e, &id.into(), with, None);
        }
        assert_eq!(type_of(&e, "id1").as_deref(), Some("[Customer]"));
        assert_eq!(type_of(&e, "id2").as_deref(), Some("[External System]"));
        let laid = e.laid(&"id2".into()).unwrap().clone();
        assert_eq!(
            C4Context::kind_description(&e, &laid),
            "Software System (External System)"
        );
        // Blank goes back to the usual type.
        let mut with = e.element(&"id1".into()).unwrap().clone();
        with.props.type_name = Some("  ".into());
        C4Context::apply_element(&mut e, &"id1".into(), with, None);
        assert_eq!(type_of(&e, "id1").as_deref(), Some("[Person]"));
        // People have a description, and other kinds can't name their type.
        assert!(e.laid(&"id1".into()).unwrap().info.has_description);
        let mut c = engine::<C4Container>();
        c.add_at_free_position(ContainerKind::Backend);
        let mut with = c.element(&"id1".into()).unwrap().clone();
        with.props.type_name = Some("API".into());
        C4Container::apply_element(&mut c, &"id1".into(), with, None);
        assert_eq!(c.element(&"id1".into()).unwrap().props.type_name, None);
        let mut long = e.element(&"id2".into()).unwrap().clone();
        long.props.type_name = Some("x".repeat(41));
        assert_eq!(
            C4Context::validate_element(&long).unwrap_err(),
            "Type must be 40 characters or fewer."
        );
    }

    #[test]
    fn context_relationships_leave_technology_out() {
        let arrow = C4Arrow {
            description: Some(" Uses ".into()),
            technology: Some("HTTPS".into()),
        };
        assert_eq!(
            normalized_arrow::<C4Context>(arrow.clone()),
            C4Arrow {
                description: Some("Uses".into()),
                technology: None
            }
        );
        assert_eq!(
            normalized_arrow::<C4Container>(arrow).technology.as_deref(),
            Some("HTTPS")
        );
        assert_eq!(
            arrow_title(
                &C4Arrow {
                    description: Some("Reads from".into()),
                    technology: Some("SQL".into())
                },
                true,
                "Database"
            ),
            "To Database [SQL]: Reads from"
        );
    }

    #[test]
    fn validation_limits_lengths() {
        let mut e = engine::<C4Container>();
        e.add_at_free_position(ContainerKind::Backend);
        let mut el = e.elements()[0].clone();
        el.props.technology = Some("x".repeat(61));
        assert_eq!(
            C4Container::validate_element(&el).unwrap_err(),
            "Technology must be 60 characters or fewer."
        );
        let arrow = C4Arrow {
            description: Some("x".repeat(151)),
            technology: None,
        };
        assert!(C4Container::validate_arrow(&arrow).is_err());
    }
}
