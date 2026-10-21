//! Registers the bundled Inter font with egui, so the editor draws text in the font its
//! layout was measured in.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId};
use hiero_core::text::{FONT_BOLD, FONT_REGULAR, FONT_SEMIBOLD, Weight};

const SEMIBOLD: &str = "Inter SemiBold";
const BOLD: &str = "Inter Bold";

/// Installs Inter as the app's proportional font, with semibold and bold families.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        Arc::new(FontData::from_static(FONT_REGULAR)),
    );
    fonts.font_data.insert(
        SEMIBOLD.into(),
        Arc::new(FontData::from_static(FONT_SEMIBOLD)),
    );
    fonts
        .font_data
        .insert(BOLD.into(), Arc::new(FontData::from_static(FONT_BOLD)));
    let fallbacks: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let family = |first: &str| {
        let mut list = vec![first.to_owned()];
        list.extend(fallbacks.iter().cloned());
        list
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("Inter"));
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), family(SEMIBOLD));
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), family(BOLD));
    ctx.set_fonts(fonts);
}

/// The font for a weight at `size` points.
pub fn font_id(size: f32, weight: Weight) -> FontId {
    let family = match weight {
        Weight::Regular => FontFamily::Proportional,
        Weight::SemiBold => FontFamily::Name(SEMIBOLD.into()),
        Weight::Bold => FontFamily::Name(BOLD.into()),
    };
    FontId::new(size, family)
}

/// Bold text for headings in the app's panels.
pub fn bold(size: f32) -> FontId {
    font_id(size, Weight::Bold)
}
