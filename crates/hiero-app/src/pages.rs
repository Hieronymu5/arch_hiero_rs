//! The app's pages: the list of diagrams, the Add diagram and Edit details forms, the editor
//! with its toolbar, and a diagram's history; and the dialogs over them.

use egui::{Align, Color32, Key, Layout, Modifiers, RichText, Ui};
use hiero_core::history::validate_version_description;
use hiero_core::model::{DiagramId, DiagramTypeId, validate_description, validate_name};
use hiero_core::text::Weight;
use hiero_editor::fonts;
use hiero_store::Status;

use crate::app::{App, DetailsForm, Dialog, ExportFormat, Outcome, Page};
use crate::format::date;

/// Something to do once a dialog closes.
type Action = Box<dyn FnOnce(&mut App)>;

fn error_text(ui: &Ui, text: &str) -> RichText {
    RichText::new(text).color(ui.visuals().error_fg_color)
}

/// The prototype's colours for the current theme.
struct Palette {
    accent: Color32,
    on_accent: Color32,
    text: Color32,
    text_muted: Color32,
    input_border: Color32,
}

fn palette(ui: &Ui) -> Palette {
    if ui.visuals().dark_mode {
        Palette {
            accent: Color32::from_rgb(0x6c, 0xb2, 0xff),
            on_accent: Color32::from_rgb(0x0f, 0x12, 0x15),
            text: Color32::from_rgb(0xe6, 0xe9, 0xed),
            text_muted: Color32::from_rgb(0xa3, 0xac, 0xb6),
            input_border: Color32::from_rgb(0x8b, 0x94, 0x9e),
        }
    } else {
        Palette {
            accent: Color32::from_rgb(0x0b, 0x5c, 0xad),
            on_accent: Color32::WHITE,
            text: Color32::from_rgb(0x1b, 0x1f, 0x24),
            text_muted: Color32::from_rgb(0x57, 0x60, 0x6a),
            input_border: Color32::from_rgb(0x6e, 0x77, 0x81),
        }
    }
}

/// A page action styled like the prototype's `.button`: an accent border, and filled with the
/// accent when it's the main action. It looks the same in every state, so hovering doesn't shift
/// its text.
fn page_button(ui: &mut Ui, text: &str, main: bool) -> egui::Response {
    accent_button(ui, text, main, egui::vec2(12.0, 5.0), 14.0)
}

/// A button in a row of the list, styled like the prototype's `.button-secondary` but at the
/// padding of egui's buttons and smaller text.
fn row_button(ui: &mut Ui, text: &str) -> egui::Response {
    let padding = ui.spacing().button_padding;
    accent_button(ui, text, false, padding, 12.0)
}

/// An accent-bordered button with the given padding and font size.
fn accent_button(
    ui: &mut Ui,
    text: &str,
    main: bool,
    padding: egui::Vec2,
    font_size: f32,
) -> egui::Response {
    let Palette {
        accent,
        on_accent,
        text: text_color,
        ..
    } = palette(ui);
    // The border turns white on hover, or the text colour in the light theme, where white
    // wouldn't show.
    let hover_border = if ui.visuals().dark_mode {
        Color32::WHITE
    } else {
        text_color
    };
    let (fill, color) = if main {
        (accent, on_accent)
    } else {
        (Color32::TRANSPARENT, accent)
    };
    ui.scope(|ui| {
        let style = ui.style_mut();
        style.spacing.button_padding = padding;
        let widgets = &mut style.visuals.widgets;
        // An outlined button's text follows its border; a filled one's stays readable on the fill.
        let hover_text = if main { color } else { hover_border };
        for (state, border, text) in [
            (&mut widgets.inactive, accent, color),
            (&mut widgets.hovered, hover_border, hover_text),
            (&mut widgets.active, hover_border, hover_text),
        ] {
            state.weak_bg_fill = fill;
            state.bg_stroke = egui::Stroke::new(1.0, border);
            state.fg_stroke.color = text;
            state.corner_radius = egui::CornerRadius::same(6);
            state.expansion = 0.0;
        }
        let label = RichText::new(text).font(fonts::font_id(font_size, Weight::SemiBold));
        let response = ui.add(egui::Button::new(label));
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response
    })
    .inner
}

