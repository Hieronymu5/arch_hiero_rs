//! Development aids for checking the app without clicking through it, set by environment
//! variables:
//!
//! - `ARCH_HIERO_DB`: the database file to use instead of the usual one.
//! - `ARCH_HIERO_OPEN`: the name of a diagram to open on start.
//! - `ARCH_HIERO_NEW`: a diagram type, such as `physical`, to create a diagram of and open.
//! - `ARCH_HIERO_SELECT`: the name of an element to select in the open diagram.
//! - `ARCH_HIERO_SCREENSHOT`: a PNG path; the window is saved there after it has settled, and
//!   the app quits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app::App;

#[derive(Default)]
pub struct DevTools {
    open: Option<String>,
    new: Option<String>,
    select: Option<String>,
    screenshot: Option<std::path::PathBuf>,
    frame: u32,
    requested: bool,
    done: Arc<AtomicBool>,
}

impl DevTools {
    pub fn from_env() -> Self {
        Self {
            open: std::env::var("ARCH_HIERO_OPEN").ok(),
            new: std::env::var("ARCH_HIERO_NEW").ok(),
            select: std::env::var("ARCH_HIERO_SELECT").ok(),
            screenshot: std::env::var_os("ARCH_HIERO_SCREENSHOT").map(Into::into),
            ..Self::default()
        }
    }

    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context) {
        self.frame += 1;
        if let Some(name) = self.open.take()
            && let Some(summary) = app.list.iter().find(|s| s.meta.name == name).cloned()
        {
            app.open_diagram(&summary.meta.id);
        }
        if let Some(kind) = self.new.take()
            && let Some(t) = hiero_core::model::DiagramTypeId::parse(&kind)
        {
            app.form = crate::app::DetailsForm {
                name: format!("New {}", t.label()),
                diagram_type: Some(t),
                ..Default::default()
            };
            app.create();
        }
        if self.frame > 5
            && let Some(label) = self.select.take()
            && let Some(open) = &mut app.open
        {
            open.editor.select_by_label(&label);
        }
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        if self.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frame > 40 && !self.requested {
            self.requested = true;
            let done = self.done.clone();
            let repaint = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::ScreenshotCallback::new(move |image| {
                    let [w, h] = image.size;
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Some(buffer) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                        let _ = buffer.save(&path);
                    }
                    done.store(true, Ordering::SeqCst);
                    repaint.request_repaint();
                }),
            ));
        }
        ctx.request_repaint();
    }
}
