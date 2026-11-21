//! The Tags and Scenes tabs of a diagram's Edit Details page.
//!
//! Tags are listed with how many elements are in each, and can be renamed, moved earlier or
//! later, or deleted, with a field to add one. Deleting a tag that has elements, or that a
//! scene shows alone, asks first. Scenes are listed with the tags they show, and can be edited,
//! moved, or deleted, with a form to add or edit one. Each change applies at once, as one undo
//! step. Elements are put in tags, and tags shown and hidden, on the diagram itself.

use egui::{Color32, RichText, Ui};
use hiero_core::tags::{
    DiagramScene, moved, new_id, scene_of, scene_problem, scenes_only_of, scenes_without_tag,
    tag_counts, tag_name_problem, untag_elements, with_scene,
};
use hiero_editor::fonts;
use hiero_editor::theme::{
    danger_button, field_label, form_style, page_button, palette, row_button,
};

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

/// What a tag's change is, for the row's buttons to report after drawing.
enum TagAction {
    Up(usize),
    Down(usize),
    Rename(String),
    SaveRename(String),
    CancelRename,
    AskDelete(String),
    Delete(String),
    Keep,
}

enum SceneAction {
    Up(usize),
    Down(usize),
    Edit(String),
    Delete(String),
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
        let tags = open.record.tags.clone();
        let scenes = open.record.scenes.clone();
        let counts = tag_counts(&open.record.content.elements);
        Self::heading(
            ui,
            "Tags",
            "Tags are sets of elements that can be shown or hidden together. Put elements in them, and show or hide them, on the diagram. They play in this order.",
        );
        form_style(ui);
        let mut action = None;
        if tags.is_empty() {
            ui.label("This diagram has no tags yet.");
        } else {
            let form = &mut self.form.tags;
            ruled_grid(ui, "tags", 3, |ui, end_row| {
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Elements").strong());
                ui.label("");
                end_row(ui);
                for (i, tag) in tags.iter().enumerate() {
                    let count = counts.get(&tag.id).copied().unwrap_or(0);
                    if let Some((id, name, error)) = &mut form.renaming
                        && *id == tag.id
                    {
                        let edit = ui.add(
                            egui::TextEdit::singleline(name)
                                .desired_width(220.0)
                                .hint_text(format!("Name of {}", tag.name)),
                        );
                        if std::mem::take(&mut form.focus) {
                            edit.request_focus();
                        }
                        let enter =
                            edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ui.vertical(|ui| {
                            if let Some(error) = error {
                                ui.label(error_text(ui, error));
                            }
                        });
                        ui.horizontal(|ui| {
                            if enter || page_button(ui, "Save", true).clicked() {
                                action = Some(TagAction::SaveRename(tag.id.clone()));
                            }
                            if page_button(ui, "Cancel", false).clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                            {
                                action = Some(TagAction::CancelRename);
                            }
                        });
                    } else if form.confirming.as_deref() == Some(tag.id.as_str()) {
                        let mut parts = Vec::new();
                        if count > 0 {
                            parts.push(format!(
                                "{count} element{} will be taken out of it.",
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
                        ui.label(format!("Delete {}? {}", tag.name, parts.join(" ")));
                        ui.label("");
                        ui.horizontal(|ui| {
                            if danger_button(ui, "Delete").clicked() {
                                action = Some(TagAction::Delete(tag.id.clone()));
                            }
                            if page_button(ui, "Keep", false).clicked() {
                                action = Some(TagAction::Keep);
                            }
                        });
                    } else {
                        ui.label(RichText::new(&tag.name).strong());
                        ui.label(count.to_string());
                        ui.horizontal(|ui| {
                            let up = ui.add_enabled_ui(i > 0, |ui| row_button(ui, "↑"));
                            if up
                                .inner
                                .on_hover_text(format!("Move {} up", tag.name))
                                .clicked()
                            {
                                action = Some(TagAction::Up(i));
                            }
                            let down =
                                ui.add_enabled_ui(i + 1 < tags.len(), |ui| row_button(ui, "↓"));
                            if down
                                .inner
                                .on_hover_text(format!("Move {} down", tag.name))
                                .clicked()
                            {
                                action = Some(TagAction::Down(i));
                            }
                            if row_button(ui, "Rename").clicked() {
                                action = Some(TagAction::Rename(tag.id.clone()));
                            }
                            if row_button(ui, "Delete").clicked() {
                                action = Some(TagAction::AskDelete(tag.id.clone()));
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
            let edit = ui
                .add(egui::TextEdit::singleline(&mut self.form.tags.new_tag).desired_width(260.0));
            add = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if page_button(ui, "Add Tag", true).clicked() {
                add = true;
            }
        });
        if let Some(error) = &self.form.tags.new_tag_error {
            ui.label(error_text(ui, error));
        }
        if add {
            let name = self.form.tags.new_tag.clone();
            match tag_name_problem(&name, &tags, None) {
                Some(problem) => self.form.tags.new_tag_error = Some(problem),
                None => {
                    self.form.tags.new_tag_error = None;
                    self.form.tags.new_tag.clear();
                    let name = name.trim().to_owned();
                    self.change_tags(|d| {
                        d.tags
                            .push(hiero_core::tags::DiagramTag { id: new_id(), name });
                    });
                }
            }
        }
        if let Some(action) = action {
            self.tag_action(action);
        }
    }

    fn tag_action(&mut self, action: TagAction) {
        let form = &mut self.form.tags;
        match action {
            TagAction::Up(i) => {
                self.change_tags(|d| d.tags = moved(&d.tags, i, i - 1));
            }
            TagAction::Down(i) => {
                self.change_tags(|d| d.tags = moved(&d.tags, i, i + 1));
            }
            TagAction::Rename(id) => {
                let name = self
                    .open
                    .as_ref()
                    .and_then(|o| o.record.tags.iter().find(|t| t.id == id))
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                form.confirming = None;
                form.renaming = Some((id, name, None));
                form.focus = true;
            }
            TagAction::CancelRename => form.renaming = None,
            TagAction::SaveRename(id) => {
                let Some((_, name, _)) = form.renaming.clone() else {
                    return;
                };
                let tags = self
                    .open
                    .as_ref()
                    .map(|o| o.record.tags.clone())
                    .unwrap_or_default();
                if let Some(problem) = tag_name_problem(&name, &tags, Some(&id)) {
                    if let Some((_, _, error)) = &mut self.form.tags.renaming {
                        *error = Some(problem);
                    }
                    return;
                }
                self.form.tags.renaming = None;
                let name = name.trim().to_owned();
                self.change_tags(|d| {
                    if let Some(tag) = d.tags.iter_mut().find(|t| t.id == id) {
                        tag.name = name;
                    }
                });
            }
            TagAction::AskDelete(id) => {
                form.renaming = None;
                let asks = self.open.as_ref().is_some_and(|o| {
                    tag_counts(&o.record.content.elements).contains_key(&id)
                        || !scenes_only_of(&o.record.scenes, &id).is_empty()
                });
                if asks {
                    self.form.tags.confirming = Some(id);
                } else {
                    self.delete_tag(&id);
                }
            }
            TagAction::Delete(id) => {
                form.confirming = None;
                self.delete_tag(&id);
            }
            TagAction::Keep => form.confirming = None,
        }
    }

    /// Deletes a tag, taking its elements out of it, and the scenes that showed only it.
    fn delete_tag(&mut self, id: &str) {
        self.change_tags(|d| {
            d.tags.retain(|t| t.id != id);
            untag_elements(&mut d.content.elements, id);
            d.scenes = scenes_without_tag(&d.scenes, id);
        });
    }

    pub fn scenes_tab(&mut self, ui: &mut Ui) {
        let Some(open) = &self.open else {
            return;
        };
        let tags = open.record.tags.clone();
        let scenes = open.record.scenes.clone();
        Self::heading(
            ui,
            "Scenes",
            "A scene shows the elements of the tags you choose and hides the other tags' elements. Elements in no tag show in every scene. While viewing the diagram, scenes are stepped through in this order.",
        );
        form_style(ui);
        let mut action = None;
        // The scene being edited is edited in its place in the list, and the form for adding
        // one is hidden meanwhile, so the list needs no scrolling.
        let editing = self
            .form
            .tags
            .editing_scene
            .as_ref()
            .and_then(|id| scenes.iter().position(|s| s.id == *id));
        if scenes.is_empty() {
            ui.label("This diagram has no scenes yet.");
        } else {
            let tag_names = |scene: &DiagramScene| {
                let names: Vec<&str> = scene
                    .tags
                    .iter()
                    .map(|id| {
                        tags.iter()
                            .find(|t| t.id == *id)
                            .map_or(id.as_str(), |t| t.name.as_str())
                    })
                    .collect();
                names.join(", ")
            };
            // The list may be split round the form, so its columns are as wide as the widest
            // name and tags of any scene, to line up either side of it.
            let width = |ui: &Ui, text: &str, strong: bool| {
                let font = if strong {
                    fonts::bold(egui::TextStyle::Body.resolve(ui.style()).size)
                } else {
                    egui::TextStyle::Body.resolve(ui.style())
                };
                ui.painter()
                    .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER)
                    .size()
                    .x
            };
            let name_width = scenes
                .iter()
                .map(|s| width(ui, &s.name, true))
                .fold(width(ui, "Name", true), f32::max);
            // The tags wrap, leaving the buttons the room they need on one line.
            let buttons = ["↑", "↓", "Edit", "Delete"]
                .iter()
                .map(|t| width(ui, t, false) + 34.0)
                .sum::<f32>()
                + 3.0 * ui.spacing().item_spacing.x;
            let room = ui.available_width() - name_width - buttons - 2.0 * 24.0;
            let tags_width = scenes
                .iter()
                .map(|s| width(ui, &tag_names(s), false))
                .fold(width(ui, "Tags", true), f32::max)
                .min(room.max(160.0));
            let cell = |ui: &mut Ui, size: f32, text: RichText| {
                ui.scope(|ui| {
                    ui.set_min_width(size);
                    ui.set_max_width(size);
                    ui.add(egui::Label::new(text).wrap());
                });
            };
            let count = scenes.len();
            let rows = |ui: &mut Ui,
                        id: &str,
                        range: std::ops::Range<usize>,
                        header: bool,
                        action: &mut Option<SceneAction>| {
                ruled_grid(ui, id, 3, |ui, end_row| {
                    if header {
                        cell(ui, name_width, RichText::new("Name").strong());
                        cell(ui, tags_width, RichText::new("Tags").strong());
                        ui.label("");
                        end_row(ui);
                    }
                    for i in range {
                        let scene = &scenes[i];
                        cell(ui, name_width, RichText::new(&scene.name).strong());
                        cell(ui, tags_width, RichText::new(tag_names(scene)));
                        // The buttons keep to one line.
                        ui.horizontal(|ui| {
                            let up = ui.add_enabled_ui(i > 0, |ui| row_button(ui, "↑"));
                            if up
                                .inner
                                .on_hover_text(format!("Move {} up", scene.name))
                                .clicked()
                            {
                                *action = Some(SceneAction::Up(i));
                            }
                            let down = ui.add_enabled_ui(i + 1 < count, |ui| row_button(ui, "↓"));
                            if down
                                .inner
                                .on_hover_text(format!("Move {} down", scene.name))
                                .clicked()
                            {
                                *action = Some(SceneAction::Down(i));
                            }
                            if row_button(ui, "Edit").clicked() {
                                *action = Some(SceneAction::Edit(scene.id.clone()));
                            }
                            if row_button(ui, "Delete").clicked() {
                                *action = Some(SceneAction::Delete(scene.id.clone()));
                            }
                        });
                        end_row(ui);
                    }
                });
            };
            match editing {
                Some(at) => {
                    rows(ui, "scenes", 0..at, true, &mut action);
                    // In the scene's place, where its Edit button was, set off from the rows
                    // round it.
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(palette(ui).surface)
                        .stroke(egui::Stroke::new(1.0, palette(ui).border))
                        .corner_radius(6)
                        .inner_margin(12)
                        .show(ui, |ui| self.scene_form(ui, &tags, &scenes));
                    ui.add_space(8.0);
                    rows(ui, "scenes-after", at + 1..count, false, &mut action);
                }
                None => rows(ui, "scenes", 0..count, true, &mut action),
            }
        }
        ui.add_space(14.0);
        if tags.is_empty() {
            ui.label("Add tags on the Tags tab first: each scene shows one or more of them.");
        } else if editing.is_none() {
            self.scene_form(ui, &tags, &scenes);
        }
        if let Some(action) = action {
            match action {
                SceneAction::Up(i) => {
                    self.change_tags(|d| d.scenes = moved(&d.scenes, i, i - 1));
                }
                SceneAction::Down(i) => {
                    self.change_tags(|d| d.scenes = moved(&d.scenes, i, i + 1));
                }
                SceneAction::Edit(id) => {
                    if let Some(scene) = scenes.iter().find(|s| s.id == id) {
                        let form = &mut self.form.tags;
                        form.editing_scene = Some(id);
                        form.scene_name.clone_from(&scene.name);
                        form.scene_tags = scene.tags.iter().cloned().collect();
                        form.scene_name_error = None;
                        form.scene_tags_error = None;
                        form.focus = true;
                    }
                }
                SceneAction::Delete(id) => {
                    if self.form.tags.editing_scene.as_deref() == Some(id.as_str()) {
                        self.reset_scene_form();
                    }
                    self.change_tags(|d| d.scenes.retain(|s| s.id != id));
                }
            }
        }
    }

    fn reset_scene_form(&mut self) {
        let form = &mut self.form.tags;
        form.editing_scene = None;
        form.scene_name.clear();
        form.scene_tags.clear();
        form.scene_name_error = None;
        form.scene_tags_error = None;
    }

    /// The form that adds a scene, or edits one: its name and the tags it shows.
    fn scene_form(
        &mut self,
        ui: &mut Ui,
        tags: &[hiero_core::tags::DiagramTag],
        scenes: &[DiagramScene],
    ) {
        let editing = self
            .form
            .tags
            .editing_scene
            .as_ref()
            .and_then(|id| scenes.iter().find(|s| s.id == *id))
            .cloned();
        let title = editing
            .as_ref()
            .map_or_else(|| "New scene".to_owned(), |s| format!("Edit {}", s.name));
        ui.label(RichText::new(title).font(fonts::bold(15.0)));
        ui.add_space(4.0);
        let form = &mut self.form.tags;
        field_label(ui, "Name", false);
        let edit = ui.add(egui::TextEdit::singleline(&mut form.scene_name).desired_width(260.0));
        if std::mem::take(&mut form.focus) && editing.is_some() {
            edit.request_focus();
        }
        if let Some(error) = &form.scene_name_error {
            ui.label(error_text(ui, error));
        }
        ui.add_space(8.0);
        field_label(ui, "Tags it shows", false);
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
        if let Some(error) = &form.scene_tags_error {
            ui.label(error_text(ui, error));
        }
        ui.add_space(8.0);
        let mut submit = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            let label = if editing.is_some() {
                "Save Scene"
            } else {
                "Add Scene"
            };
            submit = page_button(ui, label, true).clicked();
            if editing.is_some() {
                cancel = page_button(ui, "Cancel", false).clicked();
            }
        });
        if cancel {
            self.reset_scene_form();
        } else if submit {
            let form = &mut self.form.tags;
            let shown: Vec<String> = form.scene_tags.iter().cloned().collect();
            let except = editing.as_ref().map(|s| s.id.as_str());
            if let Some((name, tags_error)) =
                scene_problem(&form.scene_name, &shown, scenes, except)
            {
                form.scene_name_error = name;
                form.scene_tags_error = tags_error;
                return;
            }
            let id = editing.map_or_else(new_id, |s| s.id);
            let scene = scene_of(id, &form.scene_name, &shown, tags);
            self.reset_scene_form();
            self.change_tags(|d| d.scenes = with_scene(&d.scenes, scene));
        }
    }
}
