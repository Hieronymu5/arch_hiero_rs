//! Names and arrow text typed in place on the canvas.
//!
//! While editing, double-clicking an element, or pressing F2 on it, opens a field over its name
//! to type a new one: over its header for a container, otherwise across its middle.
//! Double-clicking an arrow's text, connector, or transformation, or a C4 relationship's
//! description, opens a field over that text, and double-clicking an arrow's line opens its
//! text, so one with none can be given some. Enter or moving away saves, and Escape leaves it
//! as it was. The same rules as the properties panel's fields apply, and a problem shows under
//! the field; moving away from text that can't be saved puts the old text back.

use egui::{Color32, Key, Pos2, RichText, Stroke, Ui, Vec2};
use hiero_core::engine::{Engine, validate_label};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::ArrowText;
use hiero_core::model::{ConnectionId, ElementId};

use super::{DiagramView, Editor, Focus, distance_to_segment};
use crate::paint::Transform;
use crate::theme::palette;
use crate::widgets;

/// How tall the field is, in canvas pixels, and the least it's wide.
const FIELD_HEIGHT: f64 = 36.0;
const MIN_WIDTH: f64 = 160.0;

/// What's being typed in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InlineTarget {
    /// An element's name.
    Name(ElementId),
    /// One of an arrow's texts.
    Arrow(ConnectionId, ArrowText),
}

/// The text being typed in place, where its field goes, and what's wrong with it.
#[derive(Debug, Clone)]
pub(super) struct InlineEdit {
    pub target: InlineTarget,
    /// The text as it was.
    before: String,
    text: String,
    /// Where the field goes, in canvas pixels.
    rect: Rect,
    error: Option<String>,
    /// Set until the field has been focused, with its text selected.
    opening: bool,
}

/// How a field typed in place was left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leave {
    /// Enter: saved, or kept open to say what's wrong.
    Enter,
    /// Moving away: saved, or put back as it was when it can't be.
    Away,
    /// Escape: put back as it was.
    Cancel,
}

impl InlineEdit {
    fn new(target: InlineTarget, before: String, rect: Rect) -> Self {
        Self {
            target,
            text: before.clone(),
            before,
            rect,
            error: None,
            opening: true,
        }
    }
}

impl<T: DiagramView> Editor<T> {
    /// Opens a field over an element's name, to type a new one in place.
    pub(super) fn start_name_edit(&mut self, id: &ElementId) {
        if self.viewing || self.tag.applying.is_some() {
            return;
        }
        let Some(element) = self.eng.layout().iter().find(|e| e.id() == id) else {
            return;
        };
        let r = element.rect;
        let top = if element.is_container {
            r.y + ((element.header.min(r.height) - FIELD_HEIGHT) / 2.0).max(0.0)
        } else {
            r.y + (r.height - FIELD_HEIGHT) / 2.0
        };
        // Inside its sides, but never narrower than an arrow's, so a figure's name fits.
        let inset = (r.width / 10.0).min(8.0);
        let width = (r.width - inset * 2.0).max(MIN_WIDTH);
        let rect = Rect::new(r.center().x - width / 2.0, top, width, FIELD_HEIGHT);
        let label = element.el.label.clone();
        self.inline = Some(InlineEdit::new(InlineTarget::Name(id.clone()), label, rect));
    }

    /// Opens a field over an arrow's text `field`, drawn over `over`, to type it in place.
    fn start_arrow_edit(&mut self, id: &ConnectionId, field: ArrowText, over: Rect) {
        let Some(connection) = self.eng.connection(id) else {
            return;
        };
        let before = T::arrow_text(&connection.props, field);
        let width = (over.width + 16.0).max(MIN_WIDTH);
        let center = over.center();
        let rect = Rect::new(
            center.x - width / 2.0,
            center.y - FIELD_HEIGHT / 2.0,
            width,
            FIELD_HEIGHT,
        );
        self.eng.select_connection(id);
        self.inline = Some(InlineEdit::new(
            InlineTarget::Arrow(id.clone(), field),
            before,
            rect,
        ));
    }

    /// A double-click on the canvas at `world`, while editing: on an arrow's text, its line, or
    /// an element, it opens a field to type it in place. Arrows are picked first, as a press
    /// picks them.
    pub(super) fn double_click(&mut self, pos: Pos2, world: Point, xf: Transform) {
        if self.viewing || self.tag.applying.is_some() {
            return;
        }
        let hidden = self.tag_hidden();
        let shown = |id: &ConnectionId| {
            self.eng
                .connection(id)
                .is_some_and(|c| !hidden.contains(&c.source_id) && !hidden.contains(&c.target_id))
        };
        let layout = self.eng.layout();
        let text = T::arrow_texts(&self.eng, layout)
            .into_iter()
            .rev()
            .find(|(id, _, r)| shown(id) && r.contains_point(world));
        if let Some((id, field, over)) = text {
            self.start_arrow_edit(&id, field, over);
            return;
        }
        let line = T::arrow_lines(&self.eng, layout, None)
            .into_iter()
            .rev()
            .filter(|l| shown(&l.id))
            .find(|l| {
                l.boxes.iter().any(|b| b.contains_point(world))
                    || distance_to_segment(pos, xf.pos(l.from), xf.pos(l.to)) <= 6.0
            });
        if let Some(line) = line {
            if let Some(field) = T::LINE_TEXT {
                // Over the middle of the line.
                let middle = Point::new(
                    f64::midpoint(line.from.x, line.to.x),
                    f64::midpoint(line.from.y, line.to.y),
                );
                self.start_arrow_edit(&line.id, field, Rect::new(middle.x, middle.y, 0.0, 0.0));
            }
            return;
        }
        if let Some(id) = Self::element_at(&self.eng, world).map(|e| e.id().clone())
            && !hidden.contains(&id)
        {
            self.start_name_edit(&id);
        }
    }

