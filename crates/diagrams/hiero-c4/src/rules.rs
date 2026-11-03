//! The C4 type's rules: what elements and relationships can hold, the shape and type line each
//! element shows, what screen readers hear about them, and how they're checked.

use hiero_core::c4::C4Shape;
use hiero_core::engine::{
    Created, DiagramType, El, Engine, Laid, Placement, list_labels, validate_label, validate_length,
};
use hiero_core::file_check::{DescriptionRule, Field, FieldRule};
use hiero_core::geometry::Size;
use hiero_core::model::{DiagramTypeId, ElementId, KindInfo, NoColors};

use crate::model::{
    ARROW_DESCRIPTION_MAX, ARROW_TECHNOLOGY_MAX, BoundaryType, C4, C4Arrow, C4Kind, C4Props, Color,
    ContainerType, DESCRIPTION_MAX, PALETTE, TECHNOLOGY_MAX, TYPE_NAME_MAX,
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

const CONTAINER_TYPE_FIELD: Field = Field {
    key: "containerType",
    rule: FieldRule::OneOf {
        values: &[
            "web-application",
            "static-content",
            "backend",
            "service",
            "queue",
            "topic",
            "store",
            "database",
        ],
        default: Some("default"),
    },
};

const BOUNDARY_TYPE_FIELD: Field = Field {
    key: "boundaryType",
    rule: FieldRule::OneOf {
        values: &["container"],
        default: Some("system"),
    },
};

/// Every element has a colour. People and systems can name their own type; containers have a
/// type that sets their shape, and a technology, as components do; boundaries say what
/// they're round.
const PERSON_FIELDS: &[Field] = &[COLOR_FIELD, TYPE_NAME_FIELD];
const CONTAINER_FIELDS: &[Field] = &[COLOR_FIELD, CONTAINER_TYPE_FIELD, TECHNOLOGY_FIELD];
const COMPONENT_FIELDS: &[Field] = &[COLOR_FIELD, TECHNOLOGY_FIELD];
const BOUNDARY_FIELDS: &[Field] = &[COLOR_FIELD, BOUNDARY_TYPE_FIELD];

const ARROW_FIELDS: &[Field] = &[
    Field {
        key: "description",
        rule: FieldRule::Text {
            name: "Description",
            max: ARROW_DESCRIPTION_MAX,
        },
    },
    Field {
        key: "technology",
        rule: FieldRule::Text {
            name: "Technology",
            max: ARROW_TECHNOLOGY_MAX,
        },
    },
];

/// Text as stored: trimmed, and unset when blank.
fn clean(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// A relationship as stored: its text trimmed and blank text left out.
pub fn normalized_arrow(arrow: C4Arrow) -> C4Arrow {
    C4Arrow {
        description: clean(arrow.description),
        technology: clean(arrow.technology),
    }
}

/// The shape an element is drawn as.
pub fn shape_of(element: &El<C4>) -> C4Shape {
    match element.kind {
        C4Kind::Person => C4Shape::Person,
        C4Kind::System => C4Shape::Box,
        C4Kind::Container => ContainerType::shape(element.props.container_type),
        C4Kind::Component => C4Shape::Component,
        C4Kind::Boundary => C4Shape::Boundary,
    }
}

/// The type line an element shows, such as "[Container: Java and Spring Boot]", "[Person]",
/// a person's or system's own type, such as "[Customer]", or a boundary's "[Container]".
pub fn type_line(element: &El<C4>) -> String {
    let props = &element.props;
    if let Some(own) = props
        .type_name
        .as_deref()
        .filter(|_| element.kind.has_custom_type())
    {
        return format!("[{own}]");
    }
    let name = match element.kind {
        C4Kind::Person => "Person",
        C4Kind::System => "Software System",
        C4Kind::Container => "Container",
        C4Kind::Component => "Component",
        C4Kind::Boundary => BoundaryType::type_name(props.boundary_type),
    };
    match props
        .technology
        .as_deref()
        .filter(|_| element.kind.has_technology())
    {
        Some(technology) => format!("[{name}: {technology}]"),
        None => format!("[{name}]"),
    }
}

/// Such as "Container (Database, MySQL, red)" or "Boundary (Container)": what sets it apart.
fn kind_description(element: &Laid<C4>) -> String {
    let props = &element.el.props;
    let kind = element.el.kind;
    let details: Vec<String> = [
        (kind == C4Kind::Container && props.container_type.is_some())
            .then(|| ContainerType::label(props.container_type).to_owned()),
        (kind == C4Kind::Boundary).then(|| BoundaryType::label(props.boundary_type).to_owned()),
        props.type_name.clone().filter(|_| kind.has_custom_type()),
        props.technology.clone().filter(|_| kind.has_technology()),
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

/// The element's relationships, and what a boundary is round, for screen readers.
fn extra_details(eng: &Engine<C4>, element: &Laid<C4>) -> Vec<String> {
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
    if element.el.kind == C4Kind::Boundary {
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

/// The element's properties with only those its kind has, and its text trimmed.
fn kept(kind: C4Kind, props: C4Props) -> C4Props {
    C4Props {
        color: props.color,
        container_type: props.container_type.filter(|_| kind == C4Kind::Container),
        boundary_type: props.boundary_type.filter(|_| kind == C4Kind::Boundary),
        technology: clean(props.technology).filter(|_| kind.has_technology()),
        type_name: clean(props.type_name).filter(|_| kind.has_custom_type()),
    }
}

/// Applies an edited element, keeping only the properties its kind has, and saying when its
/// colour or type changed.
fn apply_element(
    eng: &mut Engine<C4>,
    id: &ElementId,
    mut with: El<C4>,
    parent_id: Option<ElementId>,
) {
    let Some(selected) = eng.laid(id).cloned() else {
        return;
    };
    let kind = selected.el.kind;
    with.props = kept(kind, with.props);
    let before = &selected.el.props;
    let set_to = if with.props.container_type != before.container_type {
        Some(ContainerType::label(with.props.container_type).to_lowercase())
    } else if with.props.boundary_type != before.boundary_type {
        Some(format!(
            "a {} boundary",
            BoundaryType::label(with.props.boundary_type).to_lowercase()
        ))
    } else if with.props.color != before.color {
        Some(Color::label(with.props.color).to_lowercase())
    } else {
        None
    };
    if let Some(what) = set_to {
        eng.announcement = format!("{} set to {what}.", with.label);
    }
    eng.apply_changes(id, with, parent_id);
}

/// What a diagram added for `element` starts as, where its kind says: for a system, a
/// container diagram with a boundary round the system; for a container, a component diagram
/// with a container boundary round the container. Each boundary is named after the element.
/// Other elements' diagrams start empty, with no sub type of their own.
pub fn sub_diagram(element: &El<C4>) -> Option<(&'static str, El<C4>)> {
    let (subtype, boundary_type) = match element.kind {
        C4Kind::System => ("container", None),
        C4Kind::Container => ("component", Some(BoundaryType::Container)),
        _ => return None,
    };
    let boundary = El::<C4> {
        id: ElementId::new_v4(),
        kind: C4Kind::Boundary,
        label: element.label.clone(),
        description: None,
        parent_id: None,
        sub_diagram_id: None,
        props: C4Props {
            boundary_type,
            ..C4Props::default()
        },
        width: None,
        height: None,
        x: SUB_DIAGRAM_BOUNDARY_AT,
        y: SUB_DIAGRAM_BOUNDARY_AT,
    };
    Some((subtype, boundary))
}

/// Where the boundary a sub diagram starts with sits: a grid square and more from the corner.
const SUB_DIAGRAM_BOUNDARY_AT: f64 = 40.0;

fn validate_element(element: &El<C4>) -> Result<(), String> {
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

impl DiagramType for C4 {
    const ID: DiagramTypeId = DiagramTypeId::C4;
    type Kind = C4Kind;
    type Props = C4Props;
    type Arrow = C4Arrow;
    type Colors = NoColors;

    const ARROW_FIELDS: &'static [Field] = ARROW_FIELDS;
    const DESCRIPTION: DescriptionRule = DescriptionRule {
        max: DESCRIPTION_MAX,
        max_lines: None,
    };

    fn file_fields(kind: C4Kind) -> &'static [Field] {
        match kind {
            C4Kind::Person | C4Kind::System => PERSON_FIELDS,
            C4Kind::Container => CONTAINER_FIELDS,
            C4Kind::Component => COMPONENT_FIELDS,
            C4Kind::Boundary => BOUNDARY_FIELDS,
        }
    }

    fn normalized_arrow(arrow: C4Arrow) -> C4Arrow {
        normalized_arrow(arrow)
    }

    fn palette() -> &'static [KindInfo<C4Kind>] {
        &PALETTE
    }

    /// A container from one of the palette's container types starts as that type, named
    /// after it, such as a Database.
    fn created(
        eng: &mut Engine<Self>,
        kind: &'static KindInfo<C4Kind>,
        placement: Placement,
    ) -> Created<Self> {
        let mut created = eng.base_created(kind, placement);
        if let Some(first) = created.added.first_mut() {
            first.props.container_type = ContainerType::of_entry(kind);
        }
        created
    }

    /// A container's size comes from its type, such as a queue's lower cylinder.
    fn natural_size(element: &El<Self>, info: &'static KindInfo<C4Kind>) -> Size {
        if element.kind == C4Kind::Container {
            ContainerType::size(element.props.container_type)
        } else {
            info.size()
        }
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
        validate_element(element)
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
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{QUEUE_SIZE, SERVICE_SIZE};
    use hiero_core::geometry::Point;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    pub(crate) fn engine() -> Engine<C4> {
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

    /// Applies `change` to element `of`, as its properties panel does.
    fn change(e: &mut Engine<C4>, of: &str, change: impl FnOnce(&mut El<C4>)) {
        let mut with = e.element(&id(of)).unwrap().clone();
        change(&mut with);
        e.select(&id(of));
        C4::apply_element(
            e,
            &id(of),
            with,
            e.element(&id(of)).unwrap().parent_id.clone(),
        );
    }

    #[test]
    fn a_systems_or_containers_diagram_starts_with_a_boundary_round_it() {
        let mut e = engine();
        e.add_at_free_position(C4Kind::System);
        e.add_entry_at_free_position(PALETTE.iter().find(|k| k.label == "Service").unwrap());
        e.add_at_free_position(C4Kind::Person);
        change(&mut e, "id1", |w| w.label = "Internet Banking".into());
        change(&mut e, "id2", |w| w.label = "Orders API".into());
        let el = |of: &str| e.element(&id(of)).unwrap().clone();
        let (subtype, boundary) = sub_diagram(&el("id1")).unwrap();
        assert_eq!(subtype, "container");
        assert_eq!(boundary.kind, C4Kind::Boundary);
        assert_eq!(boundary.label, "Internet Banking");
        assert_eq!(type_line(&boundary), "[Software System]");
        let (subtype, boundary) = sub_diagram(&el("id2")).unwrap();
        assert_eq!(subtype, "component");
        assert_eq!(boundary.label, "Orders API");
        assert_eq!(type_line(&boundary), "[Container]");
        assert!(sub_diagram(&el("id3")).is_none());
    }

    #[test]
    fn boundaries_take_no_arrows() {
        let mut e = engine();
        e.add_at_free_position(C4Kind::Person);
        e.add_at_free_position(C4Kind::System);
        e.add_at_free_position(C4Kind::Boundary);
        assert!(e.can_connect(&id("id1"), &id("id2")));
        assert!(e.can_connect(&id("id2"), &id("id1")));
        assert!(!e.can_connect(&id("id1"), &id("id3")));
        assert!(!e.can_connect(&id("id3"), &id("id1")));
    }

    #[test]
    fn the_palette_offers_container_then_each_kind_of_container() {
        let labels: Vec<&str> = PALETTE.iter().map(|k| k.label).collect();
        assert_eq!(
            labels,
            [
                "Person",
                "System",
                "Container",
                "Website",
                "Static Content",
                "Backend",
                "Service",
                "Queue",
                "Topic",
                "Store",
                "Database",
                "Component",
                "Boundary"
            ]
        );
        // The kinds themselves are still five.
        let kinds: Vec<C4Kind> = C4::kinds().iter().map(|k| k.kind).collect();
        assert_eq!(
            kinds,
            [
                C4Kind::Person,
                C4Kind::System,
                C4Kind::Container,
                C4Kind::Component,
                C4Kind::Boundary
            ]
        );
        // Each container entry adds a container of its type, named after it; the generic one,
        // a plain box. Its type can be changed afterwards.
        let mut e = engine();
        let entry = |label: &str| PALETTE.iter().find(|k| k.label == label).unwrap();
        e.add_entry_at_free_position(entry("Database"));
        e.palette_drop_entry(entry("Queue"), Point::new(800.0, 600.0));
        e.add_entry_at_free_position(entry("Container"));
        let el = |e: &Engine<C4>, of: &str| e.element(&id(of)).unwrap().clone();
        assert_eq!(
            el(&e, "id1").props.container_type,
            Some(ContainerType::Database)
        );
        assert_eq!(el(&e, "id1").label, "Database");
        assert_eq!(el(&e, "id1").kind, C4Kind::Container);
        assert_eq!(
            el(&e, "id2").props.container_type,
            Some(ContainerType::Queue)
        );
        assert_eq!(e.laid(&id("id2")).unwrap().rect.size(), QUEUE_SIZE);
        assert_eq!(el(&e, "id3").props.container_type, None);
        assert_eq!(el(&e, "id3").label, "Container");
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(C4::kind_description(&e, &laid), "Container (Database)");
        change(&mut e, "id1", |w| {
            w.props.container_type = Some(ContainerType::Store);
        });
        assert_eq!(
            el(&e, "id1").props.container_type,
            Some(ContainerType::Store)
        );
    }

    #[test]
    fn a_containers_type_sets_its_shape_size_and_reading() {
        let mut e = engine();
        e.add_at_free_position(C4Kind::Container);
        let el = |e: &Engine<C4>| e.element(&id("id1")).unwrap().clone();
        assert_eq!(shape_of(&el(&e)), C4Shape::Box);
        assert_eq!(type_line(&el(&e)), "[Container]");
        change(&mut e, "id1", |w| {
            w.props.container_type = Some(ContainerType::Queue);
            w.props.technology = Some(" Kafka ".into());
            w.props.type_name = Some("Bus".into());
        });
        assert_eq!(e.announcement, "Container set to queue.");
        assert_eq!(shape_of(&el(&e)), C4Shape::Pipe);
        assert_eq!(type_line(&el(&e)), "[Container: Kafka]");
        // A container can't rename its type.
        assert_eq!(el(&e).props.type_name, None);
        assert_eq!(e.laid(&id("id1")).unwrap().rect.size(), QUEUE_SIZE);
        change(&mut e, "id1", |w| {
            w.props.container_type = Some(ContainerType::Service);
        });
        assert_eq!(e.laid(&id("id1")).unwrap().rect.size(), SERVICE_SIZE);
        change(&mut e, "id1", |w| w.props.color = Some(Color::Red));
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(
            C4::kind_description(&e, &laid),
            "Container (Service, Kafka, red)"
        );
    }

    #[test]
    fn boundaries_are_round_a_system_or_a_container_and_nest() {
        let mut e = engine();
        e.palette_drop(C4Kind::Boundary, Point::new(600.0, 400.0));
        e.palette_drop(C4Kind::Boundary, Point::new(500.0, 350.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        change(&mut e, "id2", |w| {
            w.props.boundary_type = Some(BoundaryType::Container);
        });
        assert_eq!(e.announcement, "Boundary set to a container boundary.");
        assert_eq!(
            type_line(e.element(&id("id1")).unwrap()),
            "[Software System]"
        );
        assert_eq!(type_line(e.element(&id("id2")).unwrap()), "[Container]");
        e.palette_drop(C4Kind::Component, Point::new(400.0, 300.0));
        assert_eq!(e.element(&id("id3")).unwrap().parent_id, Some(id("id2")));
        let outer = e.laid(&id("id1")).unwrap().clone();
        assert!(e.accessible_label(&outer).contains("around Boundary"));
        // The outer boundary can't be put inside the one it holds: it stays where it is.
        e.select(&id("id1"));
        assert!(e.parent_options().is_empty());
        let with = e.element(&id("id1")).unwrap().clone();
        C4::apply_element(&mut e, &id("id1"), with, Some(id("id2")));
        assert_eq!(e.element(&id("id1")).unwrap().parent_id, None);
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
    }

    #[test]
    fn people_and_systems_can_name_their_own_type() {
        let mut e = engine();
        e.add_at_free_position(C4Kind::Person);
        e.add_at_free_position(C4Kind::System);
        let type_of = |e: &Engine<C4>, of: &str| type_line(e.element(&id(of)).unwrap());
        assert_eq!(type_of(&e, "id1"), "[Person]");
        assert_eq!(type_of(&e, "id2"), "[Software System]");
        change(&mut e, "id1", |w| {
            w.props.type_name = Some(" Customer ".into());
        });
        change(&mut e, "id2", |w| {
            w.props.type_name = Some("External System".into());
            w.props.technology = Some("Java".into());
        });
        assert_eq!(type_of(&e, "id1"), "[Customer]");
        assert_eq!(type_of(&e, "id2"), "[External System]");
        // A system has no technology.
        assert_eq!(e.element(&id("id2")).unwrap().props.technology, None);
        change(&mut e, "id1", |w| w.props.type_name = Some("  ".into()));
        assert_eq!(type_of(&e, "id1"), "[Person]");
        let mut long = e.element(&id("id2")).unwrap().clone();
        long.props.type_name = Some("x".repeat(41));
        assert_eq!(
            C4::validate_element(&long).unwrap_err(),
            "Type must be 40 characters or fewer."
        );
    }

    #[test]
    fn relationships_are_trimmed_and_described() {
        let arrow = normalized_arrow(C4Arrow {
            description: Some(" Uses ".into()),
            technology: Some(" ".into()),
        });
        assert_eq!(arrow.description.as_deref(), Some("Uses"));
        assert_eq!(arrow.technology, None);
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
        let long = C4Arrow {
            description: Some("x".repeat(151)),
            technology: None,
        };
        assert!(C4::validate_arrow(&long).is_err());
    }
}
