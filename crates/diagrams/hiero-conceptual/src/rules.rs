//! The conceptual diagram's rules: figures for actors and some components, and what screen
//! readers hear about arrows.

use hiero_core::engine::{DiagramType, El, Engine, Laid, validate_label, validate_length};
use hiero_core::figures::component::FIGURE_SIZE;
use hiero_core::geometry::Size;
use hiero_core::look::DiagramColors;
use hiero_core::model::{DiagramTypeId, ElementId, KindInfo};

use crate::model::{
    ARROW_LABEL_MAX, CONNECTOR_MAX, ComponentType, Conceptual, ConceptualArrow, ConceptualKind,
    ConceptualProps, DESCRIPTION_MAX, PALETTE, TRANSFORMATION_MAX,
};

/// A figure drawn above an element's name instead of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    Person,
    Bot,
    Database,
    Storage,
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

/// Such as " via API with transformation Map fields labelled Orders".
fn saying(arrow: &ConceptualArrow) -> String {
    let mut out = String::new();
    if let Some(c) = &arrow.connector {
        out.push_str(&format!(" via {c}"));
    }
    if let Some(t) = &arrow.transformation {
        out.push_str(&format!(" with transformation {t}"));
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
        if arrow.bidirectional { "bi-directional " } else { "" }
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

    /// A figure, such as a database, can't hold elements.
    fn holds_elements(element: &El<Self>, info: &'static KindInfo<ConceptualKind>) -> bool {
        !info.can_contain.is_empty() && figure_of(element).is_none()
    }

    /// Such as "Actor (bot)", "Component (database)", or "Application (cloud hosted)".
    fn kind_description(_eng: &Engine<Self>, element: &Laid<Self>) -> String {
        let variant = if is_cloud(&element.el) {
            Some("cloud hosted")
        } else {
            match figure_of(&element.el) {
                Some(Figure::Bot) => Some("bot"),
                Some(Figure::Database) => Some("database"),
                Some(Figure::Storage) => Some("storage"),
                Some(Figure::Person) | None => None,
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
                    Some(format!("{} to {}{}", arrow_kind(&c.props), label(&c.target_id), saying(&c.props)))
                } else if c.target_id == *element.id() {
                    Some(format!("{} from {}{}", arrow_kind(&c.props), label(&c.source_id), saying(&c.props)))
                } else {
                    None
                }
            })
            .collect()
    }

    fn manual_offset(arrow: &ConceptualArrow) -> Option<f64> {
        arrow.manual_offset
    }

    /// Announces a change of bot, cloud, or component type. A component that becomes a
    /// database or storage is drawn as a figure, which can't hold elements, so what it held is
    /// put on the canvas.
    fn apply_element(eng: &mut Engine<Self>, id: &ElementId, mut with: El<Self>, parent_id: Option<ElementId>) {
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
        if !kind.can_be_cloud_hosted() {
            with.props.cloud_hosted = false;
        }
        if !kind.has_component_type() {
            with.props.component_type = None;
        }
        let label = with.label.clone();
        let mut announcement = None;
        if kind.can_be_agent() && with.props.agent != selected.el.props.agent {
            announcement = Some(if with.props.agent {
                format!("{label} marked as a bot.")
            } else {
                format!("{label} no longer marked as a bot.")
            });
        }
        if kind.can_be_cloud_hosted() && with.props.cloud_hosted != selected.el.props.cloud_hosted {
            announcement = Some(if with.props.cloud_hosted {
                format!("{label} marked as cloud hosted.")
            } else {
                format!("{label} no longer marked as cloud hosted.")
            });
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
        )
    }

    fn validate_arrow(arrow: &ConceptualArrow) -> Result<(), String> {
        validate_length("Text", arrow.label.as_deref().unwrap_or_default(), ARROW_LABEL_MAX)?;
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
        mode: arrow.mode.filter(|m| *m == crate::model::ArrowMode::Manual),
        label: clean(arrow.label),
        connector: clean(arrow.connector),
        transformation: clean(arrow.transformation),
        ..arrow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn actors_connect_to_applications_but_not_processes() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Actor);
        e.add_at_free_position(ConceptualKind::Application);
        e.add_at_free_position(ConceptualKind::Process);
        assert!(e.can_connect(&id("id1"), &id("id2")));
        assert!(!e.can_connect(&id("id1"), &id("id3")));
        assert!(!e.can_connect(&id("id2"), &id("id1")));
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
    fn hidden_details_describe_arrows() {
        let mut e = engine();
        e.add_at_free_position(ConceptualKind::Application);
        e.add_at_free_position(ConceptualKind::Application);
        e.add_connection(&id("id1"), &id("id2"));
        let mut conn = e.connections()[0].props.clone();
        conn.connector = Some("API".into());
        conn.mode = Some(crate::model::ArrowMode::Manual);
        let cid = e.connections()[0].id.clone();
        e.set_connection_props(&cid, conn);
        let laid = e.laid(&id("id1")).unwrap().clone();
        assert!(e.accessible_label(&laid).contains("manual arrow to Application via API"));
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
}
