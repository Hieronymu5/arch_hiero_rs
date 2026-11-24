//! A diagram's tags and scenes in the editor.
//!
//! The Tags panel, under the palette, shows or hides each tag's elements: faint while editing,
//! so they can still be picked, and gone while viewing. While editing, clicking a tag's name
//! applies it, so clicking elements puts them in it or takes them out, and the heading opens
//! Settings, where tags are added, renamed, and deleted. Which tags are hidden isn't saved:
//! each visit starts with all of them shown.
//!
//! While viewing a diagram with scenes, a Scenes panel takes the tags' place: Previous, Play or
//! Pause, and Next, then the scenes. What a scene doesn't show stays very faint; arrows show
//! once both their ends do. As the scenes step on, the camera eases to frame what the scene
//! adds, what it takes away fades, and what it brings in is built from the top down: each
//! element fades in as a line moving down the canvas reaches it, and each arrow is drawn down
//! as the line passes. Scrolling or zooming stops the camera.

use std::collections::BTreeSet;

use egui::{Color32, CursorIcon, Pos2, RichText, Sense, Stroke, Ui};
use hiero_core::engine::Laid;
use hiero_core::geometry::Rect;
use hiero_core::legend::Stamp;
use hiero_core::model::ElementId;
use hiero_core::scene::Scene;
use hiero_core::tags::{
    DiagramScene, DiagramTag, ShownElement, Sweep, SweepPace, ease, ease_in_out,
    hidden_element_ids, hidden_in_scene, leftmost_active_section, scene_frame,
};

use super::{DiagramRequest, DiagramView, Editor};
use crate::paint::{Transform, paint_scene_hiding};
use crate::theme::{choice_button, icon_button, palette};
use crate::view::View;
use crate::widgets::heading;

/// How faint an element its tags hide is drawn while editing, and while viewing a scene that
/// doesn't show it.
const EDIT_FAINT: f32 = 0.3;
const SCENE_FAINT: f32 = 0.08;
/// How long each scene shows while the scenes play, in seconds.
const SCENE_SECONDS: f64 = 3.0;
/// How long the camera takes to frame a scene, in seconds.
const CAMERA_SECONDS: f64 = 0.9;
/// How a scene is built from the top down: the line starts moving half a second in, at 350
/// canvas pixels a second, or faster so it takes at most 1.6 seconds.
const SWEEP: SweepPace = SweepPace {
    start: 0.5,
    speed: 350.0,
    longest: 1.6,
};
/// How long an element the line reaches takes to fade in, in seconds.
const APPEAR_SECONDS: f64 = 0.35;
/// How long what a scene takes away takes to fade, in seconds.
const LEAVE_SECONDS: f64 = 0.3;
/// The size of the panel's quiet icon buttons.
const ICON_BUTTON: f32 = 24.0;

/// The diagram's tags and scenes, and what's shown of them.
#[derive(Default)]
pub(super) struct TagState {
    pub tags: Vec<DiagramTag>,
    pub scenes: Vec<DiagramScene>,
    /// The ids of the tags hidden by hand.
    hidden: BTreeSet<String>,
    /// The tag being applied to elements by clicking them, while editing.
    pub applying: Option<String>,
    /// Where the scene showing is in the scenes, while viewing.
    scene: usize,
    /// While the scenes play: when the scene showing started showing, in egui's time.
    playing: Option<f64>,
    /// While the scenes step from one to another: what's shown changing.
    step: Option<Step>,
    /// What the camera is to frame, once the canvas is drawn.
    frame: Option<Frame>,
    /// The camera's move under way.
    camera: Option<Camera>,
    /// The view the camera last set, to tell when the user moves it, which stops the camera.
    camera_view: Option<View>,
    /// Set when a scene is shown, until the section it's about is scrolled into view.
    pub reveal_section: bool,
    /// What's drawn for the content and the elements hidden: the elements shown, with the
    /// arrows between them, and those hidden. At most five, for a step's two ends, what both
    /// show, and what's faint throughout it and what isn't.
    pub cache: Vec<(u64, BTreeSet<ElementId>, Scene, Scene)>,
}

/// The scenes stepping from one to another.
struct Step {
    /// The tags hidden before.
    before: BTreeSet<String>,
    /// When it started, in egui's time.
    start: f64,
    /// The line building what the scene adds, if it adds anything.
    sweep: Option<Sweep>,
    /// What changes, once worked out.
    changes: Option<Changes>,
}

/// What a step changes, each drawn on its own, so what both scenes draw, such as a container
/// both show, isn't drawn again over what's faint in it.
struct Changes {
    /// The elements the scene takes away, which fade together; their arrows go at once.
    leaving: Scene,
    /// Each element the scene brings in, with its top, which fades in whole as the line
    /// reaches it.
    entering: Vec<(f64, Scene)>,
    /// The arrows the scene brings in, with their connectors and text, drawn down as the line
    /// passes.
    arrows: Scene,
}

