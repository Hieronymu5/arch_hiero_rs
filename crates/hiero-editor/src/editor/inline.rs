//! Names, descriptions, and arrow text typed in place on the canvas.
//!
//! While editing, double-clicking an element's name, or pressing F2 on it, opens a field over
//! its name to type a new one; double-clicking its description opens a field over that, over
//! several lines, with Shift+Enter for a new line. Double-clicking an arrow's text, connector,
//! or transformation, or a C4 relationship's description, opens a field over that text, and
//! double-clicking an arrow's line opens its text, so one with none can be given some.
//!
//! The field sits over the text it replaces, a little bigger, in its size, weight, and
//! alignment, and kept within the element; the text itself is hidden meanwhile, so it isn't
//! shown twice. With no text to go over, it takes a box of its own. Enter or moving away saves,
//! and Escape leaves it as it was. The same rules as the properties panel's fields apply, and a
//! problem shows under the field; moving away from text that can't be saved puts the old text
//! back.

use egui::{Color32, Key, Modifiers, Pos2, RichText, Stroke, Ui, Vec2};
use hiero_core::engine::{Engine, validate_label};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::ArrowText;
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::scene::{Anchor, PlacedText, placed_texts};
use hiero_core::text::{MeasureText, Weight};

use super::{DiagramView, Editor, Focus, distance_to_segment};
use crate::paint::Transform;
use crate::theme::palette;
use crate::widgets;

/// The room kept round the text typed over, across and down, in canvas pixels: a one-line
/// field's, then a description's.
const PAD: (f64, f64) = (6.0, 3.0);
const PAD_MULTILINE: (f64, f64) = (6.0, 4.0);

/// How the text typed over is drawn, for the field to match.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TextLook {
    /// In canvas pixels.
    size: f64,
    weight: Weight,
    /// Whether it starts at its left, rather than being centred.
    start: bool,
}

/// Where a field goes, when the text it types over is drawn somewhere.
struct Place {
    /// The box it takes when there's no text to go over, such as an arrow with none yet.
    fallback: Rect,
    min_width: f64,
    min_height: f64,
    /// Across where it can be, as the element it's in.
    within: Option<(f64, f64)>,
    multiline: bool,
    /// The text it holds, on one line, which it's at least as wide as: a name drawn on two
    /// lines is typed on one.
    fit: Option<String>,
}

/// Where the field for text typed over `parts`, each where it's drawn, goes, and how its text
/// looks: over the parts, a little bigger, drawn as they are, and kept within `place.within`.
fn field_over(
    parts: &[(PlacedText, Rect)],
    place: &Place,
    measure: &dyn MeasureText,
) -> (Rect, Option<TextLook>) {
    let Some((first, _)) = parts.first() else {
        return (place.fallback, None);
    };
    let left = parts.iter().map(|(_, r)| r.x).fold(f64::MAX, f64::min);
    let top = parts.iter().map(|(_, r)| r.y).fold(f64::MAX, f64::min);
    let right = parts
        .iter()
        .map(|(_, r)| r.right())
        .fold(f64::MIN, f64::max);
    let bottom = parts
        .iter()
        .map(|(_, r)| r.bottom())
        .fold(f64::MIN, f64::max);
    let look = TextLook {
        size: first.size,
        weight: first.weight,
        start: first.anchor == Anchor::Start,
    };
    let (pad_x, pad_y) = if place.multiline { PAD_MULTILINE } else { PAD };
    let one_line = place
        .fit
        .as_deref()
        .map_or(0.0, |text| measure.width(text, look.size, look.weight));
    let text_width = (right - left).max(one_line) + pad_x * 2.0;
    let mut width = text_width.max(place.min_width);
    let mut x = if look.start {
        left - pad_x
    } else {
        f64::midpoint(left, right) - width / 2.0
    };
    if let Some((within_x, within_width)) = place.within {
        width = width.min(within_width.max(text_width));
        x = x.min(within_x + within_width - width).max(within_x);
    }
    let height = (bottom - top + pad_y * 2.0).max(place.min_height);
    let y = if place.multiline {
        top - pad_y
    } else {
        f64::midpoint(top, bottom) - height / 2.0
    };
    (Rect::new(x, y, width, height), Some(look))
}

/// A description's line as written: without the bullet it may be drawn with.
fn unbulleted(text: &str) -> &str {
    text.trim().trim_start_matches(['•', '-']).trim()
}

