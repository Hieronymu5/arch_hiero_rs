//! The prototype's colours for the light and dark themes, and the look of form fields, shared by
//! the app's pages and the editor's properties panel.

use egui::{Color32, Ui};
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
    /// A secondary button's face, raised off the page.
    pub raised: Color32,
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
    raised: Color32::from_rgb(0x2b, 0x30, 0x37),
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
    raised: Color32::WHITE,
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

/// How many steps the accent's brightness goes either way of the colour as designed, which is
/// in the middle. The last step either way is as far as the colour can go, and the others share
/// it.
pub const BRIGHTNESS_STEPS: i8 = 5;

/// The most white, or black, a shade takes, however readable it would stay.
const MAX_MIX: f32 = 0.35;

/// The least contrast WCAG AA asks of text.
const AA: f32 = 4.5;

/// How much lighter than designed the accent is in each theme, in steps from
/// `-BRIGHTNESS_STEPS` to `BRIGHTNESS_STEPS`, or darker below 0. Each theme keeps its own, as a
/// colour that reads well on light may want to differ on dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Brightness {
    pub light: i8,
    pub dark: i8,
}

impl Brightness {
    /// The steps in the theme `dark` or light.
    pub const fn of(self, dark: bool) -> i8 {
        if dark { self.dark } else { self.light }
    }

    /// With `steps` in the theme `dark` or light, kept within the slider's ends.
    #[must_use]
    pub fn with(self, dark: bool, steps: i8) -> Self {
        let steps = steps.clamp(-BRIGHTNESS_STEPS, BRIGHTNESS_STEPS);
        if dark {
            Self {
                dark: steps,
                ..self
            }
        } else {
            Self {
                light: steps,
                ..self
            }
        }
    }
}

/// How much white a shade can take, to make it lighter, and how much black, to make it darker,
/// and stay readable, each from 0 to `MAX_MIX`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixLimits {
    pub lighter: f32,
    pub darker: f32,
}

/// What the accent is drawn against in each theme: the page, the text on a button filled with
/// it, and the panels and raised buttons it's text on.
fn accent_grounds(dark: bool) -> &'static [Color32] {
    const LIGHT_GROUNDS: [Color32; 2] = [Color32::from_gray(248), Color32::WHITE];
    const DARK_GROUNDS: [Color32; 4] = [
        Color32::from_gray(27),
        Color32::from_rgb(0x0f, 0x12, 0x15),
        Color32::from_rgb(0x17, 0x1b, 0x20),
        Color32::from_rgb(0x2b, 0x30, 0x37),
    ];
    if dark { &DARK_GROUNDS } else { &LIGHT_GROUNDS }
}

/// `color` mixed with white by `amount`, from 0 to 1, or with black when it's below 0.
pub fn mixed_shade(color: Color32, amount: f32) -> Color32 {
    let toward = if amount > 0.0 { 255.0 } else { 0.0 };
    let share = amount.abs().min(1.0);
    let mix = |c: u8| {
        let c = f32::from(c);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "rounded within 0 to 255"
        )]
        let mixed = (c + (toward - c) * share).round().clamp(0.0, 255.0) as u8;
        mixed
    };
    Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

