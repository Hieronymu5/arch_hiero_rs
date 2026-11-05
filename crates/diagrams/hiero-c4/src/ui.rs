//! The C4 type's properties panel and its place in the shared editor.

use egui::Ui;
use hiero_core::engine::{Conn, DiagramType, Engine, Laid, validate_length};
use hiero_core::geometry::Point;
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::{self, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::model::{
    ARROW_DESCRIPTION_MAX, ARROW_TECHNOLOGY_MAX, BoundaryType, C4, C4Kind, Color, ContainerType,
    DESCRIPTION_MAX, TECHNOLOGY_MAX, TYPE_NAME_MAX,
};
use crate::rules::{arrow_title, normalized_arrow, shape_of};
use crate::scene::{drawn_arrows, scene_of};

impl DiagramView for C4 {
    const HELP: &'static str = crate::HELP;

    fn scene(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        _slide: Option<(&ConnectionId, f64)>,
    ) -> Scene {
        scene_of(eng, layout)
    }

    fn arrow_lines(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        _slide: Option<(&ConnectionId, f64)>,
    ) -> Vec<ArrowLine> {
        drawn_arrows(eng, layout)
            .into_iter()
            .map(|a| ArrowLine {
                id: a.id,
                from: a.look.from,
                to: a.look.to,
                boxes: Vec::new(),
            })
            .collect()
    }

    fn link_mark_at(_eng: &Engine<Self>, element: &Laid<Self>) -> Point {
        shape_of(&element.el).link_mark_at(element.rect)
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let kind = element.el.kind;
        let props = &element.el.props;
        let mut field = None;
        if cx.name_field(ui, element, &mut with) {
            field = Some("label");
        }
        if kind == C4Kind::Container {
            let options: Vec<(ContainerType, String)> = ContainerType::CHOICES
                .into_iter()
                .flatten()
                .map(|t| (t, ContainerType::label(Some(t)).to_owned()))
                .collect();
            if let Some(chosen) = widgets::select(
                ui,
                cx.field_id(element.id(), "container-type"),
                "Container type",
                &props.container_type,
                Some(ContainerType::label(None)),
                &options,
            ) {
                with.props.container_type = chosen;
                field = Some("container-type");
            }
        }
        if kind == C4Kind::Boundary {
            widgets::label(ui, "Boundary type", false);
            ui.horizontal_wrapped(|ui| {
                for value in BoundaryType::CHOICES {
                    if ui
                        .radio_value(
                            &mut with.props.boundary_type,
                            value,
                            BoundaryType::label(value),
                        )
                        .changed()
                    {
                        field = Some("boundary-type");
                    }
                }
            });
        }
        // Blank shows the usual type, which the label names.
        if kind.has_custom_type()
            && let Some(type_name) = text_field(
                ui,
                cx.field_id(element.id(), "type"),
                if kind == C4Kind::Person {
                    "Type (blank for Person)"
                } else {
                    "Type (blank for Software System)"
                },
                props.type_name.as_deref().unwrap_or_default(),
                false,
                |t| validate_length("Type", t, TYPE_NAME_MAX),
            )
        {
            with.props.type_name = Some(type_name).filter(|t| !t.trim().is_empty());
            field = Some("type");
        }
        if kind.has_technology()
            && let Some(technology) = text_field(
                ui,
                cx.field_id(element.id(), "technology"),
                "Technology",
                props.technology.as_deref().unwrap_or_default(),
                false,
                |t| validate_length("Technology", t, TECHNOLOGY_MAX),
            )
        {
            with.props.technology = Some(technology).filter(|t| !t.trim().is_empty());
            field = Some("technology");
        }
        if let Some(description) = text_field(
            ui,
            cx.field_id(element.id(), "description"),
            "Description",
            element.el.description.as_deref().unwrap_or_default(),
            true,
            |t| validate_length("Description", t, DESCRIPTION_MAX),
        ) {
            with.description = Some(description).filter(|d| !d.trim().is_empty());
            field = Some("description");
        }
        widgets::label(ui, "Colour", false);
        ui.horizontal_wrapped(|ui| {
            for value in Color::CHOICES {
                if ui
                    .radio_value(&mut with.props.color, value, Color::label(value))
                    .changed()
                {
                    field = Some("color");
                }
            }
        });
        let parent = cx.parent_field(ui, element, false);
        if parent.is_some() {
            field = Some("parent");
        }
        if let Some(field) = field {
            cx.commit(element, with, parent, field);
        }
    }

    fn arrow_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, connection: &Conn<Self>) {
        let id = connection.id.clone();
        let current = connection.props.clone();
        let mut arrow = current.clone();
        let mut field: Option<&'static str> = None;
        if let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "description"),
            "Description",
            current.description.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Description", t, ARROW_DESCRIPTION_MAX),
        ) {
            arrow.description = Some(v);
            field = Some("description");
        }
        if let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "technology"),
            "Technology",
            current.technology.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Technology", t, ARROW_TECHNOLOGY_MAX),
        ) {
            arrow.technology = Some(v);
            field = Some("technology");
        }
        if let Some(field) = field {
            let arrow = normalized_arrow(arrow);
            if C4::validate_arrow(&arrow).is_ok() && arrow != current {
                let target = id.clone();
                cx.apply(Some(format!("{id}:{field}")), move |eng| {
                    eng.set_connection_props(&target, arrow);
                });
            }
        }
    }

    fn arrow_title(
        _eng: &Engine<Self>,
        connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        arrow_title(&connection.props, outgoing, other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_says_how_to_choose_types() {
        assert_eq!(C4::HELP.lines().count(), 7);
        assert!(C4::HELP.contains("Container type"));
        assert!(C4::HELP.contains("Edit Details"));
    }
}
