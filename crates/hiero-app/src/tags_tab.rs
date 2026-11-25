//! The Scenes and Tags tabs of a diagram's Settings page.
//!
//! Scenes are listed with their names in fields and the tags they show as boxes to tick,
//! changed in place, and buttons to move or delete one, with a form after them to add one: a
//! scene added with no tags ticked gets a tag of its own, named after it. Tags are listed with
//! their names in fields, typed over to rename them, how many elements (or on a sequence
//! diagram, messages) are in each, and buttons to move one earlier or later, or delete it, with
//! a field to add one. Deleting a tag that has elements, or that a scene shows alone, asks
//! first. Each change goes to the page's draft, saved with the rest by Save Changes. Elements
//! are put in tags, and tags shown and hidden, on the diagram itself.

use egui::{RichText, Ui};
use hiero_core::tags::{
    DiagramScene, DiagramTag, moved, new_id, scene_of, scene_problem, scenes_only_of,
    scenes_without_tag, tag_counts, tag_for_scene, tag_name_problem,
};
use hiero_editor::fonts;
use hiero_editor::theme::{danger_button, field_label, form_style, page_button, row_button};

use crate::app::App;
use crate::pages::{error_text, ruled_grid};

/// Names in a sentence: "A", "A and B", or "A, B, and C".
fn list_of(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// What a row's buttons ask for, done after drawing.
enum RowAction {
    Up(usize),
    Down(usize),
    AskDelete(String),
    Delete(String),
    Keep,
}

impl App {
    fn heading(ui: &mut Ui, text: &str, hint: &str) {
        ui.heading(RichText::new(text).font(fonts::bold(18.0)));
        ui.label(RichText::new(hint).weak());
        ui.add_space(8.0);
    }

    pub fn tags_tab(&mut self, ui: &mut Ui) {
        let Some(open) = &self.open else {
            return;
        };
        let messages = open.record.meta.diagram_type.tags_hold_arrows();
        let counts = tag_counts(&open.record.content, messages);
        let (noun, nouns) = if messages {
            ("message", "Messages")
        } else {
            ("element", "Elements")
        };
        Self::heading(
            ui,
            "Tags",
            if messages {
                "Tags are sets of messages that can be shown or hidden together, as scenes do. Put messages in them on the diagram; the participants are in none and always show."
            } else {
                "Tags are sets of elements that can be shown or hidden together. Put elements in them, and show or hide them, on the diagram. They play in this order."
            },
        );
        form_style(ui);
        let problems = self.form.tags.tag_problems();
        let form = &mut self.form.tags;
        let mut action = None;
        if form.tags.is_empty() {
            ui.label("This diagram has no tags yet.");
        } else {
            let count_of = form.tags.len();
            let scenes = form.scenes.clone();
            ruled_grid(ui, "tags", 3, |ui, end_row| {
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new(nouns).strong());
                ui.label("");
                end_row(ui);
                for (i, tag) in form.tags.iter_mut().enumerate() {
                    let count = counts.get(&tag.id).copied().unwrap_or(0);
                    if form.confirming.as_deref() == Some(tag.id.as_str()) {
                        let mut parts = Vec::new();
                        if count > 0 {
                            parts.push(format!(
                                "{count} {noun}{} will be taken out of it.",
                                if count == 1 { "" } else { "s" }
                            ));
                        }
                        let only: Vec<String> = scenes_only_of(&scenes, &tag.id)
                            .into_iter()
                            .map(|s| s.name.clone())
                            .collect();
                        if !only.is_empty() {
                            parts.push(format!(
                                "{} {}, which {} only it, will be deleted.",
                                if only.len() == 1 {
                                    "The scene"
                                } else {
                                    "The scenes"
                                },
                                list_of(&only),
                                if only.len() == 1 { "shows" } else { "show" }
                            ));
                        }
                        ui.label(format!("Delete {}? {}", tag.name.trim(), parts.join(" ")));
                        ui.label("");
                        ui.horizontal(|ui| {
                            if danger_button(ui, "Delete").clicked() {
                                action = Some(RowAction::Delete(tag.id.clone()));
                            }
                            if page_button(ui, "Keep", false).clicked() {
                                action = Some(RowAction::Keep);
                            }
                        });
                    } else {
                        // The name, typed over in place.
                        ui.vertical(|ui| {
                            ui.set_min_width(220.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut tag.name)
                                    .desired_width(220.0)
                                    .id_salt(("tag-name", &tag.id)),
                            )
                            .widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::TextInput,
                                    true,
                                    format!("Name of tag {}", i + 1),
                                )
                            });
                            if let Some(problem) = problems.get(&tag.id) {
                                ui.label(error_text(ui, problem));
                            }
                        });
                        ui.label(count.to_string());
                        let name = tag.name.trim();
                        ui.horizontal(|ui| {
                            let up = ui.add_enabled_ui(i > 0, |ui| row_button(ui, "↑"));
                            if up.inner.on_hover_text(format!("Move {name} up")).clicked() {
                                action = Some(RowAction::Up(i));
                            }
                            let down =
                                ui.add_enabled_ui(i + 1 < count_of, |ui| row_button(ui, "↓"));
                            if down
                                .inner
                                .on_hover_text(format!("Move {name} down"))
                                .clicked()
                            {
                                action = Some(RowAction::Down(i));
                            }
                            if row_button(ui, "Delete").clicked() {
                                action = Some(RowAction::AskDelete(tag.id.clone()));
                            }
                        });
                    }
                    end_row(ui);
                }
            });
        }
        ui.add_space(14.0);
        field_label(ui, "New tag", false);
        let mut add = false;
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut form.new_tag).desired_width(260.0));
            add = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if page_button(ui, "Add Tag", true).clicked() {
                add = true;
            }
        });
        if let Some(error) = &form.new_tag_error {
            ui.label(error_text(ui, error));
        }
        if add {
            match tag_name_problem(&form.new_tag, &form.tags, None) {
                Some(problem) => form.new_tag_error = Some(problem),
                None => {
                    form.new_tag_error = None;
                    let name = form.new_tag.trim().to_owned();
                    form.new_tag.clear();
                    form.tags.push(DiagramTag { id: new_id(), name });
                }
            }
        }
        if let Some(action) = action {
            self.tag_action(action, &counts);
        }
    }

    fn tag_action(
        &mut self,
        action: RowAction,
        counts: &std::collections::BTreeMap<String, usize>,
    ) {
        let form = &mut self.form.tags;
        match action {
            RowAction::Up(i) => form.tags = moved(&form.tags, i, i - 1),
            RowAction::Down(i) => form.tags = moved(&form.tags, i, i + 1),
            RowAction::AskDelete(id) => {
                if counts.contains_key(&id) || !scenes_only_of(&form.scenes, &id).is_empty() {
                    form.confirming = Some(id);
                } else {
                    Self::delete_tag(form, &id);
                }
            }
            RowAction::Delete(id) => {
                form.confirming = None;
                Self::delete_tag(form, &id);
            }
            RowAction::Keep => form.confirming = None,
        }
    }

    /// Deletes a tag from the draft, with the scenes that showed only it. Its elements are
    /// taken out of it when the changes are saved.
    fn delete_tag(form: &mut crate::app::TagsForm, id: &str) {
        form.tags.retain(|t| t.id != id);
        form.scenes = scenes_without_tag(&form.scenes, id);
        form.scene_tags.remove(id);
    }

    pub fn scenes_tab(&mut self, ui: &mut Ui) {
        Self::heading(
            ui,
            "Scenes",
            "A scene shows the elements of the tags you choose and hides the other tags' elements. Elements in no tag show in every scene. Viewing the diagram opens it on the last scene, and Play steps through them in this order, from the first.",
        );
        form_style(ui);
        let problems = self.form.tags.scene_problems();
        let form = &mut self.form.tags;
        let tags = form.tags.clone();
        let mut action = None;
        if form.scenes.is_empty() {
            ui.label("This diagram has no scenes yet.");
        } else {
            let count = form.scenes.len();
            // The tags wrap, leaving the name its field and the buttons one line.
            let tags_width = (ui.available_width() - 240.0 - 200.0).clamp(180.0, 420.0);
            ruled_grid(ui, "scenes", 3, |ui, end_row| {
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Tags").strong());
                ui.label("");
                end_row(ui);
                for (i, scene) in form.scenes.iter_mut().enumerate() {
                    let problem = problems.get(&scene.id);
                    ui.vertical(|ui| {
                        ui.set_min_width(200.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut scene.name)
                                .desired_width(200.0)
                                .id_salt(("scene-name", &scene.id)),
                        )
                        .widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::TextInput,
                                true,
                                format!("Name of scene {}", i + 1),
                            )
                        });
                        if let Some(error) = problem.and_then(|p| p.0.as_ref()) {
                            ui.label(error_text(ui, error));
                        }
                    });
                    ui.vertical(|ui| {
                        ui.set_max_width(tags_width);
                        ui.horizontal_wrapped(|ui| {
                            for tag in &tags {
                                let mut on = scene.tags.contains(&tag.id);
                                if ui.checkbox(&mut on, &tag.name).changed() {
                                    if on {
                                        scene.tags.push(tag.id.clone());
                                    } else {
                                        scene.tags.retain(|t| *t != tag.id);
                                    }
                                }
                            }
                        });
                        if let Some(error) = problem.and_then(|p| p.1.as_ref()) {
                            ui.label(error_text(ui, error));
                        }
                    });
                    let name = scene.name.trim();
                    // The buttons keep to one line.
                    ui.horizontal(|ui| {
                        let up = ui.add_enabled_ui(i > 0, |ui| row_button(ui, "↑"));
                        if up.inner.on_hover_text(format!("Move {name} up")).clicked() {
                            action = Some(RowAction::Up(i));
                        }
                        let down = ui.add_enabled_ui(i + 1 < count, |ui| row_button(ui, "↓"));
                        if down
                            .inner
                            .on_hover_text(format!("Move {name} down"))
                            .clicked()
                        {
                            action = Some(RowAction::Down(i));
                        }
                        if row_button(ui, "Delete").clicked() {
                            action = Some(RowAction::Delete(scene.id.clone()));
                        }
                    });
                    end_row(ui);
                }
            });
        }
        match action {
            Some(RowAction::Up(i)) => form.scenes = moved(&form.scenes, i, i - 1),
            Some(RowAction::Down(i)) => form.scenes = moved(&form.scenes, i, i + 1),
            Some(RowAction::Delete(id)) => form.scenes.retain(|s| s.id != id),
            _ => {}
        }
        ui.add_space(14.0);
        self.scene_form(ui, &tags);
    }

    /// The form that adds a scene: its name and the tags it shows, or with none ticked, a tag of
    /// its own, the tag of its name or a new one named after it.
    fn scene_form(&mut self, ui: &mut Ui, tags: &[DiagramTag]) {
        ui.label(RichText::new("New scene").font(fonts::bold(15.0)));
        ui.add_space(4.0);
        let form = &mut self.form.tags;
        field_label(ui, "Name", false);
        ui.add(egui::TextEdit::singleline(&mut form.scene_name).desired_width(260.0));
        if let Some(error) = &form.scene_name_error {
            ui.label(error_text(ui, error));
        }
        ui.add_space(8.0);
        field_label(ui, "Tags it shows", false);
        ui.label(
            RichText::new("Leave them all unticked to give it a tag of its own, named after it.")
                .weak(),
        );
        for tag in tags {
            let mut on = form.scene_tags.contains(&tag.id);
            if ui.checkbox(&mut on, &tag.name).changed() {
                if on {
                    form.scene_tags.insert(tag.id.clone());
                } else {
                    form.scene_tags.remove(&tag.id);
                }
            }
        }
        ui.add_space(8.0);
        if page_button(ui, "Add Scene", true).clicked() {
            let ticked: Vec<String> = form.scene_tags.iter().cloned().collect();
            // Its own tag, when none is ticked, made once its name is fine.
            let own = ticked
                .is_empty()
                .then(|| tag_for_scene(&form.scene_name, tags));
            let shown = own.as_ref().map_or(ticked, |(_, id)| vec![id.clone()]);
            if let Some((name, _)) = scene_problem(&form.scene_name, &shown, &form.scenes, None)
                .filter(|(name, _)| name.is_some())
            {
                form.scene_name_error = name;
                return;
            }
            let all = own.map_or_else(|| tags.to_vec(), |(all, _)| all);
            let scene: DiagramScene = scene_of(new_id(), &form.scene_name, &shown, &all);
            form.tags = all;
            form.scenes.push(scene);
            form.scene_name.clear();
            form.scene_tags.clear();
            form.scene_name_error = None;
        }
    }
}
