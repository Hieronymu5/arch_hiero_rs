use hiero_core::Diagram;

use crate::canvas::Canvas;

const STORAGE_KEY: &str = "arch_hiero_app";

/// Top-level application state. Persisted between launches via eframe storage.
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct HieroApp {
    diagram: Diagram,
    canvas: Canvas,
}

impl HieroApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_else(|| Self {
                diagram: Diagram::sample(),
                canvas: Canvas::default(),
            })
    }
}

impl eframe::App for HieroApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Arch Hiero");
                ui.separator();
                if ui.button("Reset view").clicked() {
                    self.canvas.reset_view();
                }
                if ui.button("New diagram").clicked() {
                    self.diagram = Diagram::new();
                    self.canvas = Canvas::default();
                }
                if ui.button("Load sample").clicked() {
                    self.diagram = Diagram::sample();
                    self.canvas = Canvas::default();
                }
                ui.separator();
                ui.weak(
                    "Double-click: add node · Drag: move/pan · ⌘/Ctrl+scroll: zoom · Del: remove",
                );
            });
        });

        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.canvas.show(ui, &mut self.diagram);
        });
    }
}