/// A colour's relative luminance, as WCAG defines it.
fn luminance(color: Color32) -> f32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// The contrast ratio between two colours, from 1 to 21.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// How much white and black `shade` can take and stay readable against each of `grounds`: at
/// least 4.5:1, or as readable as it is as designed, if that's less.
pub fn mix_limits_of(shade: Color32, grounds: &[Color32]) -> MixLimits {
    let needed: Vec<f32> = grounds
        .iter()
        .map(|g| contrast(shade, *g).min(AA))
        .collect();
    let readable = |amount: f32| {
        let mixed = mixed_shade(shade, amount);
        grounds
            .iter()
            .zip(&needed)
            .all(|(g, need)| contrast(mixed, *g) >= need - 1e-6)
    };
    // Readability only falls the further it goes, so the furthest is found by halving.
    let furthest = |direction: f32| {
        if readable(direction * MAX_MIX) {
            return MAX_MIX;
        }
        let (mut low, mut high) = (0.0, MAX_MIX);
        for _ in 0..20 {
            let middle = f32::midpoint(low, high);
            if readable(direction * middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        low
    };
    MixLimits {
        lighter: furthest(1.0),
        darker: furthest(-1.0),
    }
}

/// How much white and black `accent`'s shade can take in the theme `dark` or light.
pub fn mix_limits(accent: Accent, dark: bool) -> MixLimits {
    let (light_shade, dark_shade) = accent.shades();
    let shade = if dark { dark_shade } else { light_shade };
    mix_limits_of(shade, accent_grounds(dark))
}

/// `accent`'s light and dark shades at `brightness`: in each theme, that share of the way to as
/// light, or as dark, as it can go there.
pub fn shades_at(accent: Accent, brightness: Brightness) -> (Color32, Color32) {
    let (light, dark) = accent.shades();
    let at = |shade: Color32, steps: i8, dark: bool| {
        if steps == 0 {
            return shade;
        }
        let limits = mix_limits(accent, dark);
        let share = (f32::from(steps) / f32::from(BRIGHTNESS_STEPS)).clamp(-1.0, 1.0);
        let amount = if share > 0.0 {
            share * limits.lighter
        } else {
            share * limits.darker
        };
        mixed_shade(shade, amount)
    };
    (
        at(light, brightness.light, false),
        at(dark, brightness.dark, true),
    )
}

/// The accent in use, its brightness, and its shades at that brightness, worked out once.
#[derive(Clone, Copy, Debug, Default)]
struct AccentInUse {
    accent: Accent,
    brightness: Brightness,
    shades: Option<(Color32, Color32)>,
}

fn accent_id() -> egui::Id {
    egui::Id::unique("hiero-editor.accent")
}

fn accent_in_use(ctx: &egui::Context) -> AccentInUse {
    ctx.data(|d| d.get_temp(accent_id())).unwrap_or_default()
}

/// The accent colour in use.
pub fn accent(ctx: &egui::Context) -> Accent {
    accent_in_use(ctx).accent
}

/// How much lighter or darker than designed the accent is, in each theme.
pub fn accent_brightness(ctx: &egui::Context) -> Brightness {
    accent_in_use(ctx).brightness
}

/// The accent's light theme shade and its dark theme shade, at its brightness.
pub fn accent_shades(ctx: &egui::Context) -> (Color32, Color32) {
    let in_use = accent_in_use(ctx);
    in_use.shades.unwrap_or_else(|| in_use.accent.shades())
}

/// Uses `accent` from now on, for the palette and for egui's links in each theme.
pub fn set_accent(ctx: &egui::Context, accent: Accent) {
    set_accent_with(ctx, accent, accent_brightness(ctx));
}

/// Makes the accent `brightness` lighter or darker than designed from now on.
pub fn set_accent_brightness(ctx: &egui::Context, brightness: Brightness) {
    set_accent_with(ctx, accent(ctx), brightness);
}

fn set_accent_with(ctx: &egui::Context, accent: Accent, brightness: Brightness) {
    let shades = shades_at(accent, brightness);
    ctx.data_mut(|d| {
        d.insert_temp(
            accent_id(),
            AccentInUse {
                accent,
                brightness,
                shades: Some(shades),
            },
        );
    });
    let (light, dark) = shades;
    for (theme, color) in [(egui::Theme::Light, light), (egui::Theme::Dark, dark)] {
        ctx.style_mut_of(theme, |style| style.visuals.hyperlink_color = color);
    }
}

/// The colour of the editor's selection, in the accent's light shade, as the paper is always
/// white.
pub fn selection_color(ctx: &egui::Context) -> Color32 {
    accent_shades(ctx).0
}

/// The palette of the theme `ui` is drawn in, with the accent in use.
pub fn palette(ui: &Ui) -> Palette {
    let (light, dark) = accent_shades(ui.ctx());
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

/// The font size of page buttons' labels.
const PAGE_BUTTON_FONT_SIZE: f32 = 12.5;

/// A page action: a key-like [`outlined_button`], filled with the accent when it's the main
/// action.
pub fn page_button(ui: &mut Ui, text: &str, main: bool) -> egui::Response {
    accent_button(ui, text, main, egui::vec2(12.0, 5.0), PAGE_BUTTON_FONT_SIZE)
}

/// A destructive page action, such as Delete: like a secondary [`page_button`] but with text in
/// the error colour.
pub fn danger_button(ui: &mut Ui, text: &str) -> egui::Response {
    let color = palette(ui).error;
    outlined_button(
        ui,
        text,
        color,
        false,
        egui::vec2(12.0, 5.0),
        PAGE_BUTTON_FONT_SIZE,
    )
}

/// A secondary button in a row of the list, at the padding of egui's buttons and smaller text.
pub fn row_button(ui: &mut Ui, text: &str) -> egui::Response {
    let padding = ui.spacing().button_padding;
    accent_button(ui, text, false, padding, 12.0)
}

/// An accent button with the given padding and font size.
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

/// A secondary page button showing only `icon`, `width` wide, named `name` for screen readers
/// and in its tooltip.
pub fn icon_button(ui: &mut Ui, icon: ButtonIcon, name: &str, width: f32) -> egui::Response {
    let accent = palette(ui).accent;
    // The button's sides and edge come on top of its padding, so it's `width` wide in all.
    let padding = egui::vec2(((width - BUTTON_ICON - BUTTON_FRAME) / 2.0).max(4.0), 5.0);
    outlined_button_with(
        ui,
        "",
        name,
        Some(icon),
        accent,
        false,
        padding,
        PAGE_BUTTON_FONT_SIZE,
    )
    .on_hover_text(name)
}

/// The size of a button's icon, and the gap before it.
const BUTTON_ICON: f32 = 16.0;
const BUTTON_ICON_GAP: f32 = 6.0;

/// The size of the download icon.
pub const DOWNLOAD_ICON: f32 = BUTTON_ICON;

/// Paints the prototype's 16px download icon, drawn on a 20-unit grid: an arrow down into a tray.
pub fn download_icon(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let scale = DOWNLOAD_ICON / 20.0;
    let at = |x: f32, y: f32| center + egui::vec2(x - 10.0, y - 10.0) * scale;
    let stroke = egui::Stroke::new(1.8 * scale, color);
    let path = |points: &[(f32, f32)]| {
        let points = points.iter().map(|&(x, y)| at(x, y)).collect();
        painter.add(egui::Shape::line(points, stroke));
    };
    path(&[(10.0, 3.0), (10.0, 13.0)]);
    path(&[(5.5, 8.5), (10.0, 13.0), (14.5, 8.5)]);
    path(&[(3.5, 14.0), (3.5, 16.5), (16.5, 16.5), (16.5, 14.0)]);
}

/// How thick a button's edge along its bottom and right is, under its face, when it isn't
/// pressed, and how thick its other sides are.
const BUTTON_EDGE: f32 = 2.0;
const BUTTON_SIDE: f32 = 1.5;

/// How much a button's sides and edge add to its padding, each way.
const BUTTON_FRAME: f32 = BUTTON_SIDE + BUTTON_EDGE - 2.0;

/// How round a button's corners are.
const BUTTON_RADIUS: u8 = 12;

/// A button drawn like a key: its rounded face sits on an edge along its bottom and right in a
/// darker shade, which thins as the face moves down and right while it's pressed. The main action's face is filled with `accent`;
/// another's is raised off the page, with text in `accent`. It's painted here rather than by
/// egui's button to draw the edge and the face's top highlight.
fn outlined_button(
    ui: &mut Ui,
    text: &str,
    accent: Color32,
    main: bool,
    padding: egui::Vec2,
    font_size: f32,
) -> egui::Response {
    outlined_button_with(ui, text, text, None, accent, main, padding, font_size)
}

/// An icon painted after a button's text, around the point given.
pub type ButtonIcon = fn(&egui::Painter, egui::Pos2, Color32);

/// An [`outlined_button`] named `name` for screen readers, with `icon` after its text.
#[expect(clippy::too_many_arguments, reason = "the button's look, in one place")]
fn outlined_button_with(
    ui: &mut Ui,
    text: &str,
    name: &str,
    icon: Option<ButtonIcon>,
    accent: Color32,
    main: bool,
    padding: egui::Vec2,
    font_size: f32,
) -> egui::Response {
    let colors = palette(ui);
    let dark = ui.visuals().dark_mode;
    let font = crate::fonts::font_id(font_size, Weight::Regular);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    // With no text, the icon alone, with no gap before it.
    let gap = if text.is_empty() {
        0.0
    } else {
        BUTTON_ICON_GAP
    };
    let icon_width = if icon.is_some() {
        gap + BUTTON_ICON
    } else {
        0.0
    };
    let content = egui::vec2(galley.size().x + icon_width, galley.size().y);
    let size = (content + 2.0 * padding + egui::Vec2::splat(BUTTON_FRAME))
        .max(egui::vec2(0.0, ui.spacing().interact_size.y));
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name));
    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let shade = |color: Color32, t: f32| color.lerp_to_gamma(Color32::BLACK, t);
    // The face, its top and side border, its bottom edge, its text, and its top highlight. The
    // main action lightens on hover; another's border darkens.
    let (face, side, bottom, text_color, highlight) = if main {
        let face = if hovered {
            accent.lerp_to_gamma(Color32::WHITE, 0.06)
        } else {
            accent
        };
        let highlight = Color32::from_white_alpha(if dark { 77 } else { 71 });
        // Brighter in the dark theme, where the darker shades are lost against the page.
        let (side, bottom) = if dark { (0.08, 0.22) } else { (0.28, 0.42) };
        (
            face,
            shade(accent, side),
            shade(accent, bottom),
            colors.on_accent,
            highlight,
        )
    } else {
        let (side, bottom) = if dark {
            let lighten = if hovered { 0.30 } else { 0.12 };
            (
                colors.input_border.lerp_to_gamma(Color32::WHITE, lighten),
                colors.input_border,
            )
        } else {
            let side = if hovered {
                colors.input_border
            } else {
                colors
                    .input_border
                    .lerp_to_gamma(ui.visuals().panel_fill, 0.35)
            };
            (side, shade(colors.input_border, 0.10))
        };
        let highlight = Color32::from_white_alpha(if dark { 23 } else { 255 });
        (colors.raised, side, bottom, accent, highlight)
    };

    // Pressing moves the face down and right onto a thinner edge, keeping the button's bottom
    // and right in place.
    let edge = if pressed { BUTTON_SIDE } else { BUTTON_EDGE };
    let key = egui::Rect::from_min_max(rect.min + egui::Vec2::splat(BUTTON_EDGE - edge), rect.max);
    let radius = BUTTON_RADIUS;
    let painter = ui.painter();
    painter.rect_filled(key, radius, bottom);
    painter.rect_filled(
        egui::Rect::from_min_max(key.min, key.max - egui::Vec2::splat(edge)),
        radius,
        side,
    );
    let face_rect = egui::Rect::from_min_max(
        key.min + egui::Vec2::splat(BUTTON_SIDE),
        key.max - egui::Vec2::splat(edge),
    );
    painter.rect_filled(face_rect, radius - 2, face);
    // A light line along the face's top, or a shadow while it's pressed in.
    let inset = f32::from(radius);
    let top = face_rect.top() + 0.5;
    let line = if pressed {
        Color32::from_black_alpha(if dark { 71 } else { 46 })
    } else {
        highlight
    };
    painter.line_segment(
        [
            egui::pos2(face_rect.left() + inset, top),
            egui::pos2(face_rect.right() - inset, top),
        ],
        egui::Stroke::new(1.0, line),
    );
    let text_pos = face_rect.center() - content / 2.0;
    if let Some(icon) = icon {
        let center = egui::pos2(
            text_pos.x + galley.size().x + gap + BUTTON_ICON / 2.0,
            face_rect.center().y,
        );
        icon(painter, center, text_color);
    }
    painter.galley(text_pos, galley, text_color);
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            radius + 2,
            egui::Stroke::new(2.0, colors.accent),
            egui::StrokeKind::Outside,
        );
    }
    response
}

