//! The conceptual diagram's properties panel and its place in the shared editor.

use egui::Ui;
use hiero_core::engine::{Conn, DiagramType, Dir, Engine, Laid, validate_length};
use hiero_core::geometry::Point;
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::text_field;
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::arrows::{self, offset_at, place_automatically, set_manual_offset, straighten};
use crate::model::{
    ARROW_LABEL_MAX, ArrowMode, CONNECTOR_MAX, ComponentType, Conceptual, ConceptualArrow,
    DESCRIPTION_MAX, TRANSFORMATION_MAX,
};
use crate::rules::{arrow_title, normalized_arrow};
use crate::scene::{drawn_arrows, scene_of};

impl DiagramView for Conceptual {
    const HELP: &'static str = crate::HELP;

    fn scene(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        slide: Option<(&ConnectionId, f64)>,
    ) -> Scene {
        scene_of(eng, layout, slide)
    }

    fn arrow_lines(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        slide: Option<(&ConnectionId, f64)>,
    ) -> Vec<ArrowLine> {
        drawn_arrows(eng, layout, slide)
            .into_iter()
            .map(|a| ArrowLine {
                id: a.id,
                from: a.look.from,
                to: a.look.to,
                boxes: a
                    .look
                    .connector
                    .iter()
                    .chain(a.look.transformation.iter())
                    .map(|(_, r)| *r)
                    .collect(),
            })
            .collect()
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let kind = element.el.kind;
        let mut field = None;
        if cx.name_field(ui, element, &mut with) {
            field = Some("label");
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
            with.description = Some(description);
            field = Some("description");
        }
        if kind.can_be_agent() && ui.checkbox(&mut with.props.agent, "Bot").changed() {
            field = Some("agent");
        }
        if kind.can_be_cloud_hosted()
            && ui
                .checkbox(&mut with.props.cloud_hosted, "Cloud hosted")
                .changed()
        {
            field = Some("cloud");
        }
        if kind.has_component_type() {
            ui.label("Type");
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [
                    (None, "Default"),
                    (Some(ComponentType::Database), "Database"),
                    (Some(ComponentType::Storage), "Storage"),
                ] {
                    if ui
                        .radio_value(&mut with.props.component_type, value, label)
                        .changed()
                    {
                        field = Some("component-type");
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
        let mut arrow = current.clone();
        let mut field: Option<&'static str> = None;
        let text =
            |ui: &mut Ui, name: &'static str, label: &str, value: &Option<String>, max: usize| {
                text_field(
                    ui,
                    cx.arrow_field_id(&id, name),
                    label,
                    value.as_deref().unwrap_or_default(),
                    false,
                    |t| validate_length(label, t, max),
                )
            };
        if let Some(v) = text(ui, "label", "Text", &current.label, ARROW_LABEL_MAX) {
            arrow.label = Some(v);
            field = Some("label");
        }
        ui.label("Type");
        let mut manual = current.is_manual();
        ui.horizontal(|ui| {
            if ui.radio_value(&mut manual, false, "Automated").changed()
                || ui.radio_value(&mut manual, true, "Manual").changed()
            {
                field = Some("mode");
            }
        });
        arrow.mode = manual.then_some(ArrowMode::Manual);
        if ui
            .checkbox(&mut arrow.bidirectional, "Bi-directional")
            .changed()
        {
            field = Some("bidirectional");
        }
        if let Some(v) = text(
            ui,
            "connector",
            "Connector",
            &current.connector,
            CONNECTOR_MAX,
        ) {
            arrow.connector = Some(v);
            field = Some("connector");
        }
        if let Some(v) = text(
            ui,
            "transformation",
            "Transformation",
            &current.transformation,
            TRANSFORMATION_MAX,
        ) {
            arrow.transformation = Some(v);
            field = Some("transformation");
        }
        if let Some(field) = field {
            let arrow = normalized_arrow(arrow);
            if Conceptual::validate_arrow(&arrow).is_ok() && arrow != current {
                let mode_changed = arrow.is_manual() != current.is_manual();
                let direction_changed = arrow.bidirectional != current.bidirectional;
                let (source, target) = (connection.source_id.clone(), connection.target_id.clone());
                let target_id = id.clone();
                cx.apply(Some(format!("{id}:{field}")), move |eng| {
                    eng.set_connection_props(&target_id, arrow.clone());
                    let name = eng.arrow_name(&source, &target);
                    if mode_changed {
                        let mode = if arrow.is_manual() {
                            "manual"
                        } else {
                            "automated"
                        };
                        eng.announcement = format!("{name} set to {mode}.");
                    } else if direction_changed {
                        let way = if arrow.bidirectional {
                            "bi-directional"
                        } else {
                            "one way"
                        };
                        eng.announcement = format!("{name} set to {way}.");
                    }
                });
            }
        }
        ui.add_space(6.0);
        let straightenable = arrows::range_for(cx.eng, &id).is_some();
        ui.horizontal(|ui| {
            if straightenable && ui.button("Straighten").clicked() {
                let id = id.clone();
                cx.apply(None, move |eng| straighten(eng, &id));
            }
            if current.manual_offset.is_some() && ui.button("Place automatically").clicked() {
                let id = id.clone();
                cx.apply(None, move |eng| place_automatically(eng, &id));
            }
        });
    }

    fn arrow_title(
        _eng: &Engine<Self>,
        connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        arrow_title(&connection.props, outgoing, other)
    }

    fn slide_offset(eng: &Engine<Self>, id: &ConnectionId, pointer: Point) -> Option<f64> {
        offset_at(eng, id, pointer)
    }

    fn commit_slide(eng: &mut Engine<Self>, id: &ConnectionId, offset: f64) {
        set_manual_offset(eng, id, Some(offset), "moved");
    }

    fn connection_key(eng: &mut Engine<Self>, dir: Dir, shift: bool) -> bool {
        arrows::arrow_key(eng, dir, shift)
    }
}

/// Keeps the arrow type in scope for callers that build arrows by hand.
pub type Arrow = ConceptualArrow;
