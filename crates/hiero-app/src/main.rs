//! Arch Hiero desktop entry point.

mod app;
mod canvas;

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Arch Hiero")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([480.0, 320.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Arch Hiero",
        options,
        Box::new(|cc| Ok(Box::new(app::HieroApp::new(cc)))),
    )
}
