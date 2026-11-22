//! The conceptual diagram's rules: figures for actors, files, and some components, and what screen
//! readers hear about arrows.

use hiero_core::engine::{
    Child, Conn, Created, DiagramType, El, Engine, Laid, Placement, descendant_ids, validate_label,
    validate_length,
};
use hiero_core::figures::component::FIGURE_SIZE;
use hiero_core::file_check::{DescriptionRule, Field, FieldRule};
use hiero_core::geometry::{GRID, Size};
use hiero_core::layout::{CONTAINER_HEADER, CONTAINER_PADDING};
use hiero_core::look::{DESCRIPTION_GAP, DiagramColors, container_description};
use hiero_core::model::{DiagramTypeId, ElementId, KindInfo};
use hiero_core::name_size::{header_for, on_grid, validate_font_size};
use hiero_core::text::MeasureText;

use crate::model::{
    ARROW_LABEL_MAX, ArrowMode, CONNECTOR_MAX, ComponentType, Conceptual, ConceptualArrow,
    ConceptualKind, ConceptualProps, DATABASE_ENTRY, DESCRIPTION_MAX, NAME_FONT_SIZE, PALETTE,
    STORAGE_ENTRY, TRANSFORMATION_MAX,
};

/// The component type an element added from the palette entry `entry` starts as: a database
/// from the Database entry, and storage from the Storage one.
pub fn entry_component_type(entry: &KindInfo<ConceptualKind>) -> Option<ComponentType> {
    match entry.palette_label {
        Some(DATABASE_ENTRY) if entry.kind.has_component_type() => Some(ComponentType::Database),
        Some(STORAGE_ENTRY) if entry.kind.has_component_type() => Some(ComponentType::Storage),
        _ => None,
    }
}

/// Whether `child` is a component that is, or starts as, a database or storage.
fn typed_component(child: Child<'_, Conceptual>) -> bool {
    match child {
        Child::Element(e) => e.kind.has_component_type() && e.props.component_type.is_some(),
        Child::Entry(k) => entry_component_type(k).is_some(),
    }
}

/// A figure drawn above an element's name instead of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    Person,
    Bot,
    Database,
    Storage,
    File,
}

const FONT_SIZE: Field = Field {
    key: "fontSize",
    rule: FieldRule::Number,
};

const NAME_POSITION: Field = Field {
    key: "namePosition",
    rule: FieldRule::OneOf {
        values: &["bottom"],
        default: Some("top"),
    },
};

/// The height of a line of a box's name in `size`, as it's drawn.
fn name_line_height(size: u32) -> f64 {
    (f64::from(size) * 1.5).round()
}

/// The height of the band a domain's name takes along its top or bottom: the usual header for
/// its own size or smaller, and taller for a larger name, to the grid, so it stays clear of the
/// applications it holds. Any description under the name makes it taller again, wrapped across
/// the domain's `width` as it's drawn.
pub fn name_band(element: &El<Conceptual>, width: f64, measure: &dyn MeasureText) -> f64 {
    let size = element.props.font_size_of(element.kind);
    let band = if size <= element.kind.default_font_size() {
        CONTAINER_HEADER
    } else {
        header_for(f64::from(size))
    };
    let description = container_description(element.description.as_deref(), width, measure);
    if description.lines.is_empty() {
        return band;
    }
    // As the name is drawn: 8 px from the edge, on a line 1.3 times its size, with the
    // description under it and at least the usual header's 6 px after that.
    let name_line = (f64::from(size) * 1.3).round();
    let text = name_line + DESCRIPTION_GAP + description.height();
    band.max(on_grid(8.0 + text + 6.0))
}

/// How much taller a box's header gets for its name in its own size, such as an application's
/// above the components it holds: the extra height of a larger name's line, to the grid.
pub fn larger_name(element: &El<Conceptual>) -> f64 {
    let extra = name_line_height(element.props.font_size_of(element.kind))
        - name_line_height(NAME_FONT_SIZE);
    if extra > 0.0 {
        (extra / GRID).ceil() * GRID
    } else {
        0.0
    }
}

/// The figure an element is drawn as, if any.
pub fn figure_of(element: &El<Conceptual>) -> Option<Figure> {
    if element.kind == ConceptualKind::Actor {
        return Some(if element.props.agent {
            Figure::Bot
        } else {
            Figure::Person
        });
    }
    if element.kind == ConceptualKind::File {
        return Some(Figure::File);
    }
    if element.kind.has_component_type() {
        return element.props.component_type.map(|t| match t {
            ComponentType::Database => Figure::Database,
            ComponentType::Storage => Figure::Storage,
        });
    }
    None
}

/// Whether an element is drawn as a cloud.
pub fn is_cloud(element: &El<Conceptual>) -> bool {
    element.kind.can_be_cloud_hosted() && element.props.cloud_hosted
}