/// What's being typed in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InlineTarget {
    /// An element's name.
    Name(ElementId),
    /// An element's description.
    Description(ElementId),
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
    /// How the text typed over is drawn, when it's drawn somewhere.
    look: Option<TextLook>,
    /// The text typed over, hidden meanwhile.
    pub hidden: Vec<PlacedText>,
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
    fn new(
        target: InlineTarget,
        before: String,
        parts: Vec<(PlacedText, Rect)>,
        place: &Place,
        measure: &dyn MeasureText,
    ) -> Self {
        let (rect, look) = field_over(&parts, place, measure);
        Self {
            target,
            text: before.clone(),
            before,
            rect,
            look,
            hidden: parts.into_iter().map(|(t, _)| t).collect(),
            error: None,
            opening: true,
        }
    }

    fn multiline(&self) -> bool {
        matches!(self.target, InlineTarget::Description(_))
    }
}

impl<T: DiagramView> Editor<T> {
    /// The text drawn now, each piece with the box it takes.
    fn drawn_texts(&mut self) -> Vec<(PlacedText, Rect)> {
        let texts = placed_texts(&self.still_scene().nodes);
        let measure = self.eng.measure();
        texts
            .into_iter()
            .filter(|t| !t.text.trim().is_empty())
            .map(|t| {
                let rect = t.rect(measure);
                (t, rect)
            })
            .collect()
    }

    /// Opens a field over an element's name to type a new one in place, or over its
    /// description when that's what's at `pointer`.
    pub(super) fn start_name_edit(&mut self, id: &ElementId, pointer: Option<Point>) {
        if self.viewing || self.tag.applying.is_some() {
            return;
        }
        let Some(element) = self.eng.layout().iter().find(|e| e.id() == id).cloned() else {
            return;
        };
        let area = T::drawn_rect(&self.eng, &element);
        // What's drawn inside it is the text of the elements it holds.
        let inner: Vec<Rect> = self
            .eng
            .layout()
            .iter()
            .filter(|e| e.id() != id)
            .map(|e| T::drawn_rect(&self.eng, e))
            .filter(|r| area.contains_rect(r))
            .collect();
        let mine: Vec<(PlacedText, Rect)> = self
            .drawn_texts()
            .into_iter()
            .filter(|(_, r)| {
                let c = r.center();
                area.contains_point(c) && !inner.iter().any(|i| i.contains_point(c))
            })
            .collect();
        let label = element.el.label.clone();
        let description = element.el.description.clone().unwrap_or_default();
        let in_label = |t: &PlacedText| label.contains(t.text.trim());
        // The name is drawn bold; failing that, its words.
        let bold = mine
            .iter()
            .any(|(t, _)| t.weight != Weight::Regular && in_label(t));
        let is_name = |t: &PlacedText| in_label(t) && (!bold || t.weight != Weight::Regular);
        let r = element.rect;
        let within = Some((r.x + 2.0, r.width - 4.0));
        let inset = (r.width / 10.0).min(8.0);
        let width = r.width - inset * 2.0;
        // The description, when it's what was double-clicked: any of the element's text that
        // isn't its name.
        let on_description = element.info.has_description
            && pointer.is_some_and(|p| {
                mine.iter()
                    .any(|(t, rect)| !is_name(t) && rect.expand(2.0).contains_point(p))
            });
        if on_description {
            let parts: Vec<_> = mine
                .iter()
                .filter(|(t, _)| {
                    !is_name(t)
                        && !description.is_empty()
                        && description.contains(unbulleted(&t.text))
                })
                .cloned()
                .collect();
            let height = 64.0;
            let place = Place {
                fallback: Rect::new(r.x + inset, r.center().y - height / 2.0, width, height),
                min_width: (r.width - 4.0).min(160.0),
                min_height: 44.0,
                within,
                multiline: true,
                fit: None,
            };
            self.inline = Some(InlineEdit::new(
                InlineTarget::Description(id.clone()),
                description,
                parts,
                &place,
                self.eng.measure(),
            ));
            return;
        }
        let height = 32.0;
        let top = if element.is_container {
            r.y + ((element.header.min(r.height) - height) / 2.0).max(0.0)
        } else {
            r.y + (r.height - height) / 2.0
        };
        let parts: Vec<_> = mine.into_iter().filter(|(t, _)| is_name(t)).collect();
        let place = Place {
            fallback: Rect::new(r.x + inset, top, width, height),
            min_width: (r.width - 4.0).min(120.0),
            min_height: 0.0,
            within,
            multiline: false,
            fit: Some(label.clone()),
        };
        self.inline = Some(InlineEdit::new(
            InlineTarget::Name(id.clone()),
            label,
            parts,
            &place,
            self.eng.measure(),
        ));
    }

