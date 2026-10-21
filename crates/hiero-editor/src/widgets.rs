//! Form fields for properties panels: text that keeps what's typed while it's invalid, with
//! the reason shown under it, and selects.

use egui::{Id, RichText, Ui};

use crate::theme::{field_label, palette};

/// The space above each field, between it and the one before.
pub const FIELD_GAP: f32 = 10.0;

/// The colour of error messages.
pub fn error_color(ui: &Ui) -> egui::Color32 {
    palette(ui).error
}

/// A field's label, with the space above it. Marked required when `required`.
pub fn label(ui: &mut Ui, text: &str, required: bool) {
    ui.add_space(FIELD_GAP);
    field_label(ui, text, required);
}

/// A labelled text field whose text is kept in egui's memory under `id`, so typing isn't lost
/// between frames or interrupted by saves. Returns the new text when it changed and passes
/// `validate`.
pub fn text_field(
    ui: &mut Ui,
    id: Id,
    label: &str,
    current: &str,
    multiline: bool,
    validate: impl Fn(&str) -> Result<(), String>,
) -> Option<String> {
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| current.to_owned());
    // A field that can't be left empty is required.
    self::label(ui, label, validate("").is_err());
    let edit = if multiline {
        egui::TextEdit::multiline(&mut text).desired_rows(3)
    } else {
        egui::TextEdit::singleline(&mut text)
    };
    let response = ui.add(edit.id(id.with("edit")).desired_width(f32::INFINITY));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
    let result = validate(&text);
    if let Err(message) = &result {
        ui.label(RichText::new(message).color(error_color(ui)).small());
    }
    ui.data_mut(|d| d.insert_temp(id, text.clone()));
    (response.changed() && result.is_ok() && text != current).then_some(text)
}

/// A checkbox, with the space above it that fields have.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, text: &str) -> egui::Response {
    ui.add_space(FIELD_GAP);
    ui.checkbox(checked, text)
}

/// Forgets what was typed in the field `id`, so it shows the stored value again.
pub fn reset_field(ui: &Ui, id: Id) {
    ui.data_mut(|d| d.remove::<String>(id));
}

/// A labelled select. `options` pairs each value with its label; `none` is the label of the
/// empty choice, when there is one. Returns the new value when it changed.
pub fn select<T: Clone + PartialEq>(
    ui: &mut Ui,
    id: Id,
    label: &str,
    current: &Option<T>,
    none: Option<&str>,
    options: &[(T, String)],
) -> Option<Option<T>> {
    self::label(ui, label, none.is_none());
    let selected_text = match current {
        Some(value) => options
            .iter()
            .find(|(v, _)| v == value)
            .map_or_else(String::new, |(_, l)| l.clone()),
        None => none.unwrap_or("").to_owned(),
    };
    let mut changed = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected_text)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if let Some(none) = none
                && ui.selectable_label(current.is_none(), none).clicked()
                && current.is_some()
            {
                changed = Some(None);
            }
            for (value, text) in options {
                let is = current.as_ref() == Some(value);
                if ui.selectable_label(is, text).clicked() && !is {
                    changed = Some(Some(value.clone()));
                }
            }
        });
    changed
}

/// A section heading in a panel.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).font(crate::fonts::bold(15.0)));
}