/// A palette element button, `height` tall and as wide as `ui`, its name on the left: outlined, tinted with the
/// accent and outlined in it while the pointer is over it, and filled with it while pressed, in
/// each theme's own accent. It's drawn here rather than by egui's button because the drag area
/// around it takes the hover.
pub fn element_button(ui: &mut Ui, text: &str, height: f32) -> egui::Response {
    choice_button(ui, text, height, false)
}

/// An [`element_button`] that can be the one chosen, such as the scene showing: its border then
/// thicker and in the accent, and its text bold in the accent. A name too long for one line
/// wraps, and the button grows to fit it, at least `height` tall.
pub fn choice_button(ui: &mut Ui, text: &str, height: f32, chosen: bool) -> egui::Response {
    /// The space round the text: as the prototype's 0.5rem by 0.625rem.
    const PAD: egui::Vec2 = egui::vec2(10.0, 8.0);
    let colors = palette(ui);
    let width = ui.available_width();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let font = if chosen {
        crate::fonts::font_id(font.size, Weight::SemiBold)
    } else {
        font
    };
    let galley = ui.painter().layout(
        text.to_owned(),
        font,
        Color32::PLACEHOLDER,
        (width - PAD.x * 2.0).max(1.0),
    );
    let size = egui::vec2(width, height.max(galley.size().y + PAD.y * 2.0));
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
        let (border, color, width) = if chosen {
            (colors.accent, colors.accent, 2.0)
        } else {
            (border, color, 1.0)
        };
        let painter = ui.painter();
        let radius = egui::CornerRadius::same(6);
        painter.rect(
            rect,
            radius,
            fill,
            egui::Stroke::new(width, border),
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
        // In the middle, top to bottom, on the left.
        let at = egui::pos2(rect.left() + PAD.x, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, color);
    }
    response
}