/// What the camera frames: an area of the canvas, or all that's drawn, legend and all.
#[derive(Debug, Clone, Copy)]
enum Frame {
    Area(Rect),
    All,
}

/// The camera easing the view from one part of the canvas to another.
#[derive(Debug, Clone, Copy)]
struct Camera {
    from: Rect,
    to: Rect,
    start: f64,
}

/// Between `a` and `b`, `k` of the way.
fn between(a: Rect, b: Rect, k: f64) -> Rect {
    let at = |x: f64, y: f64| x + (y - x) * k;
    Rect::new(
        at(a.x, b.x),
        at(a.y, b.y),
        at(a.width, b.width),
        at(a.height, b.height),
    )
}

/// An arrow's top and bottom between elements at `a` and `b`: between their facing sides when
/// one is above the other, else across where they overlap.
fn arrow_span(a: Rect, b: Rect) -> (f64, f64) {
    let (bottom, top) = (a.bottom().min(b.bottom()), a.y.max(b.y));
    (bottom.min(top), bottom.max(top))
}

impl TagState {
    /// Whether the diagram has tags or scenes, so the panel for them shows.
    pub fn has_any(&self) -> bool {
        !self.tags.is_empty() || !self.scenes.is_empty()
    }

    /// Viewing starts or stops: the scenes start again from the first, and applying stops.
    pub fn viewing_changed(&mut self) {
        self.scene = 0;
        self.playing = None;
        self.step = None;
        self.frame = None;
        self.camera = None;
        self.applying = None;
        self.cache.clear();
        // Viewing starts at the first scene, whose section is scrolled into view.
        self.reveal_section = !self.scenes.is_empty();
    }
}

/// The eye on a tag's row: open while the tag shows, struck through while it's hidden.
fn eye_icon(painter: &egui::Painter, center: Pos2, color: Color32, open: bool) {
    let scale = 16.0 / 20.0;
    let at = |x: f32, y: f32| center + egui::vec2(x - 10.0, y - 10.0) * scale;
    let stroke = Stroke::new(1.7 * scale, color);
    // The lid: two curves from corner to corner, through (10, 4.5) and (10, 15.5).
    let lid = |sign: f32| {
        (0..=16).map(move |i| {
            #[expect(clippy::cast_precision_loss, reason = "a few points")]
            let t = i as f32 / 16.0;
            let x = 1.5 + 17.0 * t;
            let bulge = 1.0 - (2.0 * t - 1.0).powi(2);
            (x, 10.0 - sign * 5.5 * bulge.powf(0.75))
        })
    };
    let points: Vec<Pos2> = lid(1.0)
        .chain(lid(-1.0).collect::<Vec<_>>().into_iter().rev().skip(1))
        .map(|(x, y)| at(x, y))
        .collect();
    painter.add(egui::Shape::closed_line(points, stroke));
    if open {
        painter.circle_stroke(at(10.0, 10.0), 2.5 * scale, stroke);
    } else {
        painter.line_segment([at(3.0, 3.0), at(17.0, 17.0)], stroke);
    }
}

fn scale_at(center: Pos2) -> impl Fn(f32, f32) -> Pos2 {
    move |x, y| center + egui::vec2(x - 10.0, y - 10.0) * (16.0 / 20.0)
}

fn previous_icon(painter: &egui::Painter, center: Pos2, color: Color32) {
    let at = scale_at(center);
    painter.rect_filled(
        egui::Rect::from_min_max(at(4.0, 4.0), at(6.0, 16.0)),
        0,
        color,
    );
    painter.add(egui::Shape::convex_polygon(
        vec![at(16.0, 4.0), at(16.0, 16.0), at(7.0, 10.0)],
        color,
        Stroke::NONE,
    ));
}

fn next_icon(painter: &egui::Painter, center: Pos2, color: Color32) {
    let at = scale_at(center);
    painter.rect_filled(
        egui::Rect::from_min_max(at(14.0, 4.0), at(16.0, 16.0)),
        0,
        color,
    );
    painter.add(egui::Shape::convex_polygon(
        vec![at(4.0, 4.0), at(13.0, 10.0), at(4.0, 16.0)],
        color,
        Stroke::NONE,
    ));
}

fn play_icon(painter: &egui::Painter, center: Pos2, color: Color32) {
    let at = scale_at(center);
    painter.add(egui::Shape::convex_polygon(
        vec![at(6.0, 4.0), at(16.0, 10.0), at(6.0, 16.0)],
        color,
        Stroke::NONE,
    ));
}

fn pause_icon(painter: &egui::Painter, center: Pos2, color: Color32) {
    let at = scale_at(center);
    painter.rect_filled(
        egui::Rect::from_min_max(at(5.0, 4.0), at(8.5, 16.0)),
        0,
        color,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(at(11.5, 4.0), at(15.0, 16.0)),
        0,
        color,
    );
}

