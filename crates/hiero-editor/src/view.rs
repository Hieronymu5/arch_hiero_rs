//! The canvas's view: how far it's zoomed and scrolled.

use egui::{Pos2, Rect, Vec2};
use hiero_core::geometry::{DEFAULT_CANVAS, Size};
use hiero_core::layout::canvas_rect;

use crate::paint::Transform;

/// The zoom range, as fractions. To be revisited once the zoom has been tried in use.
pub const ZOOM_MIN: f32 = 0.10;
pub const ZOOM_MAX: f32 = 1.00;
/// The slider's step.
pub const ZOOM_STEP: f32 = 0.05;

/// Space kept around the paper when scrolled to an edge: none, so the paper meets the panels.
const MARGIN: f32 = 0.0;

/// How the canvas is zoomed and scrolled. `scroll` is the screen offset of the paper's
/// top-left corner from the viewport's, like a scroll view's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub scroll: Vec2,
    /// The paper's size in canvas pixels: the diagram's canvas, or while viewing, the part
    /// of it the drawing covers.
    pub paper: Vec2,
    /// Where the paper's top-left corner is, in canvas pixels: the canvas's, or while viewing,
    /// the drawing's.
    pub corner: Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: ZOOM_MAX,
            scroll: Vec2::new(MARGIN, MARGIN),
            paper: Vec2::ZERO,
            corner: Vec2::ZERO,
        }
        .with_canvas(DEFAULT_CANVAS)
    }
}

impl View {
    pub fn transform(&self, viewport: Rect) -> Transform {
        Transform {
            origin: viewport.min + self.scroll - self.corner * self.zoom,
            zoom: self.zoom,
        }
    }

    /// The view of a canvas of `size`.
    pub fn with_canvas(self, size: Size) -> Self {
        self.with_paper(canvas_rect(size))
    }

    /// The view of `area` of the canvas, keeping what's on screen where it is.
    pub fn with_paper(mut self, area: hiero_core::geometry::Rect) -> Self {
        let corner = Vec2::new(area.x as f32, area.y as f32);
        self.scroll += (corner - self.corner) * self.zoom;
        self.corner = corner;
        self.paper = Vec2::new(area.width as f32, area.height as f32);
        self
    }

    /// The paper's size on screen.
    pub fn paper_size(&self) -> Vec2 {
        self.paper * self.zoom
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

    /// Scrolls sideways as little as it can so the world span from `left` to `right` is in the
    /// viewport, `margin` points in from its edges, showing the left end of a span too wide to
    /// fit. Returns how far it scrolled, negative to the left.
    pub fn reveal_x(&mut self, left: f32, right: f32, viewport: Rect, margin: f32) -> f32 {
        let start = self.transform(viewport).origin.x;
        let (screen_left, screen_right) = (start + left * self.zoom, start + right * self.zoom);
        let mut d = 0.0;
        if screen_right > viewport.max.x - margin {
            d = viewport.max.x - margin - screen_right;
        }
        if screen_left + d < viewport.min.x + margin {
            d = viewport.min.x + margin - screen_left;
        }
        let before = self.scroll.x;
        self.scroll.x += d;
        self.clamp(viewport);
        self.scroll.x - before
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2400-wide paper at 100%, in a viewport `width` wide, scrolled to its right edge.
    fn at_right_edge(width: f32) -> (View, Rect) {
        let viewport = Rect::from_min_size(Pos2::new(160.0, 0.0), Vec2::new(width, 800.0));
        let mut view = View::default();
        view.scroll.x = -10_000.0;
        view.clamp(viewport);
        (view, viewport)
    }

    #[test]
    fn showing_part_of_the_canvas_keeps_the_drawing_in_place_and_starts_the_paper_there() {
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0));
        let mut view = View {
            zoom: 0.5,
            scroll: Vec2::new(-100.0, -60.0),
            ..View::default()
        };
        let point = hiero_core::geometry::Point::new(700.0, 300.0);
        let before = view.transform(viewport).pos(point);
        view = view.with_paper(hiero_core::geometry::Rect::new(
            600.0, 200.0, 1600.0, 1200.0,
        ));
        assert_eq!(view.transform(viewport).pos(point), before);
        // Scrolled as far left and up as it goes, the paper's corner meets the viewport's.
        view.scroll = Vec2::new(1000.0, 1000.0);
        view.clamp(viewport);
        let corner = hiero_core::geometry::Point::new(600.0, 200.0);
        assert_eq!(view.transform(viewport).pos(corner), viewport.min);
    }

    #[test]
    fn revealing_scrolls_left_just_enough() {
        let (mut view, wide) = at_right_edge(1230.0);
        assert_eq!(view.scroll.x, 1230.0 - 2400.0);
        // A panel 290 wide opens; the element at 2180–2380 is now past the right edge.
        let narrow = Rect::from_min_size(wide.min, Vec2::new(940.0, 800.0));
        let moved = view.reveal_x(2180.0, 2380.0, narrow, 20.0);
        // It ends 20 clear of the panel, which is as far as the paper's edge allows.
        assert_eq!(moved, -290.0);
        assert_eq!(view.scroll.x, 940.0 - 2400.0);
    }

    #[test]
    fn revealing_what_is_in_view_does_nothing() {
        let (mut view, viewport) = at_right_edge(1230.0);
        assert_eq!(view.reveal_x(1600.0, 1800.0, viewport, 20.0), 0.0);
    }

    #[test]
    fn revealing_a_span_wider_than_the_view_shows_its_left_end() {
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 800.0));
        let mut view = View::default();
        view.clamp(viewport);
        let moved = view.reveal_x(1000.0, 2000.0, viewport, 20.0);
        assert_eq!(moved, -980.0);
        assert_eq!(viewport.min.x + view.scroll.x + 1000.0, 20.0);
    }

    #[test]
    fn revealing_scrolls_right_to_a_span_off_the_left() {
        let (mut view, viewport) = at_right_edge(1230.0);
        let moved = view.reveal_x(100.0, 300.0, viewport, 20.0);
        assert_eq!(
            viewport.min.x + view.scroll.x + 100.0,
            viewport.min.x + 20.0
        );
        assert!(moved > 0.0);
    }
}
