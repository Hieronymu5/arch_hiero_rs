//! Interactive, zoomable diagram canvas drawn directly with egui's painter.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use hiero_core::{Diagram, NodeId, Point};

/// View state for the canvas (pan/zoom/selection). The diagram itself lives in
/// `hiero_core` so that the model stays UI-independent.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Canvas {
    /// Screen-space offset of the world origin relative to the canvas rect.
    pan: Vec2,
    zoom: f32,
    #[serde(skip)]
    selected: Option<NodeId>,
    #[serde(skip)]
    dragging: Option<NodeId>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
            selected: None,
            dragging: None,
        }
    }
}

impl Canvas {
    const MIN_ZOOM: f32 = 0.1;
    const MAX_ZOOM: f32 = 8.0;
    const GRID: f32 = 40.0;

    pub fn reset_view(&mut self) {
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
    }

    fn to_screen(&self, rect: Rect, p: Point) -> Pos2 {
        rect.min + self.pan + Vec2::new(p.x, p.y) * self.zoom
    }

    fn to_world(&self, rect: Rect, p: Pos2) -> Point {
        let v = (p - rect.min - self.pan) / self.zoom;
        Point::new(v.x, v.y)
    }

    pub fn show(&mut self, ui: &mut egui::Ui, diagram: &mut Diagram) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        let visuals = ui.visuals().clone();

        // --- Input -----------------------------------------------------------
        if response.hovered() {
            let (zoom_delta, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
            if let Some(hover) = response.hover_pos()
                && (zoom_delta - 1.0).abs() > f32::EPSILON
            {
                // Zoom around the cursor so the point under it stays put.
                let before = self.to_world(rect, hover);
                self.zoom = (self.zoom * zoom_delta).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
                let after = self.to_screen(rect, before);
                self.pan += hover - after;
            }
            self.pan += scroll;
        }

        if response.drag_started()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.dragging = diagram.hit_test(self.to_world(rect, pos));
            self.selected = self.dragging;
        }
        if response.dragged() {
            let delta = response.drag_delta();
            match self.dragging.and_then(|id| diagram.node_mut(id)) {
                Some(node) => {
                    node.position.x += delta.x / self.zoom;
                    node.position.y += delta.y / self.zoom;
                }
                None => self.pan += delta,
            }
        }
        if response.drag_stopped() {
            self.dragging = None;
        }

        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.selected = diagram.hit_test(self.to_world(rect, pos));
        }
        if response.double_clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let world = self.to_world(rect, pos);
            if diagram.hit_test(world).is_none() {
                let label = format!("Node {}", diagram.nodes.len() + 1);
                let id = diagram.add_node(label, Point::new(world.x - 70.0, world.y - 30.0));
                if let Some(prev) = self.selected {
                    diagram.connect(prev, id);
                }
                self.selected = Some(id);
            }
        }
        if let Some(id) = self.selected
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            diagram.remove_node(id);
            self.selected = None;
        }

        // --- Paint -----------------------------------------------------------
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
        self.paint_grid(&painter, rect, visuals.faint_bg_color);

        let edge_stroke = Stroke::new(1.5 * self.zoom.sqrt(), visuals.text_color());
        for edge in &diagram.edges {
            let (Some(a), Some(b)) = (diagram.node(edge.from), diagram.node(edge.to)) else {
                continue;
            };
            let from = self.to_screen(rect, a.center());
            let to = self.to_screen(rect, b.center());
            let to_rect = self.node_rect(rect, b);
            let tip = clip_to_rect(from, to, to_rect);
            painter.line_segment([from, tip], edge_stroke);
            let dir = (tip - from).normalized() * 12.0 * self.zoom.sqrt();
            painter.arrow(tip - dir, dir, edge_stroke);
        }

        let font = FontId::proportional(14.0 * self.zoom);
        for node in &diagram.nodes {
            let r = self.node_rect(rect, node);
            if !rect.intersects(r) {
                continue; // cull off-screen nodes
            }
            let selected = self.selected == Some(node.id);
            let stroke = if selected {
                Stroke::new(2.5, visuals.selection.stroke.color)
            } else {
                Stroke::new(1.0, visuals.widgets.noninteractive.fg_stroke.color)
            };
            let radius = 6.0 * self.zoom;
            painter.rect_filled(r, radius, visuals.widgets.inactive.bg_fill);
            painter.rect_stroke(r, radius, stroke, StrokeKind::Inside);
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                &node.label,
                font.clone(),
                visuals.text_color(),
            );
        }
    }

    fn node_rect(&self, rect: Rect, node: &hiero_core::Node) -> Rect {
        Rect::from_min_size(
            self.to_screen(rect, node.position),
            Vec2::new(node.size.width, node.size.height) * self.zoom,
        )
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let step = Self::GRID * self.zoom;
        if step < 8.0 {
            return; // too dense to be useful
        }
        let stroke = Stroke::new(1.0, color);
        let origin = rect.min + self.pan;
        let mut x = rect.min.x + (origin.x - rect.min.x).rem_euclid(step);
        while x < rect.max.x {
            painter.vline(x, rect.y_range(), stroke);
            x += step;
        }
        let mut y = rect.min.y + (origin.y - rect.min.y).rem_euclid(step);
        while y < rect.max.y {
            painter.hline(rect.x_range(), y, stroke);
            y += step;
        }
    }
}

/// Returns the point where the segment `from -> to` enters `r` (with `to` at its center).
fn clip_to_rect(from: Pos2, to: Pos2, r: Rect) -> Pos2 {
    let d = to - from;
    let half = r.size() / 2.0;
    let tx = if d.x.abs() > f32::EPSILON {
        half.x / d.x.abs()
    } else {
        f32::INFINITY
    };
    let ty = if d.y.abs() > f32::EPSILON {
        half.y / d.y.abs()
    } else {
        f32::INFINITY
    };
    let t = tx.min(ty).min(1.0);
    to - d * t
}
