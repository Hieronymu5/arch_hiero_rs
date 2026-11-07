//! The high level conceptual type's properties panel and its place in the shared editor.

use egui::Ui;
use hiero_core::engine::{Conn, Engine, Laid, validate_length};
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::widgets::{self, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::model::{BlockArrow, Color, DESCRIPTION_MAX, FONT_SIZES, HighLevel, HighLevelKind};
use crate::scene::{drawn_arrows, scene_of};

impl DiagramView for HighLevel {
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
                from: a.from,
                to: a.to,
                boxes: Vec::new(),
            })
            .collect()
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let kind = element.el.kind;
        let props = &element.el.props;
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
            with.description = Some(description).filter(|d| !d.trim().is_empty());
            field = Some("description");
        }
        widgets::label(ui, "Colour", false);
        let mut color = props.color_of(kind);
        ui.horizontal_wrapped(|ui| {
            for value in Color::CHOICES {
                if ui.radio_value(&mut color, value, value.label()).changed() {
                    with.props.color = Some(color);
                    field = Some("color");
                }
            }
        });
        // The usual sizes, and this element's own when it's another, such as an agent set.
        let current = props.font_size_of(kind);
        let mut sizes: Vec<u32> = FONT_SIZES.to_vec();
        if !sizes.contains(&current) {
            sizes.push(current);
            sizes.sort_unstable();
        }
        let options: Vec<(u32, String)> = sizes.into_iter().map(|s| (s, s.to_string())).collect();
        if let Some(Some(size)) = widgets::select(
            ui,
            cx.field_id(element.id(), "font-size"),
            "Font size",
            &Some(current),
            None,
            &options,
        ) {
            with.props.font_size = Some(size);
            field = Some("font-size");
        }
        // Actors are figures, with no border to dash.
        if kind != HighLevelKind::Actors {
            widgets::label(ui, "Border", false);
            ui.horizontal_wrapped(|ui| {
                for (dashed, label) in [(false, "Solid"), (true, "Dashed")] {
                    if ui
                        .radio_value(&mut with.props.dashed, dashed, label)
                        .changed()
                    {
                        field = Some("dashed");
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
        let mut two_way = connection.props.two_way;
        if widgets::checkbox(ui, &mut two_way, "Both ways").changed() {
            let id = connection.id.clone();
            cx.apply(Some(format!("{id}:two-way")), move |eng| {
                eng.set_connection_props(&id, BlockArrow { two_way });
            });
        }
    }

    fn arrow_title(
        _eng: &Engine<Self>,
        connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        if connection.props.two_way {
            format!("Both ways with {other}")
        } else if outgoing {
            format!("To {other}")
        } else {
            format!("From {other}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_names_the_looks() {
        assert_eq!(HighLevel::HELP.lines().count(), 6);
        assert!(HighLevel::HELP.contains("font size"));
        assert!(HighLevel::HELP.contains("dashed"));
    }
}