    /// Shows the field typed in place, over the canvas, and saves or closes it as it's left.
    pub(super) fn inline_ui(&mut self, ui: &mut Ui, xf: Transform, canvas_id: egui::Id) {
        let Some(edit) = self.inline.as_mut() else {
            return;
        };
        let id = self.salt.with("inline");
        let edit_id = id.with("field");
        let screen = xf.rect(edit.rect);
        let colors = palette(ui);
        let size = (14.0 * xf.zoom).clamp(11.0, 20.0);
        let response = ui
            .scope_builder(egui::UiBuilder::new().max_rect(screen), |ui| {
                ui.set_clip_rect(ui.ctx().content_rect());
                egui::Frame::new()
                    .fill(colors.surface)
                    .stroke(Stroke::new(2.0, colors.accent))
                    .corner_radius(4)
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: Color32::from_black_alpha(50),
                    })
                    .inner_margin(egui::Margin::symmetric(6, 0))
                    .show(ui, |ui| {
                        ui.set_min_size(Vec2::new(screen.width() - 16.0, screen.height() - 4.0));
                        let field = egui::TextEdit::singleline(&mut edit.text)
                            .id(edit_id)
                            .frame(egui::Frame::NONE)
                            .align(egui::Align2::CENTER_CENTER)
                            .font(crate::fonts::bold(size))
                            .desired_width(f32::INFINITY);
                        ui.centered_and_justified(|ui| ui.add(field)).inner
                    })
                    .inner
            })
            .inner;
        let label = match &edit.target {
            InlineTarget::Name(_) => "Name",
            InlineTarget::Arrow(_, field) => field.name(),
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
        if let Some(message) = &edit.error {
            let below = egui::Rect::from_min_size(
                Pos2::new(screen.left(), screen.bottom() + 4.0),
                Vec2::new(screen.width(), 20.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(below), |ui| {
                egui::Frame::new()
                    .fill(colors.surface)
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(message)
                                    .color(widgets::error_color(ui))
                                    .small(),
                            );
                        });
                    });
            });
        }
        if edit.opening {
            edit.opening = false;
            response.request_focus();
            let mut state = egui::TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::select_all_str(&edit.text)));
            state.store(ui.ctx(), edit_id);
            return;
        }
        if !response.lost_focus() {
            return;
        }
        let leave = ui.input(|i| {
            if i.key_pressed(Key::Escape) {
                Leave::Cancel
            } else if i.key_pressed(Key::Enter) {
                Leave::Enter
            } else {
                Leave::Away
            }
        });
        self.leave_inline(ui, leave, &response, canvas_id);
    }

    /// Saves the text typed in place, or puts it back, as it's left.
    fn leave_inline(
        &mut self,
        ui: &Ui,
        leave: Leave,
        response: &egui::Response,
        canvas_id: egui::Id,
    ) {
        let Some(edit) = self.inline.take() else {
            return;
        };
        let problem = if leave == Leave::Cancel {
            None
        } else {
            self.save_inline(ui, &edit)
        };
        if let Some(problem) = problem
            && leave == Leave::Enter
        {
            // Kept open to say what's wrong.
            response.request_focus();
            self.inline = Some(InlineEdit {
                error: Some(problem),
                ..edit
            });
            return;
        }
        // Back on the canvas: on the element renamed, or on the canvas for an arrow.
        if leave != Leave::Away {
            ui.memory_mut(|m| m.request_focus(canvas_id));
        }
        self.focus = match edit.target {
            InlineTarget::Name(id) if self.eng.element(&id).is_some() => Focus::Element(id),
            _ => Focus::Canvas,
        };
    }

    /// Saves the text typed in place, as the properties panel's field would, or says what's
    /// wrong with it.
    fn save_inline(&mut self, ui: &Ui, edit: &InlineEdit) -> Option<String> {
        let text = edit.text.trim().to_owned();
        match &edit.target {
            InlineTarget::Name(id) => {
                let element = self.eng.layout().iter().find(|e| e.id() == id)?;
                let mut with = element.el.clone();
                with.label.clone_from(&text);
                if let Err(problem) =
                    validate_label(&text).and_then(|()| T::validate_element(&with))
                {
                    return Some(problem);
                }
                if text == edit.before {
                    return None;
                }
                let parent = element.el.parent_id.clone();
                let target = id.clone();
                self.act(None, move |eng: &mut Engine<T>| {
                    T::apply_element(eng, &target, with, parent);
                });
                self.eng.announcement = format!("Renamed {} to {text}.", edit.before);
                // The properties read the name again.
                widgets::reset_field(ui, self.salt.with("fields").with((id.as_str(), "label")));
                None
            }
            InlineTarget::Arrow(id, field) => {
                let connection = self.eng.connection(id)?;
                let arrow = T::with_arrow_text(&connection.props, *field, &text);
                if let Err(problem) = T::validate_arrow(&arrow) {
                    return Some(problem);
                }
                if text == edit.before {
                    return None;
                }
                let target = id.clone();
                self.act(None, move |eng: &mut Engine<T>| {
                    eng.set_connection_props(&target, arrow);
                });
                self.eng.announcement = if text.is_empty() {
                    format!("{} removed.", field.name())
                } else {
                    format!("{} changed to {text}.", field.name())
                };
                // The arrow's properties read it again.
                widgets::reset_field(
                    ui,
                    self.salt
                        .with("fields")
                        .with(("arrow", id.as_str(), field.key())),
                );
                None
            }
        }
    }
}