/// The prototype's theme picker: a monitor, a sun, and a moon in a bordered group, with the
/// chosen one filled with the accent. Returns the theme clicked, if any.
fn theme_picker(ui: &mut Ui, current: egui::ThemePreference) -> Option<egui::ThemePreference> {
    const CELL: f32 = 28.0;
    const GAP: f32 = 2.0;
    let colors = palette(ui);
    let themes = [
        (egui::ThemePreference::System, "System theme"),
        (egui::ThemePreference::Light, "Light theme"),
        (egui::ThemePreference::Dark, "Dark theme"),
    ];
    // A 1px border, 2px of padding, and the cells 2px apart.
    let inset = 1.0 + GAP;
    let size = egui::vec2(2.0 * inset + 3.0 * CELL + 2.0 * GAP, 2.0 * inset + CELL);
    let (group, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_stroke(
        group,
        egui::CornerRadius::same(6),
        egui::Stroke::new(1.0, colors.input_border),
        egui::StrokeKind::Inside,
    );
    let mut chosen = None;
    for (i, (theme, label)) in themes.into_iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "three cells")]
        let left = group.left() + inset + i as f32 * (CELL + GAP);
        let cell = egui::Rect::from_min_size(
            egui::pos2(left, group.top() + inset),
            egui::vec2(CELL, CELL),
        );
        let response = ui
            .interact(cell, ui.scope_id().with(("theme", i)), egui::Sense::click())
            .on_hover_text(label)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = current == theme;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, label)
        });
        let painter = ui.painter();
        let color = if selected {
            painter.rect_filled(cell, egui::CornerRadius::same(4), colors.accent);
            colors.on_accent
        } else if response.hovered() {
            colors.text
        } else {
            colors.text_muted
        };
        if response.has_focus() {
            painter.rect_stroke(
                cell.expand(2.0),
                egui::CornerRadius::same(6),
                egui::Stroke::new(2.0, colors.accent),
                egui::StrokeKind::Outside,
            );
        }
        theme_icon(painter, cell.center(), theme, color);
        if response.clicked() {
            chosen = Some(theme);
        }
    }
    chosen
}