/// A quiet icon button in the panel: no face, its icon muted, shaded in the accent while
/// hovered. Named `name` for screen readers, and toggled when `pressed` is given.
fn quiet_icon(
    ui: &mut Ui,
    name: &str,
    hint: &str,
    pressed: Option<bool>,
    icon: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ICON_BUTTON, ICON_BUTTON), Sense::click());
    let response = response
        .on_hover_text(hint)
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| match pressed {
        Some(on) => egui::WidgetInfo::selected(egui::Role::Button, true, on, name),
        None => egui::WidgetInfo::labeled(egui::Role::Button, true, name),
    });
    let colors = palette(ui);
    let color = if response.hovered() {
        ui.painter()
            .rect_filled(rect, 6, colors.accent.gamma_multiply(0.12));
        colors.accent
    } else {
        colors.text_muted
    };
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            6,
            Stroke::new(2.0, colors.accent),
            egui::StrokeKind::Inside,
        );
    }
    icon(ui.painter(), rect.center(), color);
    response
}

impl<T: DiagramView> Editor<T> {
    /// Sets the diagram's tags and scenes. Tags hidden or applied that it no longer has are
    /// forgotten.
    pub fn set_tags(&mut self, tags: &[DiagramTag], scenes: &[DiagramScene]) {
        let state = &mut self.tag;
        if state.tags == tags && state.scenes == scenes {
            return;
        }
        state.tags = tags.to_vec();
        state.scenes = scenes.to_vec();
        state.hidden.retain(|id| tags.iter().any(|t| t.id == *id));
        if state
            .applying
            .as_ref()
            .is_some_and(|id| !tags.iter().any(|t| t.id == *id))
        {
            state.applying = None;
        }
        state.scene = state.scene.min(scenes.len().saturating_sub(1));
        state.step = None;
        state.cache.clear();
    }

    /// The scene showing, and where it is in the scenes, while viewing a diagram with scenes.
    fn active_scene(&self) -> Option<(usize, &DiagramScene)> {
        if !self.viewing || self.tag.scenes.is_empty() {
            return None;
        }
        let index = self.tag.scene.min(self.tag.scenes.len() - 1);
        Some((index, &self.tag.scenes[index]))
    }

    /// The tags hidden: while viewing a scene, those it doesn't show; otherwise those hidden by
    /// hand.
    fn hidden_tag_ids(&self) -> BTreeSet<String> {
        match self.active_scene() {
            Some((_, scene)) => hidden_in_scene(&self.tag.tags, scene),
            None => self.tag.hidden.clone(),
        }
    }

    /// The elements hidden while the tags `hidden` are.
    fn hidden_by(&self, hidden: &BTreeSet<String>) -> BTreeSet<ElementId> {
        hidden_element_ids(
            self.eng.elements().iter().map(|e| {
                (
                    e.id.0.as_str(),
                    e.parent_id.as_ref().map(|p| p.0.as_str()),
                    e.tags.as_slice(),
                )
            }),
            hidden,
        )
        .into_iter()
        .map(ElementId)
        .collect()
    }

    /// The elements their tags hide now.
    pub fn tag_hidden(&self) -> BTreeSet<ElementId> {
        self.hidden_by(&self.hidden_tag_ids())
    }

    // Tags

    /// Shows a tag's elements, or hides them.
    pub fn toggle_tag(&mut self, id: &str) {
        let hidden = !self.tag.hidden.remove(id);
        if hidden {
            self.tag.hidden.insert(id.to_owned());
        }
        if let Some(tag) = self.tag.tags.iter().find(|t| t.id == id) {
            self.eng.announcement =
                format!("{} {}.", tag.name, if hidden { "hidden" } else { "shown" });
        }
    }

    /// Shows every tag, or hides every one.
    pub fn set_all_tags(&mut self, shown: bool) {
        self.tag.hidden = if shown {
            BTreeSet::new()
        } else {
            self.tag.tags.iter().map(|t| t.id.clone()).collect()
        };
        self.eng.announcement = if shown {
            "All tags shown.".into()
        } else {
            "All tags hidden.".into()
        };
    }

    /// Starts applying the tag `id` to elements by clicking them, or stops with `None`.
    pub fn apply_tag(&mut self, id: Option<&str>) {
        let tag = id.and_then(|id| self.tag.tags.iter().find(|t| t.id == id));
        if tag.map(|t| t.id.as_str()) == self.tag.applying.as_deref() {
            return;
        }
        self.tag.applying = tag.map(|t| t.id.clone());
        self.eng.announcement = match tag {
            Some(tag) => format!(
                "Applying {}: select elements to put them in it or take them out. Press Escape or select empty canvas to stop.",
                tag.name
            ),
            None => "Stopped applying the tag.".into(),
        };
    }

    /// The tag being applied, if any.
    pub fn applying_tag(&self) -> Option<&str> {
        self.tag.applying.as_deref()
    }

