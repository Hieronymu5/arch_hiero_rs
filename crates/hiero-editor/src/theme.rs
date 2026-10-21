//! The prototype's colours for the light and dark themes, and the look of form fields, shared by
//! the app's pages and the editor's properties panel.

use egui::{Color32, RichText, Ui};
use hiero_core::text::Weight;

/// How far page content is indented from the window's margin, to line up with the app logo's
/// lettering, which is inset in its image.
pub const CONTENT_INSET: i8 = 9;

/// The prototype's colours for one theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub accent: Color32,
    pub on_accent: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub input_border: Color32,
    pub error: Color32,
    pub surface: Color32,
}

pub const DARK: Palette = Palette {
    accent: Color32::from_rgb(0x6c, 0xb2, 0xff),
    on_accent: Color32::from_rgb(0x0f, 0x12, 0x15),
    text: Color32::from_rgb(0xe6, 0xe9, 0xed),
    text_muted: Color32::from_rgb(0xa3, 0xac, 0xb6),
    input_border: Color32::from_rgb(0x8b, 0x94, 0x9e),
    error: Color32::from_rgb(0xff, 0x8a, 0x80),
    surface: Color32::from_rgb(0x17, 0x1b, 0x20),
};

pub const LIGHT: Palette = Palette {
    accent: Color32::from_rgb(0x0b, 0x5c, 0xad),
    on_accent: Color32::WHITE,
    text: Color32::from_rgb(0x1b, 0x1f, 0x24),
    text_muted: Color32::from_rgb(0x57, 0x60, 0x6a),
    input_border: Color32::from_rgb(0x6e, 0x77, 0x81),
    error: Color32::from_rgb(0xb3, 0x26, 0x1e),
    surface: Color32::WHITE,
};

/// The palette of the theme `ui` is drawn in.
pub fn palette(ui: &Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

/// Gives labels, headings, and buttons the prototype's text colours in each theme, in place of
/// egui's greys, which are too faint in the dark theme. Weak text, such as descriptions, gets the
/// prototype's muted colour.
pub fn install_text_colors(ctx: &egui::Context) {
    for (theme, colors) in [(egui::Theme::Dark, DARK), (egui::Theme::Light, LIGHT)] {
        ctx.style_mut_of(theme, |style| {
            let visuals = &mut style.visuals;
            visuals.weak_text_color = Some(colors.text_muted);
            visuals.widgets.noninteractive.fg_stroke.color = colors.text;
            visuals.widgets.inactive.fg_stroke.color = colors.text;
        });
    }
}

/// Outlines a form's fields like the prototype's inputs, so they stand out from the page.
pub fn form_style(ui: &mut Ui) {
    let colors = palette(ui);
    let style = ui.style_mut();
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    let visuals = &mut style.visuals;
    visuals.text_edit_bg_color = Some(colors.surface);
    let widgets = &mut visuals.widgets;
    for (state, border) in [
        (&mut widgets.inactive, colors.input_border),
        (&mut widgets.hovered, colors.text),
        (&mut widgets.active, colors.accent),
        (&mut widgets.open, colors.accent),
    ] {
        state.bg_stroke = egui::Stroke::new(1.0, border);
        state.corner_radius = egui::CornerRadius::same(6);
        state.expansion = 0.0;
    }
    widgets.inactive.weak_bg_fill = colors.surface;
    widgets.hovered.weak_bg_fill = colors.surface;
}

/// A form field's label: semibold, with a red `*` when the field is required.
pub fn field_label(ui: &mut Ui, text: &str, required: bool) {
    let colors = palette(ui);
    let mut job = egui::text::LayoutJob::default();
    let font = crate::fonts::font_id(
        egui::TextStyle::Body.resolve(ui.style()).size,
        Weight::SemiBold,
    );
    let format = |color| egui::TextFormat::simple(font.clone(), color);
    job.append(text, 0.0, format(colors.text));
    if required {
        job.append("*", 4.0, format(colors.error));
    }
    ui.label(job);
    ui.add_space(-2.0);
}

/// A page action styled like the prototype's `.button`: an accent border, and filled with the
/// accent when it's the main action. It looks the same in every state, so hovering doesn't shift
/// its text.
pub fn page_button(ui: &mut Ui, text: &str, main: bool) -> egui::Response {
    accent_button(ui, text, main, egui::vec2(12.0, 5.0), 14.0)
}

/// A button in a row of the list, styled like the prototype's `.button-secondary` but at the
/// padding of egui's buttons and smaller text.
pub fn row_button(ui: &mut Ui, text: &str) -> egui::Response {
    let padding = ui.spacing().button_padding;
    accent_button(ui, text, false, padding, 12.0)
}

/// An accent-bordered button with the given padding and font size.
pub fn accent_button(
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
        let label = RichText::new(text).font(crate::fonts::font_id(font_size, Weight::SemiBold));
        let response = ui.add(egui::Button::new(label));
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response
    })
    .inner
}

/// A palette element button, `height` tall and as wide as `ui`: outlined, tinted with the
/// accent and outlined in it while the pointer is over it, and filled with it while pressed, in
/// each theme's own accent. It's drawn here rather than by egui's button because the drag area
/// around it takes the hover.
pub fn element_button(ui: &mut Ui, text: &str, height: f32) -> egui::Response {
    let colors = palette(ui);
    let size = egui::vec2(ui.available_width(), height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    if ui.is_rect_visible(rect) {
        let tint = if ui.visuals().dark_mode { 0.25 } else { 0.12 };
        let (fill, border, color) = if response.is_pointer_button_down_on() {
            (colors.accent, colors.accent, colors.on_accent)
        } else if ui.rect_contains_pointer(rect) {
            (
                colors.accent.gamma_multiply(tint),
                colors.accent,
                colors.text,
            )
        } else {
            (
                ui.visuals().widgets.inactive.weak_bg_fill,
                colors.input_border,
                colors.text,
            )
        };
        let painter = ui.painter();
        let radius = egui::CornerRadius::same(6);
        painter.rect(
            rect,
            radius,
            fill,
            egui::Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                radius,
                egui::Stroke::new(2.0, colors.accent),
                egui::StrokeKind::Outside,
            );
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::TextStyle::Button.resolve(ui.style()),
            color,
        );
    }
    response
}