/// Such as " via API with transformation Map fields carrying an e-mail labelled Orders".
fn saying(arrow: &ConceptualArrow) -> String {
    let mut out = String::new();
    if let Some(c) = &arrow.connector {
        out.push_str(&format!(" via {c}"));
    }
    if let Some(t) = &arrow.transformation {
        out.push_str(&format!(" with transformation {t}"));
    }
    if !arrow.payload.is_none() {
        out.push_str(&format!(" carrying {}", arrow.payload.spoken()));
    }
    if let Some(l) = &arrow.label {
        out.push_str(&format!(" labelled {l}"));
    }
    out
}

/// Such as "manual bi-directional arrow".
fn arrow_kind(arrow: &ConceptualArrow) -> String {
    format!(
        "{}{}arrow",
        if arrow.is_manual() { "manual " } else { "" },
        if arrow.bidirectional {
            "bi-directional "
        } else {
            ""
        }
    )
}

/// Such as "To CRM via API with transformation Map fields (manual): Orders", for the list of an
/// element's arrows.
pub fn arrow_title(arrow: &ConceptualArrow, outgoing: bool, other: &str) -> String {
    let mut traits = Vec::new();
    if arrow.is_manual() {
        traits.push("manual");
    }
    if arrow.bidirectional {
        traits.push("bi-directional");
    }
    let carries = format!("carries {}", arrow.payload.spoken());
    if !arrow.payload.is_none() {
        traits.push(&carries);
    }
    let mut out = format!("{} {other}", if outgoing { "To" } else { "From" });
    if let Some(c) = &arrow.connector {
        out.push_str(&format!(" via {c}"));
    }
    if let Some(t) = &arrow.transformation {
        out.push_str(&format!(" with transformation {t}"));
    }
    if !traits.is_empty() {
        out.push_str(&format!(" ({})", traits.join(", ")));
    }
    if let Some(l) = &arrow.label {
        out.push_str(&format!(": {l}"));
    }
    out
}

impl DiagramType for Conceptual {
    const ID: DiagramTypeId = DiagramTypeId::Conceptual;
    type Kind = ConceptualKind;
    type Props = ConceptualProps;
    type Arrow = ConceptualArrow;
    type Colors = DiagramColors;