    /// Puts an element in the tag being applied, or takes it out when it's in it.
    pub(super) fn toggle_in_applying(&mut self, id: &ElementId) {
        let (Some(tag), Some(element)) = (self.tag.applying.clone(), self.eng.element(id)) else {
            return;
        };
        let into = !element.tags.contains(&tag);
        self.set_element_tag(id, &tag, into);
    }

    /// Puts an element in the tag `tag_id`, or takes it out, as one undo step, and says so.
    pub(super) fn set_element_tag(&mut self, id: &ElementId, tag_id: &str, into: bool) {
        let Some(tag) = self.tag.tags.iter().find(|t| t.id == tag_id).cloned() else {
            return;
        };
        let Some(element) = self.eng.element(id) else {
            return;
        };
        if element.tags.contains(&tag.id) == into {
            return;
        }
        let label = element.label.clone();
        let order: Vec<String> = self.tag.tags.iter().map(|t| t.id.clone()).collect();
        self.act(None, |eng| {
            eng.replace(id, |mut e| {
                if into {
                    e.tags.push(tag.id.clone());
                    e.tags.sort_by_key(|t| order.iter().position(|o| o == t));
                } else {
                    e.tags.retain(|t| *t != tag.id);
                }
                e
            });
        });
        self.eng.announcement = format!(
            "{label} {} {}.",
            if into { "put in" } else { "taken out of" },
            tag.name
        );
    }

    /// Rings each element in the tag being applied in the accent.
    pub(super) fn paint_applying_rings(
        &self,
        painter: &egui::Painter,
        xf: Transform,
        displayed: &[Laid<T>],
        accent: Color32,
    ) {
        let Some(applying) = &self.tag.applying else {
            return;
        };
        for element in displayed.iter().filter(|e| e.el.tags.contains(applying)) {
            let rect = xf
                .rect(T::drawn_rect(&self.eng, element))
                .expand(3.0 * xf.zoom + 1.5);
            painter.rect_stroke(rect, 6, Stroke::new(3.0, accent), egui::StrokeKind::Middle);
        }
    }

    // Scenes

    /// What a step from a scene hiding `before` to one hiding `hidden` changes: the elements it
    /// takes away, those it brings in, each on its own, and the arrows it brings in, which are
    /// what's left of the scene coming in, `now`, without what both draw, `both`, and those
    /// elements.
    fn changes(
        &self,
        before: &BTreeSet<ElementId>,
        hidden: &BTreeSet<ElementId>,
        displayed: &[Laid<T>],
        both: &Scene,
        now: &Scene,
    ) -> Changes {
        let leaving: Vec<Laid<T>> = displayed
            .iter()
            .filter(|e| hidden.contains(e.id()) && !before.contains(e.id()))
            .cloned()
            .collect();
        let leaving_ids: BTreeSet<&ElementId> = leaving.iter().map(Laid::id).collect();
        let entering: Vec<(f64, Scene)> = displayed
            .iter()
            .filter(|e| before.contains(e.id()) && !hidden.contains(e.id()))
            .map(|e| {
                let alone = self.eng.only(|id| id == e.id(), false);
                (
                    T::drawn_rect(&self.eng, e).y,
                    T::scene(&alone, std::slice::from_ref(e), None),
                )
            })
            .collect();
        let arrows = entering
            .iter()
            .fold(now.without(both), |arrows, (_, element)| {
                arrows.without(element)
            });
        Changes {
            leaving: T::scene(
                &self.eng.only(|id| leaving_ids.contains(id), false),
                &leaving,
                None,
            ),
            entering,
            arrows,
        }
    }

    /// Shows the scene at `index`. Stepping from another, the camera frames what it adds, and
    /// what it brings in is built from the top down.
    fn show_scene(&mut self, index: usize, now: f64) {
        let Some(scene) = self.tag.scenes.get(index) else {
            return;
        };
        let stepped = self.active_scene().is_some_and(|(at, _)| at != index);
        let before = self.hidden_tag_ids();
        self.eng.announcement = format!(
            "Scene {} of {}: {}.",
            index + 1,
            self.tag.scenes.len(),
            scene.name
        );
        self.tag.scene = index;
        self.tag.reveal_section = true;
        if stepped {
            let sweep = self.sweep_from(&self.hidden_by(&before));
            self.tag.step = Some(Step {
                before,
                start: now,
                sweep,
                changes: None,
            });
            self.tag.frame = self.scene_frame();
        }
    }

    /// What the camera frames for the scene showing: all that's drawn when it hides nothing,
    /// else what it shows that another scene hides, or all it shows when that's nothing.
    fn scene_frame(&self) -> Option<Frame> {
        let hidden = self.tag_hidden();
        if hidden.is_empty() {
            return Some(Frame::All);
        }
        let somewhere_hidden: BTreeSet<ElementId> = self
            .tag
            .scenes
            .iter()
            .flat_map(|scene| self.hidden_by(&hidden_in_scene(&self.tag.tags, scene)))
            .collect();
        let shown: Vec<(Rect, bool)> = self
            .eng
            .layout()
            .iter()
            .filter(|e| !hidden.contains(e.id()))
            .map(|e| {
                (
                    T::drawn_rect(&self.eng, e),
                    somewhere_hidden.contains(e.id()),
                )
            })
            .collect();
        scene_frame(&shown).map(Frame::Area)
    }

