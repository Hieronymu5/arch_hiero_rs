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
    /// Table rules and other subtle dividers.
    pub border: Color32,
    pub error: Color32,
    pub surface: Color32,
}

pub const DARK: Palette = Palette {
    accent: Color32::from_rgb(0x6c, 0xb2, 0xff),
    on_accent: Color32::from_rgb(0x0f, 0x12, 0x15),
    text: Color32::from_rgb(0xe6, 0xe9, 0xed),
    text_muted: Color32::from_rgb(0xa3, 0xac, 0xb6),
    input_border: Color32::from_rgb(0x8b, 0x94, 0x9e),
    border: Color32::from_rgb(0x35, 0x3c, 0x44),
    error: Color32::from_rgb(0xff, 0x8a, 0x80),
    surface: Color32::from_rgb(0x17, 0x1b, 0x20),
};

pub const LIGHT: Palette = Palette {
    accent: Color32::from_rgb(0x0b, 0x5c, 0xad),
    on_accent: Color32::WHITE,
    text: Color32::from_rgb(0x1b, 0x1f, 0x24),
    text_muted: Color32::from_rgb(0x57, 0x60, 0x6a),
    input_border: Color32::from_rgb(0x6e, 0x77, 0x81),
    border: Color32::from_rgb(0xd0, 0xd7, 0xde),
    error: Color32::from_rgb(0xb3, 0x26, 0x1e),
    surface: Color32::WHITE,
};

/// The app's primary colour, for buttons, links, focus rings, and the editor's selection, chosen
/// beside the theme picker. Each has a shade for the light theme and one for the dark, which pass
/// WCAG AA, 4.5:1, against the backgrounds and as a button's fill under the theme's
/// `on_accent` text. None is red, which reads as an error.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Blue,
    Navy,
    Denim,
    Indigo,
    Pine,
    Forest,
    Slate,
    Graphite,
    Gold,
    Bronze,
    Copper,
    Brown,
    Chestnut,
    Amber,
    Tangerine,
    Rose,
}

impl Accent {
    /// Every accent in the order offered, four to a row in the picker: blues, greens and greys,
    /// yellow and browns, then reddish brown, oranges, and pink.
    pub const ALL: [Self; 16] = [
        Self::Blue,
        Self::Navy,
        Self::Denim,
        Self::Indigo,
        Self::Pine,
        Self::Forest,
        Self::Slate,
        Self::Graphite,
        Self::Gold,
        Self::Bronze,
        Self::Copper,
        Self::Brown,
        Self::Chestnut,
        Self::Amber,
        Self::Tangerine,
        Self::Rose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Navy => "Navy",
            Self::Denim => "Denim",
            Self::Indigo => "Indigo",
            Self::Pine => "Pine",
            Self::Forest => "Forest",
            Self::Slate => "Slate",
            Self::Graphite => "Graphite",
            Self::Gold => "Gold",
            Self::Bronze => "Bronze",
            Self::Copper => "Copper",
            Self::Brown => "Brown",
            Self::Chestnut => "Chestnut",
            Self::Amber => "Amber",
            Self::Tangerine => "Tangerine",
            Self::Rose => "Rose",
        }
    }

    /// The accent's name as it's stored, such as "pine".
    pub fn key(self) -> String {
        self.label().to_lowercase()
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == key)
    }

    /// Its light theme's shade and its dark theme's.
    pub const fn shades(self) -> (Color32, Color32) {
        match self {
            Self::Blue => (
                Color32::from_rgb(0x0b, 0x5c, 0xad),
                Color32::from_rgb(0x6c, 0xb2, 0xff),
            ),
            Self::Navy => (
                Color32::from_rgb(0x22, 0x40, 0x7a),
                Color32::from_rgb(0x93, 0xb3, 0xee),
            ),
            Self::Denim => (
                Color32::from_rgb(0x42, 0x69, 0xae),
                Color32::from_rgb(0x9f, 0xb5, 0xdb),
            ),
            Self::Indigo => (
                Color32::from_rgb(0x39, 0x49, 0xab),
                Color32::from_rgb(0x9f, 0xa8, 0xff),
            ),
            Self::Pine => (
                Color32::from_rgb(0x0f, 0x61, 0x52),
                Color32::from_rgb(0x6c, 0xcf, 0xae),
            ),
            Self::Forest => (
                Color32::from_rgb(0x34, 0x79, 0x40),
                Color32::from_rgb(0x77, 0xc5, 0x84),
            ),
            Self::Slate => (
                Color32::from_rgb(0x4a, 0x5b, 0x74),
                Color32::from_rgb(0xa7, 0xb6, 0xcc),
            ),
            Self::Graphite => (
                Color32::from_rgb(0x4b, 0x55, 0x63),
                Color32::from_rgb(0xc0, 0xc7, 0xd1),
            ),
            Self::Gold => (
                Color32::from_rgb(0x7f, 0x62, 0x00),
                Color32::from_rgb(0xf0, 0xc6, 0x4a),
            ),
            Self::Bronze => (
                Color32::from_rgb(0x81, 0x67, 0x31),
                Color32::from_rgb(0xcd, 0xb1, 0x7a),
            ),
            Self::Copper => (
                Color32::from_rgb(0x9e, 0x44, 0x20),
                Color32::from_rgb(0xf2, 0xa2, 0x83),
            ),
            Self::Brown => (
                Color32::from_rgb(0x7a, 0x4e, 0x2d),
                Color32::from_rgb(0xd4, 0xa5, 0x7e),
            ),
            Self::Chestnut => (
                Color32::from_rgb(0xa6, 0x54, 0x3f),
                Color32::from_rgb(0xdd, 0xa7, 0x9a),
            ),
            Self::Amber => (
                Color32::from_rgb(0xb0, 0x4f, 0x00),
                Color32::from_rgb(0xfb, 0xa9, 0x5a),
            ),
            Self::Tangerine => (
                Color32::from_rgb(0xb3, 0x4a, 0x00),
                Color32::from_rgb(0xfb, 0x9e, 0x5b),
            ),
            Self::Rose => (
                Color32::from_rgb(0xa8, 0x32, 0x5e),
                Color32::from_rgb(0xf7, 0xa1, 0xc1),
            ),
        }
    }
}