    const ARROW_FIELDS: &'static [Field] = &[
        Field {
            key: "mode",
            rule: FieldRule::OneOf {
                values: &["manual"],
                default: Some("automated"),
            },
        },
        Field {
            key: "bidirectional",
            rule: FieldRule::True,
        },
        Field {
            key: "payload",
            rule: FieldRule::OneOf {
                values: &["email", "file"],
                default: Some("none"),
            },
        },
        Field {
            key: "label",
            rule: FieldRule::Text {
                name: "Text",
                max: ARROW_LABEL_MAX,
            },
        },
        Field {
            key: "connector",
            rule: FieldRule::Text {
                name: "Connector",
                max: CONNECTOR_MAX,
            },
        },
        Field {
            key: "transformation",
            rule: FieldRule::Text {
                name: "Transformation",
                max: TRANSFORMATION_MAX,
            },
        },
        Field {
            key: "manualOffset",
            rule: FieldRule::Number,
        },
    ];
    const DESCRIPTION: DescriptionRule = DescriptionRule {
        max: DESCRIPTION_MAX,
        max_lines: None,
    };

    fn file_fields(kind: ConceptualKind) -> &'static [Field] {
        match kind {
            ConceptualKind::Actor => &[
                Field {
                    key: "agent",
                    rule: FieldRule::True,
                },
                Field {
                    key: "group",
                    rule: FieldRule::True,
                },
            ],
            ConceptualKind::Domain => &[FONT_SIZE, NAME_POSITION],
            ConceptualKind::Component => &[
                Field {
                    key: "componentType",
                    rule: FieldRule::OneOf {
                        values: &["database", "storage"],
                        default: Some("default"),
                    },
                },
                FONT_SIZE,
            ],
            ConceptualKind::Application => &[
                Field {
                    key: "cloudHosted",
                    rule: FieldRule::True,
                },
                FONT_SIZE,
            ],
            ConceptualKind::Process | ConceptualKind::Job | ConceptualKind::Entity => &[FONT_SIZE],
            ConceptualKind::File | ConceptualKind::Section => &[],
        }
    }

    fn normalized_arrow(arrow: ConceptualArrow) -> ConceptualArrow {
        normalized_arrow(arrow)
    }

    fn palette() -> &'static [KindInfo<ConceptualKind>] {
        &PALETTE
    }

    /// A component drawn as a figure is taller, to fit its name under the figure.
    fn natural_size(element: &El<Self>, info: &'static KindInfo<ConceptualKind>) -> Size {
        match figure_of(element) {
            Some(Figure::Database | Figure::Storage) => FIGURE_SIZE,
            _ => info.size(),
        }
    }

    /// A domain's name takes a band along its top, as tall as its font size needs, or along its
    /// bottom, leaving the usual gap above what it holds. A box's header, such as an
    /// application's, grows for a larger name.
    fn header_height(
        element: &El<Self>,
        info: &'static KindInfo<ConceptualKind>,
        width: f64,
        measure: &dyn MeasureText,
    ) -> f64 {
        if element.kind.has_name_position() {
            if element.props.name_at_bottom() {
                CONTAINER_PADDING
            } else {
                name_band(element, width, measure)
            }
        } else if element.kind.has_font_size() {
            info.header() + larger_name(element)
        } else {
            info.header()
        }
    }

    /// A domain's name along its bottom takes a band below what it holds, past the usual gap.
    fn footer_height(
        element: &El<Self>,
        info: &'static KindInfo<ConceptualKind>,
        width: f64,
        measure: &dyn MeasureText,
    ) -> f64 {
        if element.kind.has_name_position() && element.props.name_at_bottom() {
            name_band(element, width, measure) - CONTAINER_PADDING
        } else {
            info.footer()
        }
    }

    /// A component holds another component only as a database or storage, such as the database
    /// a service keeps its data in.
    fn hold_problem(
        container: &'static KindInfo<ConceptualKind>,
        child: Child<'_, Self>,
    ) -> Option<String> {
        (container.kind == ConceptualKind::Component
            && child.kind() == ConceptualKind::Component
            && !typed_component(child))
        .then(|| "A component can hold another component only as a database or storage.".into())
    }

    /// A person, a file, a database, or storage can't be resized to line up with what's under
    /// it, so each moves in half grid steps, to sit in the middle of what it's over.
    fn move_step(child: Child<'_, Self>) -> f64 {
        let kind = child.kind();
        if matches!(kind, ConceptualKind::Actor | ConceptualKind::File) || typed_component(child) {
            GRID / 2.0
        } else {
            GRID
        }
    }

    /// A component from the Database or Storage entry starts as that type.
    fn created(
        eng: &mut Engine<Self>,
        kind: &'static KindInfo<ConceptualKind>,
        placement: Placement,
    ) -> Created<Self> {
        let mut created = eng.base_created(kind, placement);
        if let (Some(component_type), Some(first)) =
            (entry_component_type(kind), created.added.first_mut())
        {
            first.props.component_type = Some(component_type);
        }
        created
    }

    /// A figure, such as a database, can't hold elements.
    fn holds_elements(element: &El<Self>, info: &'static KindInfo<ConceptualKind>) -> bool {
        !info.can_contain.is_empty() && figure_of(element).is_none()
    }

    /// Such as "Actor (bot)", "Actor (group of people)", "Component (database)", or
    /// "Application (cloud hosted)".
    fn kind_description(_eng: &Engine<Self>, element: &Laid<Self>) -> String {
        let variant = if is_cloud(&element.el) {
            Some("cloud hosted")
        } else if element.el.kind.can_be_group() && element.el.props.group {
            Some(if element.el.props.agent {
                "group of bots"
            } else {
                "group of people"
            })
        } else {
            match figure_of(&element.el) {
                Some(Figure::Bot) => Some("bot"),
                Some(Figure::Database) => Some("database"),
                Some(Figure::Storage) => Some("storage"),
                // A person or a file is what its kind always is.
                Some(Figure::Person | Figure::File) | None => None,
            }
        };
        match variant {
            Some(v) => format!("{} ({v})", element.info.label),
            None => element.info.label.to_owned(),
        }
    }

    /// The element's arrows.
    fn extra_details(eng: &Engine<Self>, element: &Laid<Self>) -> Vec<String> {
        eng.connections()
            .iter()
            .filter_map(|c| {
                let label = |id: &ElementId| eng.label_of(id).unwrap_or_default().to_owned();
                if c.source_id == *element.id() {
                    Some(format!(
                        "{} to {}{}",
                        arrow_kind(&c.props),
                        label(&c.target_id),
                        saying(&c.props)
                    ))
                } else if c.target_id == *element.id() {
                    Some(format!(
                        "{} from {}{}",
                        arrow_kind(&c.props),
                        label(&c.source_id),
                        saying(&c.props)
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    fn manual_offset(arrow: &ConceptualArrow) -> Option<f64> {
        arrow.manual_offset
    }

    /// A kind `source` connects to, but not a domain it's in, nor what a domain holds, as the
    /// arrow would run inside the domain.
    fn can_connect(eng: &Engine<Self>, source: &Laid<Self>, target: &Laid<Self>) -> bool {
        let holds = |outer: &Laid<Self>, inner: &Laid<Self>| {
            outer.el.kind == ConceptualKind::Domain
                && descendant_ids(eng.elements(), outer.id()).contains(inner.id())
        };
        source.id() != target.id()
            && source.info.connects_to.contains(&target.el.kind)
            && !holds(source, target)
            && !holds(target, source)
    }

    /// People act by hand, so a person's arrows start out manual (dashed). A bot's, an actor
    /// marked as an agent, start out automated, as other elements' do.
    fn new_arrow(eng: &Engine<Self>, source: &ElementId, _target: &ElementId) -> ConceptualArrow {
        ConceptualArrow {
            mode: eng
                .element(source)
                .is_some_and(|e| figure_of(e) == Some(Figure::Person))
                .then_some(ArrowMode::Manual),
            ..ConceptualArrow::default()
        }
    }

    fn arrow_added(eng: &Engine<Self>, connection: &Conn<Self>) -> String {
        let name = eng.arrow_name(&connection.source_id, &connection.target_id);
        if connection.props.is_manual() {
            format!("{name} added, as manual.")
        } else {
            format!("{name} added.")
        }
    }

    /// Announces a change of bot, cloud, or component type. A component that becomes a
    /// database or storage is drawn as a figure, which can't hold elements, so what it held is
    /// put on the canvas.
    fn apply_element(
        eng: &mut Engine<Self>,
        id: &ElementId,
        mut with: El<Self>,
        parent_id: Option<ElementId>,
    ) {
        let Some(selected) = eng.laid(id).cloned() else {
            return;
        };
        let kind = selected.el.kind;
        if !selected.info.has_description {
            with.description.clone_from(&selected.el.description);
        }
        if !kind.can_be_agent() {
            with.props.agent = false;
        }
        if !kind.can_be_group() {
            with.props.group = false;
        }
        if !kind.can_be_cloud_hosted() {
            with.props.cloud_hosted = false;
        }
        if !kind.has_component_type() {
            with.props.component_type = None;
        }
        if !kind.has_name_position() {
            with.props.name_position = None;
        }
        // The kind's own size is the default, so it isn't stored.
        with.props.font_size = with
            .props
            .font_size
            .filter(|s| kind.has_font_size() && *s != kind.default_font_size());
        let label = with.label.clone();
        let mut announcement = None;
        if kind.can_be_agent() && with.props.agent != selected.el.props.agent {
            announcement = Some(if with.props.agent {
                format!("{label} marked as a bot.")
            } else {
                format!("{label} no longer marked as a bot.")
            });
        }
        if kind.can_be_group() && with.props.group != selected.el.props.group {
            announcement = Some(if with.props.group {
                format!("{label} marked as a group.")
            } else {
                format!("{label} no longer marked as a group.")
            });
        }
        if kind.can_be_cloud_hosted() && with.props.cloud_hosted != selected.el.props.cloud_hosted {
            announcement = Some(if with.props.cloud_hosted {
                format!("{label} marked as cloud hosted.")
            } else {
                format!("{label} no longer marked as cloud hosted.")
            });
        }
        if kind.has_name_position() && with.props.name_position != selected.el.props.name_position {
            let at = if with.props.name_at_bottom() {
                "bottom"
            } else {
                "top"
            };
            announcement = Some(format!("{label}'s name moved to the {at}."));
        }
        if kind.has_font_size() && with.props.font_size != selected.el.props.font_size {
            announcement = Some(format!(
                "{label}'s name set to {} pixels.",
                with.props.font_size_of(kind)
            ));
        }
        // A component inside another holds its place only as a database or storage, so one
        // that's neither now is put on the canvas.
        let mut parent_id = parent_id;
        if kind.has_component_type()
            && with.props.component_type.is_none()
            && parent_id
                .as_ref()
                .and_then(|p| eng.element(p))
                .is_some_and(|p| p.kind == ConceptualKind::Component)
        {
            parent_id = None;
        }
        if kind.has_component_type() {
            if with.props.component_type.is_some() && selected.child_count > 0 {
                let elements = eng
                    .elements()
                    .iter()
                    .cloned()
                    .map(|e| {
                        if e.parent_id.as_ref() == Some(id) {
                            e.with_parent(None)
                        } else {
                            e
                        }
                    })
                    .collect();
                eng.set_elements(elements);
            }
            if with.props.component_type != selected.el.props.component_type {
                let type_label = with
                    .props
                    .component_type
                    .map_or("Default", ComponentType::label);
                announcement = Some(format!("{label} set to {}.", type_label.to_lowercase()));
            }
        }
        if let Some(text) = announcement {
            eng.announcement = text;
        }
        eng.apply_changes(id, with, parent_id);
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

    fn validate_arrow(arrow: &ConceptualArrow) -> Result<(), String> {
        validate_length(
            "Text",
            arrow.label.as_deref().unwrap_or_default(),
            ARROW_LABEL_MAX,
        )?;
        validate_length(
            "Connector",
            arrow.connector.as_deref().unwrap_or_default(),
            CONNECTOR_MAX,
        )?;
        validate_length(
            "Transformation",
            arrow.transformation.as_deref().unwrap_or_default(),
            TRANSFORMATION_MAX,
        )
    }
}

/// An arrow's properties with blank text cleared and the rest trimmed, as stored.
pub fn normalized_arrow(arrow: ConceptualArrow) -> ConceptualArrow {
    let clean = |s: Option<String>| s.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
    ConceptualArrow {
        mode: arrow.mode.filter(|m| *m == ArrowMode::Manual),
        label: clean(arrow.label),
        connector: clean(arrow.connector),
        transformation: clean(arrow.transformation),
        ..arrow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NamePosition;
    use hiero_core::geometry::Point;
    use hiero_core::text::EstimateText;

    static ESTIMATE: EstimateText = EstimateText;

    pub(crate) fn engine() -> Engine<Conceptual> {
        let mut n = 0;
        Engine::new(Vec::new(), Vec::new(), DiagramColors::default())
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
    fn a_database_or_storage_goes_into_a_component_but_a_plain_component_does_not() {
        let entry = |label: &str| {
            PALETTE
                .iter()
                .find(|k| k.palette_label == Some(label))
                .unwrap()
        };
        let mut e = engine();
        e.palette_drop(ConceptualKind::Component, Point::new(300.0, 300.0));
        let centre = e.laid(&id("id1")).unwrap().rect.center();
        // A database goes in, as a database, at its figure's size, named after its entry.
        e.palette_drop_entry(entry("Database"), centre);
        let database = e.laid(&id("id2")).unwrap().clone();
        assert_eq!(database.el.parent_id, Some(id("id1")));
        assert_eq!(
            database.el.props.component_type,
            Some(ComponentType::Database)
        );
        assert_eq!(database.el.label, "Database");
        assert_eq!(database.rect.size(), FIGURE_SIZE);
        // Another plain component dropped on it stays on the canvas.
        e.palette_drop(ConceptualKind::Component, centre);
        assert_eq!(e.element(&id("id3")).unwrap().parent_id, None);
        e.add_entry_at_free_position(entry("Storage"));
        let storage = e.element(&id("id4")).unwrap();
        assert_eq!(storage.props.component_type, Some(ComponentType::Storage));
        // The component offered as its container is the one holding it; a plain component is
        // offered none.
        e.select(&id("id2"));
        assert_eq!(
            e.parent_field_label().as_deref(),
            Some("Application or Component")
        );
        e.select(&id("id3"));
        assert_eq!(e.parent_field_label().as_deref(), Some("Application"));
        // Made plain again, as an agent could, it leaves the component it was in.
        let mut with = database.el.clone();
        with.props.component_type = None;
        <Conceptual as DiagramType>::apply_element(&mut e, &id("id2"), with, Some(id("id1")));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, None);
    }

    #[test]
    fn figures_move_in_half_grid_steps_and_boxes_in_whole_ones() {
        use hiero_core::engine::Dir;
        let mut e = engine();
        e.palette_drop(ConceptualKind::Actor, Point::new(400.0, 400.0));
        e.palette_drop(ConceptualKind::Process, Point::new(800.0, 400.0));
        let x = |e: &Engine<Conceptual>, at: &str| e.element(&id(at)).unwrap().x;
        let (actor, process) = (x(&e, "id1"), x(&e, "id2"));
        e.arrow_key(&id("id1"), Dir::Right, false);
        e.arrow_key(&id("id2"), Dir::Right, false);
        assert_eq!(x(&e, "id1") - actor, GRID / 2.0);
        assert_eq!(x(&e, "id2") - process, GRID);
        // Moved together, they keep to the coarser grid.
        e.select(&id("id1"));
        e.toggle_selected(&id("id2"), None);
        let placements = e.group_placements(Point::new(GRID * 0.75, 0.0));
        let moved: Vec<f64> = placements.iter().map(|(_, p)| p.position.x).collect();
        assert_eq!(moved, [actor + GRID / 2.0 + GRID, process + GRID + GRID]);
    }

    #[test]
    fn applications_go_into_domains_and_components_into_applications() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Domain, Point::new(300.0, 140.0));
        e.palette_drop(ConceptualKind::Application, Point::new(200.0, 140.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        e.palette_drop(ConceptualKind::Component, Point::new(200.0, 160.0));
        assert_eq!(e.element(&id("id3")).unwrap().parent_id, Some(id("id2")));
        let app = e.laid(&id("id2")).unwrap();
        assert!(app.text_at_top);
        assert!(!app.is_region);
    }

    #[test]
    fn actors_connect_to_applications_processes_and_actors_but_not_sections() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Actor);
        e.add_at_free_position(ConceptualKind::Application);
        e.add_at_free_position(ConceptualKind::Process);
        e.add_at_free_position(ConceptualKind::Section);
        e.add_at_free_position(ConceptualKind::Actor);
        e.add_at_free_position(ConceptualKind::Job);
        e.add_at_free_position(ConceptualKind::Entity);
        assert!(e.can_connect(&id("id1"), &id("id2")));
        assert!(e.can_connect(&id("id1"), &id("id3")));
        assert!(!e.can_connect(&id("id1"), &id("id4")));
        // A person who hands work to an assistant.
        assert!(e.can_connect(&id("id1"), &id("id5")));
        // A process starts arrows too, and applications, processes, and jobs point to actors,
        // such as a system that sends someone a report.
        assert!(e.can_connect(&id("id3"), &id("id2")));
        assert!(e.can_connect(&id("id2"), &id("id1")));
        assert!(e.can_connect(&id("id3"), &id("id1")));
        assert!(e.can_connect(&id("id6"), &id("id1")));
        // Data and sections don't.
        assert!(!e.can_connect(&id("id7"), &id("id1")));
        assert!(!e.can_connect(&id("id4"), &id("id1")));
    }

    #[test]
    fn systems_point_to_a_domain_but_not_one_they_are_in() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Domain, Point::new(300.0, 140.0));
        e.palette_drop(ConceptualKind::Application, Point::new(200.0, 140.0));
        e.palette_drop(ConceptualKind::Component, Point::new(200.0, 160.0));
        assert_eq!(e.element(&id("id3")).unwrap().parent_id, Some(id("id2")));
        e.add_at_free_position(ConceptualKind::Domain);
        e.add_at_free_position(ConceptualKind::Job);
        // Another domain, from an application, a component, or a job.
        assert!(e.can_connect(&id("id2"), &id("id4")));
        assert!(e.can_connect(&id("id3"), &id("id4")));
        assert!(e.can_connect(&id("id5"), &id("id1")));
        // Not the domain they're in, as the arrow would run inside it.
        assert!(!e.can_connect(&id("id2"), &id("id1")));
        assert!(!e.can_connect(&id("id3"), &id("id1")));
        // A domain starts no arrows.
        assert!(!e.can_connect(&id("id1"), &id("id4")));
        e.select(&id("id2"));
        let targets: Vec<_> = e.arrow_target_options().into_iter().map(|o| o.0).collect();
        assert!(targets.contains(&id("id4")) && !targets.contains(&id("id1")));
    }

    #[test]
    fn a_persons_arrows_start_manual_and_a_bots_automated() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Actor);
        e.add_at_free_position(ConceptualKind::Application);
        e.add_at_free_position(ConceptualKind::Actor);
        let mut bot = e.element(&id("id3")).unwrap().clone();
        bot.props.agent = true;
        <Conceptual as DiagramType>::apply_element(&mut e, &id("id3"), bot, None);
        let person = e.add_connection(&id("id1"), &id("id2")).unwrap();
        assert!(e.connection(&person).unwrap().props.is_manual());
        assert!(e.announcement.ends_with("added, as manual."));
        let from_bot = e.add_connection(&id("id3"), &id("id2")).unwrap();
        assert!(!e.connection(&from_bot).unwrap().props.is_manual());
        let from_app = e.add_connection(&id("id2"), &id("id1")).unwrap();
        assert!(!e.connection(&from_app).unwrap().props.is_manual());
        assert_eq!(e.announcement, "Arrow from Application to Actor added.");
    }

    #[test]
    fn a_component_that_becomes_a_database_lets_go_of_its_children() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Component, Point::new(200.0, 200.0));
        e.palette_drop(ConceptualKind::Entity, Point::new(200.0, 230.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        e.select(&id("id1"));
        let mut with = e.element(&id("id1")).unwrap().clone();
        with.props.component_type = Some(ComponentType::Database);
        <Conceptual as DiagramType>::apply_element(&mut e, &id("id1"), with, None);
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, None);
        assert_eq!(e.laid(&id("id1")).unwrap().rect.size(), FIGURE_SIZE);
        assert_eq!(e.announcement, "Component set to database.");
    }

    #[test]
    fn an_actor_marked_as_a_group_keeps_its_area_and_is_read_as_one() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Actor);
        let before = e.laid(&id("id1")).unwrap().rect;
        let mut with = e.element(&id("id1")).unwrap().clone();
        with.label = "Sales".into();
        with.props.group = true;
        <Conceptual as DiagramType>::apply_element(&mut e, &id("id1"), with, None);
        assert_eq!(e.announcement, "Sales marked as a group.");
        let actor = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(actor.rect, before);
        assert_eq!(
            Conceptual::kind_description(&e, &actor),
            "Actor (group of people)"
        );
        // Stored as group: true, and left out for one.
        let json = serde_json::to_value(&actor.el.props).unwrap();
        assert_eq!(json, serde_json::json!({"group": true}));
        assert_eq!(
            serde_json::to_value(ConceptualProps::default()).unwrap(),
            serde_json::json!({})
        );
        let mut with = actor.el.clone();
        with.props.agent = true;
        <Conceptual as DiagramType>::apply_element(&mut e, &id("id1"), with, None);
        let actor = e.laid(&id("id1")).unwrap().clone();
        assert_eq!(
            Conceptual::kind_description(&e, &actor),
            "Actor (group of bots)"
        );
        // Only actors can be a group.
        e.add_at_free_position(ConceptualKind::Application);
        let mut with = e.element(&id("id2")).unwrap().clone();
        with.props.group = true;
        <Conceptual as DiagramType>::apply_element(&mut e, &id("id2"), with, None);
        assert!(!e.element(&id("id2")).unwrap().props.group);
    }

    #[test]
    fn a_file_goes_into_an_application_or_component_as_an_entity_does() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Application, Point::new(200.0, 140.0));
        e.palette_drop(ConceptualKind::File, Point::new(200.0, 160.0));
        assert_eq!(e.element(&id("id2")).unwrap().parent_id, Some(id("id1")));
        e.palette_drop(ConceptualKind::Component, Point::new(800.0, 400.0));
        e.palette_drop(ConceptualKind::File, Point::new(800.0, 400.0));
        assert_eq!(e.element(&id("id4")).unwrap().parent_id, Some(id("id3")));
        let file = e.laid(&id("id4")).unwrap().clone();
        assert_eq!(file.rect.size(), FIGURE_SIZE);
        // Read as a file, not "File (file)", and takes arrows but doesn't start them.
        assert_eq!(Conceptual::kind_description(&e, &file), "File");
        assert!(!e.can_connect(&id("id4"), &id("id1")));
        assert!(e.can_connect(&id("id1"), &id("id4")));
    }

    #[test]
    fn hidden_details_describe_arrows() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Application);
        e.add_at_free_position(ConceptualKind::Application);
        e.add_connection(&id("id1"), &id("id2"));
        let mut conn = e.connections()[0].props.clone();
        conn.connector = Some("API".into());
        conn.mode = Some(ArrowMode::Manual);
        conn.payload = crate::model::ArrowPayload::Email;
        let cid = e.connections()[0].id.clone();
        e.set_connection_props(&cid, conn.clone());
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert!(
            e.accessible_label(&laid)
                .contains("manual arrow to Application via API carrying an e-mail")
        );
        assert_eq!(
            arrow_title(&conn, true, "CRM"),
            "To CRM via API (manual, carries an e-mail)"
        );
    }

    #[test]
    fn validation_limits_lengths() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Application);
        let mut el = e.elements()[0].clone();
        el.description = Some("x".repeat(501));
        assert!(Conceptual::validate_element(&el).is_err());
        let arrow = ConceptualArrow {
            connector: Some("x".repeat(21)),
            ..ConceptualArrow::default()
        };
        assert_eq!(
            Conceptual::validate_arrow(&arrow).unwrap_err(),
            "Connector must be 20 characters or fewer."
        );
    }

    fn change(e: &mut Engine<Conceptual>, of: &str, change: impl FnOnce(&mut El<Conceptual>)) {
        let mut with = e.element(&id(of)).unwrap().clone();
        change(&mut with);
        let parent = with.parent_id.clone();
        <Conceptual as DiagramType>::apply_element(e, &id(of), with, parent);
    }

    #[test]
    fn a_domains_name_band_grows_for_a_larger_name_and_can_move_to_the_bottom() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Domain, Point::new(300.0, 200.0));
        let domain = e.laid(&id("id1")).unwrap();
        assert_eq!((domain.header, domain.footer), (40.0, 0.0));
        change(&mut e, "id1", |w| w.props.font_size = Some(32));
        assert_eq!(e.laid(&id("id1")).unwrap().header, 60.0);
        assert_eq!(e.announcement, "Domain's name set to 32 pixels.");
        change(&mut e, "id1", |w| {
            w.props.name_position = Some(NamePosition::Bottom);
        });
        let domain = e.laid(&id("id1")).unwrap();
        // The usual gap above what it holds, and the name's band below it.
        assert_eq!((domain.header, domain.footer), (20.0, 40.0));
        assert_eq!(e.announcement, "Domain's name moved to the bottom.");
    }

    #[test]
    fn a_domains_name_band_grows_for_each_line_of_its_description() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Domain, Point::new(300.0, 200.0));
        let band = |e: &Engine<Conceptual>| {
            let domain = e.laid(&id("id1")).unwrap();
            (domain.header, domain.footer)
        };
        change(&mut e, "id1", |w| w.description = Some("Orders".into()));
        assert_eq!(
            e.element(&id("id1")).unwrap().description.as_deref(),
            Some("Orders")
        );
        assert_eq!(band(&e), (60.0, 0.0));
        change(&mut e, "id1", |w| {
            w.description = Some("Orders\nBilling\nSupport".into());
        });
        assert_eq!(band(&e), (100.0, 0.0));
        // A long one wraps across the 480 px domain to three lines in a smaller font, which
        // takes less room than three of 13 px.
        change(&mut e, "id1", |w| w.description = Some("word ".repeat(60)));
        assert_eq!(band(&e), (80.0, 0.0));
        // At the smallest size it takes more lines, so nothing is cut: five 10 px lines of 8 px.
        change(&mut e, "id1", |w| w.description = Some("word ".repeat(100)));
        let fitted = container_description(
            e.element(&id("id1")).unwrap().description.as_deref(),
            480.0,
            e.measure(),
        );
        assert_eq!((fitted.lines.len(), fitted.style.size), (5, 8.0));
        assert_eq!(band(&e), (100.0, 0.0));
        change(&mut e, "id1", |w| {
            w.description = Some("Orders".into());
            w.props.font_size = Some(32);
        });
        assert_eq!(band(&e), (80.0, 0.0));
        change(&mut e, "id1", |w| {
            w.props.font_size = None;
            w.props.name_position = Some(NamePosition::Bottom);
        });
        assert_eq!(band(&e), (20.0, 40.0));
    }

    #[test]
    fn what_a_domain_holds_moves_down_when_its_band_grows() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Domain, Point::new(300.0, 200.0));
        let domain = e.laid(&id("id1")).unwrap().rect;
        e.palette_drop(
            ConceptualKind::Application,
            Point::new(domain.x + 120.0, domain.y + 80.0),
        );
        let app = e.laid(&id("id2")).unwrap().clone();
        assert_eq!(app.el.parent_id, Some(id("id1")));
        let below = |e: &Engine<Conceptual>| {
            let domain = e.laid(&id("id1")).unwrap();
            let app = e.laid(&id("id2")).unwrap();
            app.rect.y - (domain.rect.y + domain.header)
        };
        assert!(below(&e) >= 0.0);
        change(&mut e, "id1", |w| {
            w.description = Some("Orders\nBilling\nSupport".into());
        });
        let moved = e.laid(&id("id2")).unwrap().rect;
        assert!(below(&e) >= 0.0, "{}", below(&e));
        assert!(moved.y > app.rect.y);
        assert_eq!(moved.x, app.rect.x);
        // The domain grows to hold it.
        let domain = e.laid(&id("id1")).unwrap().rect;
        assert!(domain.bottom() >= moved.bottom());
    }

    #[test]
    fn a_name_at_its_kinds_size_is_left_out() {
        let mut e = engine();
        e.palette_drop(ConceptualKind::Domain, Point::new(300.0, 200.0));
        e.add_at_free_position(ConceptualKind::Actor);
        change(&mut e, "id1", |w| w.props.font_size = Some(20));
        assert_eq!(e.element(&id("id1")).unwrap().props.font_size, None);
        // An actor's name keeps its usual size, and its name sits where it always does.
        change(&mut e, "id2", |w| {
            w.props.font_size = Some(24);
            w.props.name_position = Some(NamePosition::Bottom);
        });
        let actor = &e.element(&id("id2")).unwrap().props;
        assert_eq!((actor.font_size, actor.name_position), (None, None));
    }

    #[test]
    fn an_applications_header_grows_for_a_larger_name() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Application);
        assert_eq!(e.laid(&id("id1")).unwrap().header, 60.0);
        change(&mut e, "id1", |w| w.props.font_size = Some(24));
        // A 36 px line rather than 24: 20 more, on the grid.
        assert_eq!(e.laid(&id("id1")).unwrap().header, 80.0);
        change(&mut e, "id1", |w| w.props.font_size = Some(12));
        assert_eq!(e.laid(&id("id1")).unwrap().header, 60.0);
    }

    #[test]
    fn processes_jobs_and_entities_resize_down_to_their_starting_height() {
        let mut e = engine();
        for kind in [
            ConceptualKind::Process,
            ConceptualKind::Job,
            ConceptualKind::Entity,
        ] {
            e.add_at_free_position(kind);
        }
        for of in ["id1", "id2", "id3"] {
            let laid = e.laid(&id(of)).unwrap().clone();
            assert_eq!(laid.info.resizable, hiero_core::model::Resizable::Both);
            let limits = Conceptual::resize_limits(&e, &laid);
            assert_eq!(limits.min_size, Size::new(120.0, 40.0));
        }
    }

    #[test]
    fn font_sizes_are_whole_pixels_in_range() {
        let mut with = engine_element(ConceptualKind::Process);
        with.props.font_size = Some(60);
        assert_eq!(
            Conceptual::validate_element(&with).unwrap_err(),
            "Font size must be from 8 to 48."
        );
        with.props.font_size = Some(8);
        assert!(Conceptual::validate_element(&with).is_ok());
    }

    fn engine_element(kind: ConceptualKind) -> El<Conceptual> {
        let mut e = engine();
        e.add_at_free_position(kind);
        e.element(&id("id1")).unwrap().clone()
    }
}
