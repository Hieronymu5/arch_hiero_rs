//! Arch Hiero: a native diagramming app. Diagrams are kept in a local SQLite database, with
//! versions saved on request and every change saved as it's made.

mod app;
mod devtools;
mod editors;
mod format;
mod list_sort;
mod pages;
mod worker;

use std::path::{Path, PathBuf};

use app::{App, Databases, Outcome, Page};
use hiero_editor::theme::Accent;
use hiero_store::Store;
use hiero_store::locations::Locations;

const THEME_KEY: &str = "theme";
const ACCENT_KEY: &str = "accent";

fn theme_from(text: Option<String>) -> egui::ThemePreference {
    match text.as_deref() {
        Some("light") => egui::ThemePreference::Light,
        Some("dark") => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    }
}

fn theme_name(theme: egui::ThemePreference) -> &'static str {
    match theme {
        egui::ThemePreference::Light => "light",
        egui::ThemePreference::Dark => "dark",
        egui::ThemePreference::System => "system",
    }
}

/// The app, or why it couldn't start.
enum Shell {
    Running(Box<App>, devtools::DevTools),
    Failed(String),
}

impl Shell {
    fn start(cc: &eframe::CreationContext<'_>) -> Self {
        hiero_editor::fonts::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        hiero_editor::theme::install_text_colors(&cc.egui_ctx);
        let theme = theme_from(cc.storage.and_then(|s| s.get_string(THEME_KEY)));
        cc.egui_ctx.set_theme(theme);
        // Applied before the first frame, as the theme is. An unknown name gives the default.
        let accent = cc
            .storage
            .and_then(|s| s.get_string(ACCENT_KEY))
            .and_then(|key| Accent::from_key(&key))
            .unwrap_or_default();
        hiero_editor::theme::set_accent(&cc.egui_ctx, accent);
        // `ARCH_HIERO_DB` picks the database for this run only; otherwise it's the one used last.
        let chosen = std::env::var_os("ARCH_HIERO_DB").map(PathBuf::from);
        let remember = chosen.is_none();
        let mut locations = if remember {
            Locations::load()
        } else {
            Locations::default()
        };
        let Some(mut path) = chosen.or_else(|| locations.current_or_default()) else {
            return Self::Failed("There's no folder to keep the diagrams in.".into());
        };
        let start = |path: &Path| {
            let store = Store::open(path)?;
            let worker = worker::Worker::start(path.to_owned(), cc.egui_ctx.clone())?;
            Ok::<_, hiero_store::StoreError>((store, worker))
        };
        // A database used last that's gone, such as on a drive that's been removed, isn't made
        // again empty: the default one is used instead, as it is when one can't be opened.
        let default = hiero_store::default_path().filter(|d| *d != path);
        let mut notice = None;
        let mut started = if remember && default.is_some() && !path.exists() {
            notice = Some(format!(
                "{} wasn't found, so the default database is in use.",
                path.display()
            ));
            None
        } else {
            Some(start(&path))
        };
        if let Some(Err(e)) = &started
            && remember
            && default.is_some()
        {
            notice = Some(format!(
                "The diagrams at {} couldn't be opened, so the default database is in use. {e}",
                path.display()
            ));
            started = None;
        }
        if started.is_none()
            && let Some(default) = default
        {
            path = default;
            started = Some(start(&path));
        }
        let started = started.expect("a database was tried");
        match started {
            Ok((store, worker)) => {
                locations.use_path(&path);
                let databases = Databases::new(path, locations, remember);
                let mut app = App::new(store, worker, theme, accent, databases);
                app.remember_locations();
                if let Some(notice) = notice {
                    app.outcome = Some(Outcome::error(notice));
                }
                Self::Running(Box::new(app), devtools::DevTools::from_env())
            }
            Err(e) => Self::Failed(format!(
                "The diagrams at {} couldn't be opened. {e}",
                path.display()
            )),
        }
    }
}

impl eframe::App for Shell {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Self::Running(app, _) = self {
            storage.set_string(THEME_KEY, theme_name(app.theme).to_owned());
            storage.set_string(ACCENT_KEY, app.accent.key());
        }
    }

    fn on_exit(&mut self) {
        if let Self::Running(app, _) = self {
            app.flush_save();
            // Give the store thread a moment to write the last change.
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (app, dev) = match self {
            Self::Running(app, dev) => (app, dev),
            Self::Failed(message) => {
                egui::CentralPanel::default_margins().show(ui, |ui| {
                    ui.heading("Arch Hiero couldn't start");
                    ui.label(message.as_str());
                });
                return;
            }
        };
        let ctx = ui.ctx().clone();
        dev.frame(app, &ctx);
        app.handle_events();
        app.top_bar(ui);
        // The editor lines up its own toolbar; its panels and canvas fill the window.
        egui::CentralPanel::default_margins().show(ui, |ui| match app.page.clone() {
            Page::List => pages::under_logo(ui, |ui| app.list_page(ui)),
            Page::New => pages::under_logo(ui, |ui| app.new_page(ui)),
            Page::Details(id) => pages::under_logo(ui, |ui| app.details_page(ui, &id)),
            Page::Editor(id) => app.editor_page(ui, &id),
            Page::History(id) => pages::under_logo(ui, |ui| app.history_page(ui, &id)),
        });
        app.dialogs(&ctx);
        app.collect_changes();
        app.shortcuts(&ctx);
        app.save_if_due();
        if app.save_due.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn main() -> eframe::Result {
    env_logger::init();
    // The window's and Dock's icon: the logo on a rounded square. The macOS bundle uses the
    // same image, as AppIcon.icns.
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/app-icon.png"))
        .expect("the bundled app icon is a valid PNG");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Arch Hiero")
            .with_icon(icon)
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Arch Hiero",
        options,
        Box::new(|cc| Ok(Box::new(Shell::start(cc)))),
    )
}
