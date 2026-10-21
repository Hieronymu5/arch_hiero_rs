//! The app's pages: the list of diagrams, the Add diagram and Edit details forms, the editor
//! with its toolbar, and a diagram's history; and the dialogs over them.

use egui::{Align, Color32, Key, Layout, Modifiers, RichText, Ui};
use hiero_core::history::validate_version_description;
use hiero_core::model::{DiagramId, DiagramTypeId, validate_description, validate_name};
use hiero_editor::fonts;
use hiero_store::Status;

use crate::app::{App, DetailsForm, Dialog, ExportFormat, Outcome, Page};
use crate::format::date;

fn error_text(ui: &Ui, text: &str) -> RichText {
    RichText::new(text).color(ui.visuals().error_fg_color)
}

/// A Saved/Unsaved badge.
fn status_badge(ui: &mut Ui, status: Status) {
    let (fill, text) = if status.saved {
        (
            Color32::from_rgb(213, 232, 212),
            Color32::from_rgb(30, 80, 30),
        )
    } else {
        (
            Color32::from_rgb(255, 242, 204),
            Color32::from_rgb(110, 80, 0),
        )
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(status.label()).color(text).small().strong());
        });
}

impl App {
    pub fn top_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("top-bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Arch Hiero").font(fonts::bold(18.0)));
                ui.separator();
                if ui
                    .selectable_label(matches!(self.page, Page::List), "Diagrams")
                    .clicked()
                {
                    self.flush_save();
                    self.back.clear();
                    self.page = Page::List;
                    self.refresh_list();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for (theme, label) in [
                        (egui::ThemePreference::Dark, "Dark"),
                        (egui::ThemePreference::Light, "Light"),
                        (egui::ThemePreference::System, "System"),
                    ] {
                        if ui.selectable_label(self.theme == theme, label).clicked() {
                            self.theme = theme;
                            ui.ctx().set_theme(theme);
                        }
                    }
                    ui.label(RichText::new("Theme").weak());
                });
            });
        });
    }

    fn outcome_ui(&mut self, ui: &mut Ui) {
        if let Some(outcome) = &self.outcome {
            let text = if outcome.error {
                error_text(ui, &outcome.message)
            } else {
                RichText::new(&outcome.message)
            };
            ui.horizontal(|ui| {
                ui.label(text);
                if ui.small_button("Dismiss").clicked() {
                    self.outcome = None;
                }
            });
            ui.add_space(6.0);
        }
    }

    // The list of diagrams

    pub fn list_page(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Diagrams").font(fonts::bold(26.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Add diagram").clicked() {
                    self.form = DetailsForm::default();
                    self.go(Page::New);
                }
                if ui.button("Import diagram").clicked() {
                    self.import();
                }
            });
        });
        ui.add_space(8.0);
        self.outcome_ui(ui);
        if self.list.is_empty() {
            ui.label("No diagrams have been added yet.");
            return;
        }
        ui.label(RichText::new(format!("{} diagrams, sorted by name", self.list.len())).weak());
        let rows = self.list.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("diagrams")
                .num_columns(5)
                .striped(true)
                .spacing([24.0, 10.0])
                .show(ui, |ui| {
                    for header in ["Name", "Type", "Last updated", "Status", ""] {
                        ui.label(RichText::new(header).strong());
                    }
                    ui.end_row();
                    for row in rows {
                        let id = row.meta.id.clone();
                        if ui.link(RichText::new(&row.meta.name).strong()).clicked() {
                            self.open_diagram(&id);
                        }
                        ui.label(row.meta.diagram_type.label());
                        ui.label(date(&row.meta.updated_at));
                        status_badge(ui, row.status);
                        ui.horizontal(|ui| {
                            if ui.button("History").clicked() {
                                self.go(Page::History(id.clone()));
                            }
                            if ui.button("Export").clicked() {
                                self.export(&id, None, ExportFormat::Zip);
                            }
                            if ui.button("Delete").clicked() {
                                self.dialog = Some(Dialog::DeleteDiagram {
                                    id: id.clone(),
                                    name: row.meta.name.clone(),
                                });
                            }
                        });
                        ui.end_row();
                    }
                });
        });
    }

    // Add diagram and Edit details

    fn details_fields(&mut self, ui: &mut Ui, adding: bool) -> bool {
        let tried = self.form.tried;
        ui.label(RichText::new("Fields marked * are required.").weak());
        ui.add_space(4.0);
        ui.label("Name *");
        ui.add(egui::TextEdit::singleline(&mut self.form.name).desired_width(420.0));
        let name_ok = validate_name(&self.form.name);
        if let (true, Err(message)) = (tried, name_ok) {
            ui.label(error_text(ui, message));
        }
        ui.add_space(6.0);
        ui.label("Type *");
        if adding {
            let selected = self
                .form
                .diagram_type
                .map_or("Select a diagram type", DiagramTypeId::label);
            egui::ComboBox::from_id_salt("diagram-type")
                .selected_text(selected)
                .width(420.0)
                .show_ui(ui, |ui| {
                    for t in DiagramTypeId::ALL {
                        ui.selectable_value(&mut self.form.diagram_type, Some(t), t.label());
                    }
                });
            if tried && self.form.diagram_type.is_none() {
                ui.label(error_text(ui, "Select a diagram type."));
            }
        } else {
            ui.label(
                RichText::new(self.form.diagram_type.map_or("", DiagramTypeId::label)).strong(),
            );
            ui.label(
                RichText::new("A diagram's type can't be changed after it's created.")
                    .weak()
                    .small(),
            );
        }
        ui.add_space(6.0);
        ui.label("Description");
        ui.add(
            egui::TextEdit::multiline(&mut self.form.description)
                .desired_width(420.0)
                .desired_rows(4),
        );
        let description_ok = validate_description(&self.form.description);
        if let Err(message) = description_ok {
            ui.label(error_text(ui, message));
        }
        name_ok.is_ok() && description_ok.is_ok() && self.form.diagram_type.is_some()
    }

    pub fn new_page(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Add diagram").font(fonts::bold(26.0)));
        ui.add_space(8.0);
        let valid = self.details_fields(ui, true);
        if let Some(error) = &self.form.error {
            ui.label(error_text(ui, error));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Add diagram").clicked() {
                self.form.tried = true;
                if valid {
                    self.create();
                }
            }
            if ui.button("Cancel").clicked() {
                self.go_back();
            }
        });
    }

    pub fn details_page(&mut self, ui: &mut Ui, id: &DiagramId) {
        let Some(open) = self.open.as_ref().filter(|o| o.record.meta.id == *id) else {
            ui.label("This diagram doesn't exist. It may have been deleted.");
            return;
        };
        let meta = open.record.meta.clone();
        ui.heading(RichText::new("Edit diagram details").font(fonts::bold(26.0)));
        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let valid = self.details_fields(ui, false);
            let ops = hiero_registry::ops(meta.diagram_type);
            if !ops.color_fields().is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("Colors").strong());
                let defaults = ops.default_colors();
                for (key, label) in ops.color_fields() {
                    let current = self
                        .form
                        .colors
                        .get(*key)
                        .or_else(|| defaults.get(*key))
                        .cloned()
                        .unwrap_or_default();
                    let [r, g, b] = hiero_core::color::channels(&current);
                    let mut rgb = [r, g, b];
                    ui.horizontal(|ui| {
                        if ui.color_edit_button_srgb(&mut rgb).changed() {
                            self.form.colors.insert(
                                (*key).to_owned(),
                                format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
                            );
                        }
                        ui.label(*label);
                    });
                }
                if ui.link("Use the default colors").clicked() {
                    self.form.colors.clear();
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Save changes").clicked() {
                    self.form.tried = true;
                    if valid {
                        self.save_details();
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.go_back();
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new(format!("Created {}", date(&meta.created_at))).weak());
            ui.label(RichText::new(format!("Last updated {}", date(&meta.updated_at))).weak());
        });
    }

    // The editor

    pub fn editor_page(&mut self, ui: &mut Ui, id: &DiagramId) {
        if !self.open.as_ref().is_some_and(|o| o.record.meta.id == *id) {
            ui.label("This diagram doesn't exist. It may have been deleted.");
            if ui.link("Back to diagrams").clicked() {
                self.page = Page::List;
            }
            return;
        }
        let (can_undo, can_redo) = (self.can_undo(), self.can_redo());
        let mut action: Option<&'static str> = None;
        if let Some(open) = &mut self.open {
            let meta = open.record.meta.clone();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&meta.name).font(fonts::bold(22.0)));
                    ui.label(
                        RichText::new(format!("{} diagram", meta.diagram_type.label())).weak(),
                    );
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    status_badge(ui, open.status);
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Back").clicked() {
                        action = Some("back");
                    }
                    if ui.button("Edit details").clicked() {
                        action = Some("details");
                    }
                    let has_content = open.editor.has_content();
                    if ui
                        .add_enabled(has_content, egui::Button::new("Export draw.io"))
                        .clicked()
                    {
                        action = Some("drawio");
                    }
                    if ui
                        .add_enabled(has_content, egui::Button::new("Export SVG"))
                        .clicked()
                    {
                        action = Some("svg");
                    }
                    let help = open.editor.show_help();
                    if ui.selectable_label(*help, "Help").clicked() {
                        *help = !*help;
                    }
                    if ui.button("History").clicked() {
                        action = Some("history");
                    }
                    if ui
                        .add_enabled(can_redo, egui::Button::new("Redo"))
                        .on_hover_text("Shift+Cmd+Z")
                        .clicked()
                    {
                        action = Some("redo");
                    }
                    if ui
                        .add_enabled(can_undo, egui::Button::new("Undo"))
                        .on_hover_text("Cmd+Z")
                        .clicked()
                    {
                        action = Some("undo");
                    }
                    if ui
                        .add_enabled(!open.status.saved, egui::Button::new("Save"))
                        .on_hover_text("Record a version (Cmd+S)")
                        .clicked()
                    {
                        action = Some("save");
                    }
                });
            });
            if let Some(message) = &open.save_failed {
                ui.label(error_text(ui, &format!("Your last change couldn't be saved. Make another change to try again. ({message})")));
            }
            if let Some(notice) = &open.notice {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(notice).color(Color32::from_rgb(180, 110, 0)));
                });
            }
            ui.separator();
            open.editor.ui(ui);
        }
        match action {
            Some("back") => self.go_back(),
            Some("details") => self.start_details(),
            Some("drawio") => self.export(id, None, ExportFormat::Drawio),
            Some("svg") => self.export(id, None, ExportFormat::Svg),
            Some("history") => self.go(Page::History(id.clone())),
            Some("undo") => self.undo(),
            Some("redo") => self.redo(),
            Some("save") => self.ask_save_version(),
            _ => {}
        }
    }

    pub fn start_details(&mut self) {
        let Some(open) = &self.open else {
            return;
        };
        let meta = &open.record.meta;
        self.form = DetailsForm {
            name: meta.name.clone(),
            diagram_type: Some(meta.diagram_type),
            description: meta.description.clone().unwrap_or_default(),
            colors: open.record.colors.clone(),
            tried: false,
            error: None,
        };
        let id = meta.id.clone();
        self.go(Page::Details(id));
    }

    pub fn ask_save_version(&mut self) {
        let Some(open) = &self.open else {
            return;
        };
        if open.status.saved {
            return;
        }
        let next = self.next_version(&open.record.meta.id);
        self.dialog = Some(Dialog::SaveVersion {
            description: String::new(),
            next,
        });
    }

    // History

    pub fn history_page(&mut self, ui: &mut Ui, id: &DiagramId) {
        let name = self
            .list
            .iter()
            .find(|s| s.meta.id == *id)
            .map(|s| s.meta.name.clone())
            .or_else(|| self.open.as_ref().map(|o| o.record.meta.name.clone()))
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.heading(RichText::new(format!("History of {name}")).font(fonts::bold(24.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Back").clicked() {
                    self.go_back();
                }
            });
        });
        ui.add_space(6.0);
        self.outcome_ui(ui);
        let status = self.status_of(id);
        if let Some(status) = status {
            status_badge(ui, status);
        }
        let versions = match self.store.list_versions(id) {
            Ok(v) => v,
            Err(e) => {
                ui.label(error_text(ui, &e.to_string()));
                return;
            }
        };
        if versions.is_empty() {
            ui.label("There are no saved versions.");
            return;
        }
        let unsaved = status.is_some_and(|s| !s.saved);
        ui.add_space(6.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("versions")
                .num_columns(4)
                .striped(true)
                .spacing([24.0, 10.0])
                .show(ui, |ui| {
                    for header in ["Version", "Saved", "Description", ""] {
                        ui.label(RichText::new(header).strong());
                    }
                    ui.end_row();
                    for v in versions {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("v{}", v.version)).strong());
                            if v.matches_current {
                                ui.label(RichText::new("matches current").italics().weak());
                            }
                        });
                        ui.label(date(&v.saved_at));
                        ui.label(v.description.clone().unwrap_or_else(|| "–".into()));
                        ui.horizontal(|ui| {
                            if !v.matches_current && ui.button("Revert").clicked() {
                                let next = self.next_version(id);
                                self.dialog = Some(Dialog::Revert {
                                    id: id.clone(),
                                    version: v.version,
                                    unsaved,
                                    next,
                                    save_description: String::new(),
                                    description: format!("Reverted to v{}", v.version),
                                });
                            }
                            ui.menu_button("Export", |ui| {
                                for format in
                                    [ExportFormat::Zip, ExportFormat::Svg, ExportFormat::Drawio]
                                {
                                    if ui.button(format.label()).clicked() {
                                        self.export(id, Some(v.version), format);
                                        ui.close();
                                    }
                                }
                            });
                            if ui.button("Delete").clicked() {
                                self.dialog = Some(Dialog::DeleteVersion {
                                    id: id.clone(),
                                    version: v.version,
                                });
                            }
                        });
                        ui.end_row();
                    }
                });
        });
    }

    // Dialogs

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        let mut next: Option<Box<dyn FnOnce(&mut App)>> = None;
        let mut dialog = dialog;
        let modal = egui::Modal::new(egui::Id::unique("dialog")).show(ctx, |ui| {
            ui.set_width(420.0);
            match &mut dialog {
                Dialog::DeleteDiagram { id, name } => {
                    ui.heading("Delete diagram?");
                    ui.label(format!("“{name}”, everything on it, and its history will be deleted. This can’t be undone."));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let cancel = ui.button("Cancel");
                        cancel.request_focus();
                        if cancel.clicked() {
                            keep = false;
                        }
                        if ui.button(RichText::new("Delete").color(ui.visuals().error_fg_color)).clicked() {
                            let (id, name) = (id.clone(), name.clone());
                            next = Some(Box::new(move |app| app.delete(&id, &name)));
                            keep = false;
                        }
                    });
                }
                Dialog::SaveVersion { description, next: number } => {
                    ui.heading(format!("Save version {number}"));
                    ui.label("Description (optional)");
                    let edit = ui.add(
                        egui::TextEdit::singleline(description)
                            .hint_text("Such as “Before the network redesign”")
                            .desired_width(f32::INFINITY),
                    );
                    if !edit.has_focus() && !ui.memory(|m| m.has_focus(edit.id)) {
                        edit.request_focus();
                    }
                    let valid = validate_version_description(description);
                    if let Err(message) = valid {
                        ui.label(error_text(ui, message));
                    }
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if (ui.button("Save version").clicked() || enter) && valid.is_ok() {
                            let text = description.clone();
                            next = Some(Box::new(move |app| app.save_version(&text)));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Revert {
                    id,
                    version,
                    unsaved,
                    next: number,
                    save_description,
                    description,
                } => {
                    ui.heading(format!("Revert to v{version}?"));
                    ui.label(format!("This records the content of v{version} as v{}.", if *unsaved { *number + 1 } else { *number }));
                    if *unsaved {
                        ui.label("Your current changes haven't been saved as a version. You can bring them back with Undo until you quit the app, or save them first.");
                        ui.label("Description for the save (optional)");
                        ui.add(egui::TextEdit::singleline(save_description).desired_width(f32::INFINITY));
                    }
                    ui.label("Description of the reverted version");
                    ui.add(egui::TextEdit::singleline(description).desired_width(f32::INFINITY));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let cancel = ui.button("Cancel");
                        if cancel.clicked() {
                            keep = false;
                        }
                        if *unsaved && ui.button(format!("Save as v{number} first")).clicked() {
                            let (id, version, save, text) =
                                (id.clone(), *version, save_description.clone(), description.clone());
                            next = Some(Box::new(move |app| {
                                app.save_unsaved(&id, &save);
                                app.revert(&id, version, &text);
                            }));
                            keep = false;
                        }
                        if ui.button("Revert").clicked() {
                            let (id, version, text) = (id.clone(), *version, description.clone());
                            next = Some(Box::new(move |app| app.revert(&id, version, &text)));
                            keep = false;
                        }
                    });
                }
                Dialog::DeleteVersion { id, version } => {
                    ui.heading(format!("Delete v{version}?"));
                    ui.label("This can’t be undone.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let cancel = ui.button("Cancel");
                        cancel.request_focus();
                        if cancel.clicked() {
                            keep = false;
                        }
                        if ui.button(RichText::new("Delete").color(ui.visuals().error_fg_color)).clicked() {
                            let (id, version) = (id.clone(), *version);
                            next = Some(Box::new(move |app| app.delete_version(&id, version)));
                            keep = false;
                        }
                    });
                }
            }
        });
        if modal.should_close() {
            keep = false;
        }
        if keep {
            self.dialog = Some(dialog);
        }
        if let Some(action) = next {
            action(self);
        }
    }

    /// Saves a diagram that's Unsaved as a version, whether or not it's open.
    pub fn save_unsaved(&mut self, id: &DiagramId, description: &str) {
        if self.open.as_ref().is_some_and(|o| o.record.meta.id == *id) {
            self.save_version(description);
            return;
        }
        let Ok(Some(record)) = self.store.get(id) else {
            return;
        };
        let description = Some(description.trim().to_owned()).filter(|d| !d.is_empty());
        if let Err(e) = self
            .worker
            .call(|reply| crate::worker::Request::SaveVersion {
                diagram: record.diagram,
                expected: record.revision,
                description,
                reply,
            })
        {
            self.outcome = Some(Outcome::error(format!(
                "The version couldn't be saved. {e}"
            )));
        }
    }

    /// Cmd+S, Cmd+Z, and Shift+Cmd+Z (or Ctrl+Y), unless typing in a field.
    pub fn shortcuts(&mut self, ctx: &egui::Context) {
        if !matches!(self.page, Page::Editor(_)) || self.dialog.is_some() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            self.ask_save_version();
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (redo, undo) = ctx.input_mut(|i| {
            let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || i.consume_key(Modifiers::CTRL, Key::Y);
            let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
            (redo, undo)
        });
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
    }
}