    /// Opens a field over an arrow's text `field`, drawn over `over`, to type it in place.
    fn start_arrow_edit(&mut self, id: &ConnectionId, field: ArrowText, over: Rect) {
        let Some(connection) = self.eng.connection(id) else {
            return;
        };
        let before = T::arrow_text(&connection.props, field);
        // The text itself, where it's drawn.
        let parts: Vec<_> = if before.is_empty() {
            Vec::new()
        } else {
            self.drawn_texts()
                .into_iter()
                .filter(|(t, r)| {
                    over.expand(2.0).contains_point(r.center()) && before.contains(t.text.trim())
                })
                .collect()
        };
        // Or, for an arrow with none yet, on the middle of the line.
        let centre = over.center();
        let place = Place {
            fallback: Rect::new(centre.x - 80.0, centre.y - 16.0, 160.0, 32.0),
            min_width: 120.0,
            min_height: 0.0,
            within: None,
            multiline: false,
            fit: Some(before.clone()),
        };
        self.eng.select_connection(id);
        self.inline = Some(InlineEdit::new(
            InlineTarget::Arrow(id.clone(), field),
            before,
            parts,
            &place,
            self.eng.measure(),
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
            self.start_name_edit(&id, Some(world));
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
        let multiline = edit.multiline();
        // Drawn as the text typed over is, or as a name, or a description, would be.
        let look = edit.look.unwrap_or(TextLook {
            size: if multiline { 13.0 } else { 14.0 },
            weight: if multiline {
                Weight::Regular
            } else {
                Weight::Bold
            },
            start: false,
        });
        #[expect(clippy::cast_possible_truncation, reason = "a font size")]
        let size = ((look.size as f32) * xf.zoom).max(8.0);
        let font = crate::fonts::font_id(size, look.weight);
        let align = match (look.start, multiline) {
            (true, true) => egui::Align2::LEFT_TOP,
            (true, false) => egui::Align2::LEFT_CENTER,
            (false, true) => egui::Align2::CENTER_TOP,
            (false, false) => egui::Align2::CENTER_CENTER,
        };
        // In a description, Enter saves, as in a one-line field, and Shift+Enter starts a new
        // line.
        let enter = multiline
            && ui.memory(|m| m.has_focus(edit_id))
            // Which takes Shift+Enter too, so it's looked at first.
            && ui.input_mut(|i| !i.modifiers.shift && i.consume_key(Modifiers::NONE, Key::Enter));
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
                    // With the 2px border, the room kept round the text typed over.
                    .inner_margin(egui::Margin::symmetric(4, if multiline { 2 } else { 0 }))
                    .show(ui, |ui| {
                        let inner = Vec2::new(
                            screen.width() - 12.0,
                            screen.height() - if multiline { 8.0 } else { 4.0 },
                        );
                        ui.set_min_size(inner);
                        let field = if multiline {
                            egui::TextEdit::multiline(&mut edit.text).desired_rows(1)
                        } else {
                            egui::TextEdit::singleline(&mut edit.text)
                        }
                        .id(edit_id)
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .align(align)
                        .font(font)
                        .desired_width(inner.x);
                        ui.add_sized(inner, field)
                    })
                    .inner
            })
            .inner;
        let label = match &edit.target {
            InlineTarget::Name(_) => "Name",
            InlineTarget::Description(_) => "Description",
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
        if enter {
            self.leave_inline(ui, Leave::Enter, &response, canvas_id);
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
            InlineTarget::Name(id) | InlineTarget::Description(id)
                if self.eng.element(&id).is_some() =>
            {
                Focus::Element(id)
            }
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
            InlineTarget::Description(id) => {
                let element = self.eng.layout().iter().find(|e| e.id() == id)?;
                let mut with = element.el.clone();
                with.description = (!text.is_empty()).then(|| text.clone());
                if let Err(problem) = T::validate_element(&with) {
                    return Some(problem);
                }
                if text == edit.before.trim() {
                    return None;
                }
                let label = element.el.label.clone();
                let parent = element.el.parent_id.clone();
                let target = id.clone();
                self.act(None, move |eng: &mut Engine<T>| {
                    T::apply_element(eng, &target, with, parent);
                });
                self.eng.announcement = if text.is_empty() {
                    format!("{label}'s description removed.")
                } else {
                    format!("{label}'s description changed.")
                };
                // The properties read it again.
                widgets::reset_field(
                    ui,
                    self.salt.with("fields").with((id.as_str(), "description")),
                );
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