fn accent_id() -> egui::Id {
    egui::Id::unique("hiero-editor.accent")
}

/// The accent colour in use.
pub fn accent(ctx: &egui::Context) -> Accent {
    ctx.data(|d| d.get_temp(accent_id())).unwrap_or_default()
}

/// Uses `accent` from now on, for the palette and for egui's links in each theme.
pub fn set_accent(ctx: &egui::Context, accent: Accent) {
    ctx.data_mut(|d| d.insert_temp(accent_id(), accent));
    let (light, dark) = accent.shades();
    for (theme, color) in [(egui::Theme::Light, light), (egui::Theme::Dark, dark)] {
        ctx.style_mut_of(theme, |style| style.visuals.hyperlink_color = color);
    }
}

/// The colour of the editor's selection, in the accent's light shade, as the paper is always
/// white.
pub fn selection_color(ctx: &egui::Context) -> Color32 {
    accent(ctx).shades().0
}

/// The palette of the theme `ui` is drawn in, with the accent in use.
pub fn palette(ui: &Ui) -> Palette {
    let (light, dark) = accent(ui.ctx()).shades();
    if ui.visuals().dark_mode {
        Palette {
            accent: dark,
            ..DARK
        }
    } else {
        Palette {
            accent: light,
            ..LIGHT
        }
    }
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

/// A destructive page action, such as Delete: like an outlined [`page_button`] but in the error
/// colour.
pub fn danger_button(ui: &mut Ui, text: &str) -> egui::Response {
    let color = palette(ui).error;
    outlined_button(ui, text, color, false, egui::vec2(12.0, 5.0), 14.0)
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
    let accent = palette(ui).accent;
    outlined_button(ui, text, accent, main, padding, font_size)
}

/// A button bordered in `accent`, and filled with it when it's the main action.
fn outlined_button(
    ui: &mut Ui,
    text: &str,
    accent: Color32,
    main: bool,
    padding: egui::Vec2,
    font_size: f32,
) -> egui::Response {
    let Palette {
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

/// A palette element button, `height` tall and as wide as `ui`, its name on the left: outlined, tinted with the
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
            egui::pos2(rect.left() + 10.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::TextStyle::Button.resolve(ui.style()),
            color,
        );
    }
    response
}