    /// The line that builds what the scene showing brings in, coming from one that hid
    /// `before`: the elements it shows that were hidden, and the arrows it draws that weren't.
    fn sweep_from(&self, before: &BTreeSet<ElementId>) -> Option<Sweep> {
        let hidden = self.tag_hidden();
        let layout = self.eng.layout();
        let rect_of = |id: &ElementId| {
            layout
                .iter()
                .find(|e| e.id() == id)
                .map(|e| T::drawn_rect(&self.eng, e))
        };
        let mut spans: Vec<(f64, f64)> = layout
            .iter()
            .filter(|e| before.contains(e.id()) && !hidden.contains(e.id()))
            .map(|e| {
                let r = T::drawn_rect(&self.eng, e);
                (r.y, r.bottom())
            })
            .collect();
        for c in self.eng.connections() {
            let ends = [&c.source_id, &c.target_id];
            let drawn_now = ends.iter().all(|id| !hidden.contains(*id));
            let drawn_before = ends.iter().all(|id| !before.contains(*id));
            if drawn_now
                && !drawn_before
                && let (Some(a), Some(b)) = (rect_of(&c.source_id), rect_of(&c.target_id))
            {
                spans.push(arrow_span(a, b));
            }
        }
        Sweep::over(&spans, SWEEP)
    }

    /// Moves the camera, called each frame once the canvas's `area` (all that's drawn) and
    /// `viewport` are known: starts framing what a scene asked for, eases on, and stops when
    /// the user has scrolled or zoomed since. Returns whether it's framing, so the section
    /// needn't be scrolled into view.
    pub(super) fn move_camera(
        &mut self,
        ctx: &egui::Context,
        area: Rect,
        viewport: egui::Rect,
        now: f64,
    ) -> bool {
        if self.tag.camera_view.is_some_and(|v| v != self.view) {
            self.tag.camera = None;
            self.tag.camera_view = None;
        }
        if let Some(frame) = self.tag.frame.take() {
            let target = match frame {
                Frame::Area(r) => r,
                Frame::All => area,
            };
            self.tag.camera = View::framing(target, viewport).map(|to| Camera {
                from: self.view.shown(viewport),
                to,
                start: now,
            });
        }
        let Some(camera) = self.tag.camera else {
            return false;
        };
        let t = ((now - camera.start) / CAMERA_SECONDS).clamp(0.0, 1.0);
        self.view
            .show(between(camera.from, camera.to, ease_in_out(t)), viewport);
        self.tag.camera_view = Some(self.view);
        if t < 1.0 {
            ctx.request_repaint();
        } else {
            self.tag.camera = None;
        }
        true
    }

    /// The zoom to remember as the user's own: none while it's the one the camera set.
    pub fn zoom_to_remember(&self) -> Option<u32> {
        (self.tag.camera_view.map(|v| v.zoom) != Some(self.view.zoom)).then(|| self.zoom_percent())
    }

    /// Scrolls the leftmost section the scene showing is active in into view, if it isn't
    /// already, so the stage of the diagram it's about can be seen: its left end, if it's too
    /// wide to fit.
    pub(super) fn reveal_scene_section(&mut self, viewport: egui::Rect) {
        if self.active_scene().is_none() {
            return;
        }
        let hidden = self.tag_hidden();
        let shown: Vec<(Rect, ShownElement)> = self
            .eng
            .layout()
            .iter()
            .filter(|e| !hidden.contains(e.id()))
            .map(|e| {
                let rect = T::drawn_rect(&self.eng, e);
                let element = ShownElement {
                    rect,
                    section: T::is_section(e),
                    tagged: !e.el.tags.is_empty(),
                };
                (rect, element)
            })
            .collect();
        let elements: Vec<ShownElement> = shown.iter().map(|(_, e)| *e).collect();
        let Some(index) = leftmost_active_section(&elements) else {
            return;
        };
        let r = shown[index].0;
        #[expect(clippy::cast_possible_truncation, reason = "canvas pixels")]
        let (left, right, top, bottom) =
            (r.x as f32, r.right() as f32, r.y as f32, r.bottom() as f32);
        self.view.reveal_x(left, right, viewport, 20.0);
        self.view.reveal_y(top, bottom, viewport, 20.0);
    }

    /// Shows the scene at `index`, stopping play.
    pub fn go_to_scene(&mut self, index: usize, now: f64) {
        self.tag.playing = None;
        self.show_scene(index, now);
    }