/// Paints a theme's 18px icon, drawn on the prototype's 24-unit grid with 2-unit lines.
fn theme_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    theme: egui::ThemePreference,
    color: Color32,
) {
    let scale = 18.0 / 24.0;
    let at = |x: f32, y: f32| center + egui::vec2(x - 12.0, y - 12.0) * scale;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let line = |from: (f32, f32), to: (f32, f32)| {
        painter.line_segment([at(from.0, from.1), at(to.0, to.1)], stroke);
    };
    match theme {
        egui::ThemePreference::System => {
            painter.rect_stroke(
                egui::Rect::from_min_max(at(3.0, 4.0), at(21.0, 16.0)),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
            line((8.0, 20.0), (16.0, 20.0));
            line((12.0, 16.0), (12.0, 20.0));
        }
        egui::ThemePreference::Light => {
            painter.circle_stroke(center, 4.0 * scale, stroke);
            for (from, to) in [
                ((12.0, 2.0), (12.0, 4.0)),
                ((12.0, 20.0), (12.0, 22.0)),
                ((4.9, 4.9), (6.3, 6.3)),
                ((17.7, 17.7), (19.1, 19.1)),
                ((2.0, 12.0), (4.0, 12.0)),
                ((20.0, 12.0), (22.0, 12.0)),
                ((4.9, 19.1), (6.3, 17.7)),
                ((17.7, 6.3), (19.1, 4.9)),
            ] {
                line(from, to);
            }
        }
        egui::ThemePreference::Dark => {
            // The crescent's two arcs, from `M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z`:
            // the outer one around (12.64, 11.36), and the inner one, a half circle whose
            // radius SVG stretches to reach its ends, around (14.75, 9.25).
            let arc = |cx: f32, cy: f32, r: f32, start: f32, sweep: f32| {
                (0..=16).map(move |i| {
                    #[expect(clippy::cast_precision_loss, reason = "small counts")]
                    let angle = start + sweep * i as f32 / 16.0;
                    (cx + r * angle.cos(), cy + r * angle.sin())
                })
            };
            let points = arc(12.6435, 11.3565, 8.0, 0.4038, 3.9048)
                .chain(arc(14.75, 9.25, 7.4246, -2.3562, -std::f32::consts::PI).skip(1))
                .map(|(x, y)| at(x, y))
                .collect();
            painter.add(egui::Shape::closed_line(points, stroke));
        }
    }
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
    /// The page's title for the top bar, with a description under it: the diagram's name and
    /// description in the editor, and Diagrams on the list. Other pages have their own heading.
    fn page_heading(&self) -> Option<(String, Option<String>)> {
        match &self.page {
            Page::List => Some(("Diagrams".to_owned(), None)),
            Page::Editor(id) => Some(
                self.open
                    .as_ref()
                    .filter(|o| o.record.meta.id == *id)
                    .map_or_else(
                        || ("Diagram".to_owned(), None),
                        |o| {
                            let meta = &o.record.meta;
                            (meta.name.clone(), meta.description.clone())
                        },
                    ),
            ),
            _ => None,
        }
    }

    pub fn top_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("top-bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                // White on transparent, tinted to the theme's text colour: black or white.
                let tint = if ui.visuals().dark_mode {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                };
                let logo = ui
                    .add(
                        egui::Image::new(egui::include_image!("../assets/logo.png"))
                            .fit_to_exact_size(egui::vec2(60.0, 45.0))
                            .tint(tint)
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                logo.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Link, true, "ArchHiero, go to diagrams")
                });
                if logo.clicked() && !matches!(self.page, Page::List) {
                    self.flush_save();
                    self.back.clear();
                    self.page = Page::List;
                    self.refresh_list();
                }
                if let Some((title, description)) = self.page_heading() {
                    // In the editor, the diagram's Saved/Unsaved badge goes next to its name.
                    let status = match &self.page {
                        Page::Editor(id) => self
                            .open
                            .as_ref()
                            .filter(|o| o.record.meta.id == *id)
                            .map(|o| o.status),
                        _ => None,
                    };
                    ui.add_space(8.0);
                    // The title on one line with the description under it, each cut short when
                    // there isn't room. The description's full text shows on hover.
                    let room = (ui.available_width() - 260.0).max(120.0);
                    ui.allocate_ui(egui::vec2(room, 45.0), |ui| {
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            let title = RichText::new(title)
                                .font(fonts::bold(18.0))
                                .color(ui.visuals().strong_text_color());
                            ui.horizontal(|ui| {
                                // Leave room for the badge, about 90 points wide.
                                let badge_room = if status.is_some() { 90.0 } else { 0.0 };
                                ui.scope(|ui| {
                                    ui.set_max_width(ui.available_width() - badge_room);
                                    ui.add(egui::Label::new(title).truncate());
                                });
                                if let Some(status) = status {
                                    status_badge(ui, status);
                                }
                            });
                            if let Some(description) = description.filter(|d| !d.is_empty()) {
                                ui.add(
                                    egui::Label::new(RichText::new(&description).weak()).truncate(),
                                )
                                .on_hover_text(description);
                            }
                        });
                    });
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(theme) = theme_picker(ui, self.theme) {
                        self.theme = theme;
                        ui.ctx().set_theme(theme);
                    }
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
            let error = outcome.error;
            ui.horizontal(|ui| {
                // Announced politely when it worked, and at once when it didn't.
                let message = ui.label(text);
                ui.ctx().accesskit_node_builder(message.id, |node| {
                    node.set_live(if error {
                        egui::accesskit::Live::Assertive
                    } else {
                        egui::accesskit::Live::Polite
                    });
                });
                if ui.small_button("Dismiss").clicked() {
                    self.outcome = None;
                }
            });
            ui.add_space(6.0);
        }
    }

    // The list of diagrams

    pub fn list_page(&mut self, ui: &mut Ui) {
        // The top bar shows the title, Diagrams.
        ui.horizontal(|ui| {
            if page_button(ui, "Add Diagram", true).clicked() {
                self.form = DetailsForm::default();
                self.go(Page::New);
            }
            if page_button(ui, "Import Diagram", false).clicked() {
                self.import();
            }
        });
        ui.add_space(8.0);
        self.outcome_ui(ui);
        if self.list.is_empty() {
            ui.label("No diagrams have been added yet.");
            return;
        }
        let rows = self.list.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("diagrams")
                .num_columns(5)
                .striped(true)
                .spacing([24.0, 10.0])
                .show(ui, |ui| {
                    let size = egui::TextStyle::Body.resolve(ui.style()).size;
                    for header in ["Name", "Type", "Last updated", "Status", ""] {
                        ui.label(
                            RichText::new(header)
                                .font(fonts::bold(size))
                                .color(ui.visuals().strong_text_color()),
                        );
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
                            if row_button(ui, "History").clicked() {
                                self.go(Page::History(id.clone()));
                            }
                            if row_button(ui, "Export").clicked() {
                                self.export(&id, None, ExportFormat::Zip);
                            }
                            if row_button(ui, "Delete").clicked() {
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
            if page_button(ui, "Add Diagram", true).clicked() {
                self.form.tried = true;
                if valid {
                    self.create();
                }
            }
            if page_button(ui, "Cancel", false).clicked() {
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
                if page_button(ui, "Save Changes", true).clicked() {
                    self.form.tried = true;
                    if valid {
                        self.save_details();
                    }
                }
                if page_button(ui, "Cancel", false).clicked() {
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
        if self.open.as_ref().is_none_or(|o| o.record.meta.id != *id) {
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
            // The top bar shows the name, description, and status.
            ui.horizontal(|ui| {
                if page_button(ui, "Back", true).clicked() {
                    action = Some("back");
                }
                let save =
                    ui.add_enabled_ui(!open.status.saved, |ui| page_button(ui, "Save", false));
                if save
                    .inner
                    .on_hover_text("Record a version (Cmd+S)")
                    .clicked()
                {
                    action = Some("save");
                }
                let undo = ui.add_enabled_ui(can_undo, |ui| page_button(ui, "Undo", false));
                if undo.inner.on_hover_text("Cmd+Z").clicked() {
                    action = Some("undo");
                }
                let redo = ui.add_enabled_ui(can_redo, |ui| page_button(ui, "Redo", false));
                if redo.inner.on_hover_text("Shift+Cmd+Z").clicked() {
                    action = Some("redo");
                }
                if page_button(ui, "History", false).clicked() {
                    action = Some("history");
                }
                // Filled while the help shows.
                let help = open.editor.show_help();
                if page_button(ui, "Help", *help).clicked() {
                    *help = !*help;
                }
                let has_content = open.editor.has_content();
                let svg = ui.add_enabled_ui(has_content, |ui| page_button(ui, "Export SVG", false));
                if svg.inner.clicked() {
                    action = Some("svg");
                }
                let drawio =
                    ui.add_enabled_ui(has_content, |ui| page_button(ui, "Export draw.io", false));
                if drawio.inner.clicked() {
                    action = Some("drawio");
                }
                if page_button(ui, "Edit details", false).clicked() {
                    action = Some("details");
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} diagram", meta.diagram_type.label()))
                            .font(fonts::bold(16.0))
                            .color(ui.visuals().strong_text_color()),
                    );
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
        let mut next: Option<Action> = None;
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