/// The prototype's tag icon, `size` points across, filled while its tag is applied.
pub fn tag_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    size: f32,
    color: Color32,
    filled: bool,
) {
    let scale = size / 20.0;
    let at = |x: f32, y: f32| center + egui::vec2(x - 10.0, y - 10.0) * scale;
    let points = vec![
        at(2.5, 10.5),
        at(9.5, 3.5),
        at(16.5, 3.5),
        at(16.5, 10.5),
        at(9.5, 17.5),
    ];
    let stroke = egui::Stroke::new(1.7 * scale, color);
    let fill = if filled {
        color.gamma_multiply(0.3)
    } else {
        Color32::TRANSPARENT
    };
    painter.add(egui::Shape::convex_polygon(points, fill, stroke));
    painter.circle_filled(at(13.0, 7.0), 1.25 * scale, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accent_as_designed_keeps_its_shades() {
        for accent in Accent::ALL {
            assert_eq!(shades_at(accent, Brightness::default()), accent.shades());
        }
    }

    #[test]
    fn each_end_of_the_brightness_stays_readable_in_its_theme() {
        for accent in Accent::ALL {
            let designed = accent.shades();
            for steps in [-BRIGHTNESS_STEPS, BRIGHTNESS_STEPS] {
                let (light, dark) = shades_at(
                    accent,
                    Brightness {
                        light: steps,
                        dark: steps,
                    },
                );
                for (shade, was, dark_theme) in
                    [(light, designed.0, false), (dark, designed.1, true)]
                {
                    for ground in accent_grounds(dark_theme) {
                        let needed = contrast(was, *ground).min(AA);
                        // A rounding of the channels away.
                        assert!(
                            contrast(shade, *ground) >= needed - 0.05,
                            "{} at {steps} on {ground:?}",
                            accent.label()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn lighter_steps_lighten_and_darker_ones_darken() {
        let accent = Accent::Blue;
        let limits = mix_limits(accent, false);
        assert!(limits.lighter > 0.0 || limits.darker > 0.0);
        let (designed, _) = accent.shades();
        let (lighter, _) = shades_at(accent, Brightness { light: 5, dark: 0 });
        let (darker, _) = shades_at(accent, Brightness { light: -5, dark: 0 });
        assert!(luminance(darker) <= luminance(designed));
        assert!(luminance(lighter) >= luminance(designed));
        assert_ne!(lighter, darker);
    }

    #[test]
    fn an_icon_button_is_as_wide_as_asked() {
        let ctx = egui::Context::default();
        let mut width = 0.0;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            width = icon_button(ui, download_icon, "Download", 60.0)
                .rect
                .width();
        })
        .textures_delta
        .clear();
        assert!((width - 60.0).abs() < 0.01, "{width}");
    }

    #[test]
    fn each_theme_keeps_its_own_brightness() {
        let b = Brightness::default().with(true, 3).with(false, -9);
        assert_eq!(
            b,
            Brightness {
                light: -BRIGHTNESS_STEPS,
                dark: 3
            }
        );
        assert_eq!(b.of(true), 3);
    }
}