    /// Plays the scenes from the one showing, or from the first when it's the last, or pauses.
    pub fn toggle_play(&mut self, now: f64) {
        if self.tag.playing.take().is_some() {
            self.eng.announcement = "Paused.".into();
            return;
        }
        if self.tag.scene + 1 >= self.tag.scenes.len() {
            self.show_scene(0, now);
        }
        self.tag.playing = Some(now);
    }

    /// Steps to the next scene while they play, stopping at the last; ends a step that's
    /// over; and asks for the frames they need.
    pub(super) fn tick_scenes(&mut self, ctx: &egui::Context, now: f64) {
        if let Some(since) = self.tag.playing {
            if self.active_scene().is_none() {
                self.tag.playing = None;
            } else if now - since >= SCENE_SECONDS {
                let next = self.tag.scene + 1;
                if next < self.tag.scenes.len() {
                    self.show_scene(next, now);
                }
                self.tag.playing = (next + 1 < self.tag.scenes.len()).then_some(now);
            } else {
                #[expect(clippy::cast_possible_truncation, reason = "seconds")]
                ctx.request_repaint_after(std::time::Duration::from_secs_f32(
                    (SCENE_SECONDS - (now - since)) as f32,
                ));
            }
        }
        if let Some(step) = &self.tag.step {
            let end = step
                .sweep
                .map_or(0.0, |s| s.end(APPEAR_SECONDS))
                .max(LEAVE_SECONDS);
            if now - step.start >= end {
                self.tag.step = None;
            } else {
                ctx.request_repaint();
            }
        }
    }

    // Drawing

    /// What's drawn while the elements `hidden` are hidden: the elements shown, with the arrows
    /// between them and, while viewing, the legend; and those hidden, with no arrows.
    fn picture(
        &mut self,
        hidden: &BTreeSet<ElementId>,
        displayed: &[Laid<T>],
        dragging: bool,
    ) -> (Scene, Scene) {
        let changes = self.eng.changes();
        if !dragging
            && let Some((_, _, shown, faint)) = self
                .tag
                .cache
                .iter()
                .find(|(c, h, _, _)| *c == changes && h == hidden)
        {
            return (shown.clone(), faint.clone());
        }
        let is_hidden = |e: &Laid<T>| hidden.contains(&e.el.id);
        let shown_layout: Vec<Laid<T>> = displayed
            .iter()
            .filter(|e| !is_hidden(e))
            .cloned()
            .collect();
        let hidden_layout: Vec<Laid<T>> =
            displayed.iter().filter(|e| is_hidden(e)).cloned().collect();
        let mut shown = T::scene(
            &self.eng.only(|id| !hidden.contains(id), true),
            &shown_layout,
            self.slide(),
        );
        let faint = T::scene(
            &self.eng.only(|id| hidden.contains(id), false),
            &hidden_layout,
            None,
        );
        // While viewing, the legend sits beside the whole drawing, where it would without tags.
        if self.viewing
            && let Some(legend) = T::legend(&self.eng, Stamp::today(self.legend_version.clone()))
        {
            shown.bounds = T::scene(&self.eng, displayed, None).bounds;
            legend.add_to_canvas(&mut shown);
        }
        if !dragging {
            if self.tag.cache.len() >= 5 {
                self.tag.cache.remove(0);
            }
            self.tag
                .cache
                .push((changes, hidden.clone(), shown.clone(), faint.clone()));
        }
        (shown, faint)
    }

