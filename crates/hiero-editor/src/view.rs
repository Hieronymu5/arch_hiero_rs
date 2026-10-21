//! The canvas's view: how far it's zoomed and scrolled.

use egui::{Pos2, Rect, Vec2};
use hiero_core::geometry::{CANVAS_HEIGHT, CANVAS_WIDTH};

use crate::paint::Transform;

/// The zoom range, as fractions. To be revisited once the zoom has been tried in use.
pub const ZOOM_MIN: f32 = 0.10;
pub const ZOOM_MAX: f32 = 1.00;
/// The slider's step.
pub const ZOOM_STEP: f32 = 0.05;

/// Space kept around the paper when scrolled to an edge.
const MARGIN: f32 = 24.0;

/// How the canvas is zoomed and scrolled. `scroll` is the screen offset of the paper's
/// top-left corner from the viewport's, like a scroll view's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub scroll: Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: ZOOM_MAX,
            scroll: Vec2::new(MARGIN, MARGIN),
        }
    }
}

impl View {
    pub fn transform(&self, viewport: Rect) -> Transform {
        Transform {
            origin: viewport.min + self.scroll,
            zoom: self.zoom,
        }
    }

    /// The paper's size on screen.
    pub fn paper_size(&self) -> Vec2 {
        Vec2::new(CANVAS_WIDTH as f32, CANVAS_HEIGHT as f32) * self.zoom
    }

    /// Keeps the paper reachable: it can't be scrolled further than a margin past any edge,
    /// and is centred when it's smaller than the viewport.
    pub fn clamp(&mut self, viewport: Rect) {
        let paper = self.paper_size();
        let axis = |scroll: f32, paper: f32, view: f32| {
            if paper + MARGIN * 2.0 <= view {
                (view - paper) / 2.0
            } else {
                scroll.min(MARGIN).max(view - paper - MARGIN)
            }
        };
        self.scroll = Vec2::new(
            axis(self.scroll.x, paper.x, viewport.width()),
            axis(self.scroll.y, paper.y, viewport.height()),
        );
    }

    /// Zooms to `zoom`, keeping the world point under `anchor` (a screen position) where it is.
    pub fn zoom_around(&mut self, zoom: f32, anchor: Pos2, viewport: Rect) {
        let zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        let before = self.transform(viewport).world(anchor);
        self.zoom = zoom;
        let after = self.transform(viewport).pos(before);
        self.scroll += anchor - after;
        self.clamp(viewport);
    }
}
