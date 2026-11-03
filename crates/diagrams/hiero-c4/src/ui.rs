//! The C4 types' properties panels and their place in the shared editor.

use egui::Ui;
use hiero_core::engine::{Conn, Engine, Laid, validate_length};
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::{self, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::model::{
    ARROW_DESCRIPTION_MAX, ARROW_TECHNOLOGY_MAX, C4Component, C4Container, C4Context, C4Type,
    Color, DESCRIPTION_MAX, TECHNOLOGY_MAX, TYPE_NAME_MAX,
};
use crate::rules::{arrow_title, normalized_arrow};
use crate::scene::{drawn_arrows, scene_of};

fn arrow_lines<T: C4Type>(eng: &Engine<T>, layout: &[Laid<T>]) -> Vec<ArrowLine> {
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

fn element_properties<T: C4Type + DiagramView>(
    ui: &mut Ui,
    cx: &mut PropsCx<'_, T>,
    element: &Laid<T>,
) {
    let mut with = element.el.clone();
    let role = T::role(element.el.kind);
    let mut field = None;
    if cx.name_field(ui, element, &mut with) {
        field = Some("label");
    }
    // Blank shows the usual type, such as [Person], which the label names.
    if role.has_custom_type()
        && let Some(type_name) = text_field(
            ui,
            cx.field_id(element.id(), "type"),
            &format!("Type (blank for {})", role.type_name().unwrap_or_default()),
            element.el.props.type_name.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Type", t, TYPE_NAME_MAX),
        )
    {
        with.props.type_name = Some(type_name).filter(|t| !t.trim().is_empty());
        field = Some("type");
    }
    if role.has_technology()
        && let Some(technology) = text_field(
            ui,
            cx.field_id(element.id(), "technology"),
            "Technology",
            element.el.props.technology.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Technology", t, TECHNOLOGY_MAX),
        )
    {
        with.props.technology = Some(technology).filter(|t| !t.trim().is_empty());
        field = Some("technology");
    }
    if element.info.has_description
        && let Some(description) = text_field(
            ui,
            cx.field_id(element.id(), "description"),
            "Description",
            element.el.description.as_deref().unwrap_or_default(),
            true,
            |t| validate_length("Description", t, DESCRIPTION_MAX),
        )
    {
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

fn arrow_properties<T: C4Type + DiagramView>(
    ui: &mut Ui,
    cx: &mut PropsCx<'_, T>,
    connection: &Conn<T>,
) {
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
    if T::ARROW_TECHNOLOGY
        && let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "technology"),
            "Technology",
            current.technology.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Technology", t, ARROW_TECHNOLOGY_MAX),
        )
    {
        arrow.technology = Some(v);
        field = Some("technology");
    }
    if let Some(field) = field {
        let arrow = normalized_arrow::<T>(arrow);
        if T::validate_arrow(&arrow).is_ok() && arrow != current {
            let target = id.clone();
            cx.apply(Some(format!("{id}:{field}")), move |eng| {
                eng.set_connection_props(&target, arrow);
            });
        }
    }
}

/// The instructions shown above the canvas, with what goes in a boundary on this type.
macro_rules! help {
    ($dropping:expr, $resizing:expr) => {
        concat!(
            "• Drag an element onto the canvas, or select it to add it. ",
            $dropping,
            "\n",
            "• Drag an element, or use the arrow keys, to move it. ",
            $resizing,
            "\n",
            "• Press Enter to edit an element's properties and Delete to remove it. Hold Shift, Ctrl, or Cmd as you click, or press Shift+Space on each, to select several and move or remove them together.\n",
            "• Drag from the round handle on the selected element, or use Add arrow in its properties, to draw a relationship. Select a relationship to describe it.\n",
            "• Cut, copy, and paste elements with Cmd or Ctrl and X, C, and V, with what they hold and the relationships between them.\n",
            "• Drag an empty part of the canvas to draw a box around elements and select them, holding Shift, Ctrl, or Cmd to add them to the selection.\n",
            "• Drag with the right mouse button, or scroll with a trackpad or with Shift and the scroll wheel, to move around. Use the scroll wheel, a pinch on a trackpad, or the Zoom slider in the canvas's corner to zoom."
        )
    };
}

/// Implements `DiagramView` for a C4 type with the shared panels.
macro_rules! c4_view {
    ($ty:ty, $help:expr) => {
        impl DiagramView for $ty {
            const HELP: &'static str = $help;

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
                arrow_lines(eng, layout)
            }

            fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
                element_properties(ui, cx, element);
            }

            fn arrow_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, connection: &Conn<Self>) {
                arrow_properties(ui, cx, connection);
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
    };
}

c4_view!(
    C4Context,
    help!(
        "Choose an element's colour in its properties: by C4's custom, blue for the system in scope, orange for another of your own, and red for one outside.",
        "Resize software systems with their handles."
    )
);
c4_view!(
    C4Container,
    help!(
        "Drop websites, static content, backends, services, queues, topics, stores, and databases into the system boundary.",
        "Resize elements and the system boundary with their handles."
    )
);
c4_view!(
    C4Component,
    help!(
        "Drop a container boundary, UI containers, databases, and stores into the system boundary, and components into the container boundary.",
        "Resize elements and boundaries with their handles."
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_has_help() {
        for help in [C4Context::HELP, C4Container::HELP, C4Component::HELP] {
            assert_eq!(help.lines().count(), 7);
        }
        assert!(C4Container::HELP.contains("into the system boundary"));
    }
}