    /// Paints the diagram as its tags and scenes show it, and returns the scene of what's
    /// shown; or paints nothing and returns `None` when they hide nothing.
    #[expect(clippy::too_many_arguments, reason = "what the canvas is painting")]
    pub(super) fn paint_tagged(
        &mut self,
        painter: &egui::Painter,
        xf: Transform,
        displayed: &[Laid<T>],
        dragging: bool,
        dark: bool,
        hidden: &BTreeSet<ElementId>,
        now: f64,
    ) -> Option<Scene> {
        let step = self
            .tag
            .step
            .take()
            .filter(|_| self.active_scene().is_some());
        if hidden.is_empty() && step.is_none() {
            return None;
        }
        // Gone while viewing without scenes, and faint otherwise.
        let faint = if !self.viewing {
            Some(EDIT_FAINT)
        } else if self.active_scene().is_some() {
            Some(SCENE_FAINT)
        } else {
            None
        };
        let now_picture = self.picture(hidden, displayed, dragging);
        // Each picture's elements shown, at `opacity`, and with `faint`, those it hides. A step
        // draws faint only what's hidden at either end, once, so it stays as faint as ever.
        // Leaving out the text typed over in place.
        let hide = self.inline_hidden();
        let draw = |editor: &mut Self,
                    (shown, hidden): &(Scene, Scene),
                    opacity: f32,
                    with_faint: bool| {
            let cache = &mut editor.cache;
            paint_scene_hiding(painter, shown, xf, cache, opacity, dark, &hide);
            if let Some(faint) = faint.filter(|_| with_faint) {
                paint_scene_hiding(painter, hidden, xf, cache, faint, dark, &hide);
            }
        };
        match step {
            Some(mut step) => {
                let elapsed = now - step.start;
                let before = self.hidden_by(&step.before);
                // What both show.
                let either: BTreeSet<ElementId> = before.union(hidden).cloned().collect();
                let both = self.picture(&either, displayed, dragging);
                draw(self, &both, 1.0, false);
                let changes = match step.changes.take() {
                    Some(changes) if !dragging => changes,
                    _ => self.changes(&before, hidden, displayed, &both.0, &now_picture.0),
                };
                // Faint, what this step takes away or brings in, under it, so it fades to its
                // faint self or comes up from it. What both hide is drawn faint last, over all
                // that's drawn again, such as a container both show, so it never goes.
                let always: BTreeSet<ElementId> = before.intersection(hidden).cloned().collect();
                let changing: BTreeSet<ElementId> = either.difference(&always).cloned().collect();
                let paint_faint = |editor: &mut Self, ids: &BTreeSet<ElementId>| {
                    if let Some(faint) = faint.filter(|_| !ids.is_empty()) {
                        let (_, hidden) = editor.picture(ids, displayed, dragging);
                        paint_scene_hiding(
                            painter,
                            &hidden,
                            xf,
                            &mut editor.cache,
                            faint,
                            dark,
                            &hide,
                        );
                    }
                };
                paint_faint(self, &changing);
                let mut paint = |scene: &Scene, painter: &egui::Painter, opacity: f64| {
                    #[expect(clippy::cast_possible_truncation, reason = "a fraction")]
                    let opacity = opacity as f32;
                    if opacity > 0.0 {
                        paint_scene_hiding(
                            painter,
                            scene,
                            xf,
                            &mut self.cache,
                            opacity,
                            dark,
                            &hide,
                        );
                    }
                };
                // What the one before showed and this one doesn't fades to its faint self.
                paint(
                    &changes.leaving,
                    painter,
                    1.0 - ease(elapsed / LEAVE_SECONDS),
                );
                // What this one brings in: each element fades in whole as the line moving down
                // reaches its top, and the arrows are drawn down as it passes.
                if let Some(sweep) = step.sweep {
                    for (top, scene) in &changes.entering {
                        let delay = sweep.start + (top - sweep.top) / sweep.speed;
                        paint(scene, painter, ease((elapsed - delay) / APPEAR_SECONDS));
                    }
                    let clip = painter.clip_rect();
                    let line = xf
                        .pos(hiero_core::geometry::Point::new(
                            0.0,
                            sweep.line_at(elapsed),
                        ))
                        .y;
                    if line > clip.top() {
                        let above = egui::Rect::from_x_y_ranges(clip.x_range(), clip.top()..=line);
                        paint(&changes.arrows, &painter.with_clip_rect(above), 1.0);
                    }
                }
                paint_faint(self, &always);
                step.changes = Some(changes);
                self.tag.step = Some(step);
            }
            None => draw(self, &now_picture, 1.0, true),
        }
        Some(now_picture.0)
    }

    // The panel

    /// The Tags panel, or while viewing a diagram with scenes, the Scenes panel.
    pub(super) fn tags_ui(&mut self, ui: &mut Ui) {
        if self.viewing && !self.tag.scenes.is_empty() {
            self.scenes_ui(ui);
        } else if !self.tag.tags.is_empty() {
            self.tag_list_ui(ui);
        }
    }

