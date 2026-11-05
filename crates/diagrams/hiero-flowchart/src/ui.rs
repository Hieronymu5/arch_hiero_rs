//! The flowchart's properties panel and its place in the shared editor.

use egui::Ui;
use hiero_core::engine::{Conn, DiagramType, Engine, Laid, validate_label, validate_length};
use hiero_core::geometry::Point;
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::{self, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::model::{CONNECTOR_NAME_MAX, Flowchart, FlowchartKind, LABEL_MAX, Orientation};
use crate::rules::{arrow_title, normalized_arrow};
use crate::scene::{drawn_arrows, scene_of};

impl DiagramView for Flowchart {
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
        match element.el.kind.shape() {
            Some(shape) => shape.link_mark_at(element.rect),
            None => hiero_core::look::link_mark_corner(element.rect),
        }
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let mut field = None;
        if element.el.kind == FlowchartKind::Connector {
            // A connector's name is short: a letter or number its other end shares.
            if let Some(label) = text_field(
                ui,
                cx.field_id(element.id(), "label"),
                "Name (matches its other end)",
                &element.el.label,
                false,
                |t| {
                    validate_label(t)?;
                    validate_length("A connector's name", t, CONNECTOR_NAME_MAX)
                },
            ) {
                with.label = label;
                field = Some("label");
            }
        } else if cx.name_field(ui, element, &mut with) {
            field = Some("label");
        }
        if element.el.kind == FlowchartKind::Pool {
            widgets::label(ui, "Lanes run", false);
            ui.horizontal_wrapped(|ui| {
                for value in Orientation::CHOICES {
                    if ui
                        .radio_value(
                            &mut with.props.orientation,
                            value,
                            Orientation::label(value),
                        )
                        .changed()
                    {
                        field = Some("orientation");
                    }
                }
            });
        }
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
        if let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "label"),
            "Label",
            current.label.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Label", t, LABEL_MAX),
        ) {
            let mut arrow = current.clone();
            arrow.label = Some(v);
            let arrow = normalized_arrow(arrow);
            if Flowchart::validate_arrow(&arrow).is_ok() && arrow != current {
                let target = id.clone();
                cx.apply(Some(format!("{id}:label")), move |eng| {
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
    fn the_help_says_how_to_draw_the_flow() {
        assert_eq!(Flowchart::HELP.lines().count(), 8);
        assert!(Flowchart::HELP.contains("Pool"));
        assert!(Flowchart::HELP.contains("Yes"));
    }
}