    fn tag_list_ui(&mut self, ui: &mut Ui) {
        let colors = palette(ui);
        let hidden_count = self
            .tag
            .tags
            .iter()
            .filter(|t| self.tag.hidden.contains(&t.id))
            .count();
        let all_shown = hidden_count == 0;
        let none_shown = hidden_count == self.tag.tags.len();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if self.viewing {
                heading(ui, "Tags");
            } else {
                // The heading opens where tags are added, renamed, and deleted.
                ui.add_space(4.0);
                let text = RichText::new("Tags").font(crate::fonts::bold(15.0));
                let response = ui
                    .add(egui::Label::new(text).sense(Sense::click()))
                    .on_hover_text("Add, rename, and delete tags")
                    .on_hover_cursor(CursorIcon::PointingHand);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Link,
                        true,
                        "Tags: add, rename, and delete tags",
                    )
                });
                if response.hovered() {
                    let rect = response.rect;
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom() - 1.0,
                        Stroke::new(1.0, colors.accent),
                    );
                }
                if response.clicked() {
                    self.request = Some(DiagramRequest::EditTags);
                }
            }
            // The eye for every tag: faint while only some show.
            let hint = if all_shown {
                "Hide all tags"
            } else {
                "Show all tags"
            };
            let eye = quiet_icon(ui, "Show all tags", hint, Some(all_shown), |p, c, col| {
                let col = if !all_shown && !none_shown {
                    col.gamma_multiply(0.5)
                } else {
                    col
                };
                eye_icon(p, c, col, !none_shown);
            });
            if eye.clicked() {
                self.set_all_tags(!all_shown);
            }
        });
        ui.add_space(4.0);
        let mut toggle = None;
        for tag in &self.tag.tags {
            let hidden = self.tag.hidden.contains(&tag.id);
            let applying = self.tag.applying.as_deref() == Some(tag.id.as_str());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let verb = if hidden { "Show" } else { "Hide" };
                // The whole row shows or hides the tag. The tag being applied, from the
                // toolbar's tag button, has its name in the accent, and bold.
                let width = ui.available_width();
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(width, ICON_BUTTON), Sense::click());
                let response = response
                    .on_hover_text(format!("{verb} {}", tag.name))
                    .on_hover_cursor(CursorIcon::PointingHand);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        true,
                        !hidden,
                        format!("Show {}", tag.name),
                    )
                });
                let color = if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, 6, colors.accent.gamma_multiply(0.12));
                    colors.accent
                } else {
                    colors.text_muted
                };
                eye_icon(
                    ui.painter(),
                    egui::pos2(rect.left() + ICON_BUTTON / 2.0, rect.center().y),
                    color,
                    !hidden,
                );
                // A long name is cut short at the row's end, ending in "…".
                let body = egui::TextStyle::Body.resolve(ui.style());
                let font = if applying {
                    crate::fonts::font_id(body.size, hiero_core::text::Weight::SemiBold)
                } else {
                    body
                };
                let mut job = egui::text::LayoutJob::simple_singleline(
                    tag.name.clone(),
                    font,
                    Color32::PLACEHOLDER,
                );
                job.wrap =
                    egui::text::TextWrapping::truncate_at_width(rect.width() - ICON_BUTTON - 8.0);
                let galley = ui.painter().layout_job(job);
                let text_color = if hidden {
                    colors.text_muted
                } else if response.hovered() || applying {
                    colors.accent
                } else {
                    colors.text
                };
                let at = egui::pos2(
                    rect.left() + ICON_BUTTON + 4.0,
                    rect.center().y - galley.size().y / 2.0,
                );
                if hidden {
                    let y = rect.center().y;
                    ui.painter().hline(
                        at.x..=at.x + galley.size().x,
                        y,
                        Stroke::new(1.0, colors.text_muted),
                    );
                }
                ui.painter().galley(at, galley, text_color);
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        rect,
                        6,
                        Stroke::new(2.0, colors.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    toggle = Some(tag.id.clone());
                }
            });
        }
        if let Some(id) = toggle {
            self.toggle_tag(&id);
        }
    }

    fn scenes_ui(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        let (index, _) = self.active_scene().expect("viewing with scenes");
        let last = self.tag.scenes.len() - 1;
        let playing = self.tag.playing.is_some();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let width = (ui.available_width() - 12.0) / 3.0;
            let previous = ui.add_enabled_ui(index > 0, |ui| {
                icon_button(ui, previous_icon, "Previous scene", width)
            });
            if previous.inner.clicked() {
                self.go_to_scene(index.saturating_sub(1), now);
            }
            let (icon, name): (crate::theme::ButtonIcon, _) = if playing {
                (pause_icon, "Pause")
            } else {
                (play_icon, "Play the scenes")
            };
            if icon_button(ui, icon, name, width).clicked() {
                self.toggle_play(now);
            }
            let next = ui.add_enabled_ui(index < last, |ui| {
                icon_button(ui, next_icon, "Next scene", width)
            });
            if next.inner.clicked() {
                self.go_to_scene((index + 1).min(last), now);
            }
        });
        ui.add_space(8.0);
        let mut go = None;
        for (i, scene) in self.tag.scenes.iter().enumerate() {
            if choice_button(ui, &scene.name, 28.0, i == index).clicked() {
                go = Some(i);
            }
        }
        if let Some(i) = go {
            self.go_to_scene(i, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arrow_spans_between_facing_sides_or_across_the_overlap() {
        let above = Rect::new(0.0, 0.0, 100.0, 60.0);
        let below = Rect::new(0.0, 200.0, 100.0, 60.0);
        assert_eq!(arrow_span(above, below), (60.0, 200.0));
        assert_eq!(arrow_span(below, above), (60.0, 200.0));
        // Side by side, overlapping from 30 to 60: across the overlap.
        let beside = Rect::new(300.0, 30.0, 100.0, 60.0);
        assert_eq!(arrow_span(above, beside), (30.0, 60.0));
    }

    #[test]
    fn the_camera_eases_between_two_areas() {
        let a = Rect::new(0.0, 0.0, 100.0, 50.0);
        let b = Rect::new(100.0, 100.0, 300.0, 150.0);
        assert_eq!(between(a, b, 0.0), a);
        assert_eq!(between(a, b, 1.0), b);
        assert_eq!(between(a, b, 0.5), Rect::new(50.0, 50.0, 200.0, 100.0));
    }
}
