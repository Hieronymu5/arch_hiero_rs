//! The diagram editor every diagram type shares: a palette, a zoomable canvas where elements
//! are selected, moved, resized, connected, and removed with the pointer or the keyboard, and
//! a properties panel each type fills in. Changes are reported to the app, which saves them
//! and keeps their undo history.

use std::borrow::Cow;
use std::collections::BTreeMap;

use egui::{Align2, Color32, CursorIcon, Id, Key, Pos2, RichText, Sense, Stroke, Ui, Vec2};
use hiero_core::engine::{
    Conn, Corner, DiagramType, Dir, El, Engine, Laid, Placement, validate_label,
};
use hiero_core::geometry::{CANVAS_HEIGHT, CANVAS_WIDTH, GRID, Point, Rect};
use hiero_core::model::{ColorScheme, ConnectionId, ElementId};
use hiero_core::scene::Scene;
use hiero_core::stored::{ContentError, StoredContent};

use crate::paint::{PaintCache, Transform, color, paint_scene, world_line};
use crate::theme::{element_button, page_button};
use crate::view::{View, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP};
use crate::widgets::{self, heading, select, text_field};

/// An arrow as drawn, for selecting it with the pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrowLine {
    pub id: ConnectionId,
    pub from: Point,
    pub to: Point,
    /// Boxes on the arrow, such as its connector, which select it too.
    pub boxes: Vec<Rect>,
}

/// What a diagram type adds to the shared editor: how it's drawn, and its properties.
pub trait DiagramView: DiagramType {
    /// The editor's instructions.
    const HELP: &'static str;

    /// The diagram drawn from `layout`, with an arrow being slid at `slide`'s offset.
    fn scene(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        slide: Option<(&ConnectionId, f64)>,
    ) -> Scene;

    /// The arrows as drawn.
    fn arrow_lines(
        _eng: &Engine<Self>,
        _layout: &[Laid<Self>],
        _slide: Option<(&ConnectionId, f64)>,
    ) -> Vec<ArrowLine> {
        Vec::new()
    }

    /// The type's fields for the selected element.
    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>);

    /// The type's fields for the selected arrow.
    fn arrow_properties(_ui: &mut Ui, _cx: &mut PropsCx<'_, Self>, _connection: &Conn<Self>) {}

    /// Such as "To CRM via API", in the list of an element's arrows.
    fn arrow_title(
        _eng: &Engine<Self>,
        _connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        format!("{} {other}", if outgoing { "To" } else { "From" })
    }

    /// The offset an arrow being slid sideways to `pointer` takes, if arrows can be slid.
    fn slide_offset(_eng: &Engine<Self>, _id: &ConnectionId, _pointer: Point) -> Option<f64> {
        None
    }

    /// Runs a slid arrow straight at `offset`.
    fn commit_slide(_eng: &mut Engine<Self>, _id: &ConnectionId, _offset: f64) {}

    /// An arrow key while an arrow is selected. Returns whether it was used.
    fn connection_key(_eng: &mut Engine<Self>, _dir: Dir, _shift: bool) -> bool {
        false
    }
}

/// A change to apply from a properties panel.
type Op<T> = Box<dyn FnOnce(&mut Engine<T>)>;

/// What a properties panel works with: the engine to read, and changes to apply after it's
/// drawn, each grouped with the field it came from so typing is undone in one step.
pub struct PropsCx<'a, T: DiagramType> {
    pub eng: &'a Engine<T>,
    ops: Vec<(Option<String>, Op<T>)>,
    salt: Id,
}

impl<T: DiagramType> PropsCx<'_, T> {
    /// The id of an element's field, so what's typed in it is kept until it's selected again.
    pub fn field_id(&self, element: &ElementId, field: &str) -> Id {
        self.salt.with((element.as_str(), field))
    }

    pub fn arrow_field_id(&self, arrow: &ConnectionId, field: &str) -> Id {
        self.salt.with(("arrow", arrow.as_str(), field))
    }

    /// Applies `change` after the panel is drawn. Changes with the same `group` in a row are
    /// undone together.
    pub fn apply(&mut self, group: Option<String>, change: impl FnOnce(&mut Engine<T>) + 'static) {
        self.ops.push((group, Box::new(change)));
    }

    /// The element's name field. Updates `with` and returns whether it changed.
    pub fn name_field(&self, ui: &mut Ui, element: &Laid<T>, with: &mut El<T>) -> bool {
        match text_field(
            ui,
            self.field_id(element.id(), "label"),
            "Name",
            &element.el.label,
            false,
            validate_label,
        ) {
            Some(label) => {
                with.label = label;
                true
            }
            None => false,
        }
    }

    /// A field for the container the element sits in, when it could sit in any. Returns the
    /// new container when it changed. With `required`, there's no empty choice.
    pub fn parent_field(
        &self,
        ui: &mut Ui,
        element: &Laid<T>,
        required: bool,
    ) -> Option<Option<ElementId>> {
        let label = self.eng.parent_field_label()?;
        let options = self.eng.parent_options();
        select(
            ui,
            self.field_id(element.id(), "parent"),
            &label,
            &element.el.parent_id,
            (!required).then_some("None"),
            &options,
        )
    }

    /// Applies an edited element if it's valid: its fields, and its container when `parent`
    /// says it moved.
    pub fn commit(
        &mut self,
        element: &Laid<T>,
        with: El<T>,
        parent: Option<Option<ElementId>>,
        field: &str,
    ) {
        if T::validate_element(&with).is_err() {
            return;
        }
        let id = element.id().clone();
        let parent = parent.unwrap_or_else(|| element.el.parent_id.clone());
        self.apply(Some(format!("{id}:{field}")), move |eng| {
            T::apply_element(eng, &id, with, parent);
        });
    }
}

/// A change the editor made, for the app to save and add to the undo history.
#[derive(Debug, Clone)]
pub struct Change {
    /// The content before the change.
    pub before: StoredContent,
    /// Changes with the same group in a row are undone together, such as typing in a field.
    pub group: Option<String>,
}

/// What the pointer is doing on the canvas.
enum Drag {
    None,
    Pan {
        scroll: Vec2,
        pointer: Pos2,
    },
    Move {
        id: ElementId,
        grab: Point,
        group: bool,
        moved: bool,
        placements: Vec<(ElementId, Placement)>,
    },
    Resize {
        id: ElementId,
        corner: Corner,
        start: Point,
        rect: Rect,
    },
    Connect {
        source: ElementId,
        pointer: Point,
    },
    Slide {
        id: ConnectionId,
        start: Point,
        moved: bool,
        offset: Option<f64>,
    },
}

/// What has keyboard focus on the canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Focus {
    Canvas,
    Element(ElementId),
    Handle(ElementId, Corner),
}

/// A palette item being dragged onto the canvas.
#[derive(Debug, Clone, Copy)]
struct PaletteDrag(usize);

/// The colours of the editor's own marks, over the white paper whatever the theme.
const SELECTION: Color32 = Color32::from_rgb(37, 99, 235);
const HANDLE_FILL: Color32 = Color32::WHITE;
/// The size of handles on screen, whatever the zoom.
const HANDLE: f32 = 12.0;
const CONNECT_RADIUS: f32 = 7.0;

/// The diagram editor for diagram type `T`.
pub struct Editor<T: DiagramView> {
    pub eng: Engine<T>,
    view: View,
    drag: Drag,
    focus: Focus,
    selection_before_focus: Option<Vec<ElementId>>,
    cache: PaintCache,
    scene_cache: Option<(u64, Scene)>,
    outbox: Vec<Change>,
    pub show_help: bool,
    focus_name: bool,
    salt: Id,
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() == 0.0 {
        0.0
    } else {
        ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
    };
    (a + ab * t - p).length()
}

impl<T: DiagramView> Editor<T> {
    pub fn new(eng: Engine<T>, salt: Id) -> Self {
        Self {
            eng,
            view: View::default(),
            drag: Drag::None,
            focus: Focus::Canvas,
            selection_before_focus: None,
            cache: PaintCache::default(),
            scene_cache: None,
            outbox: Vec::new(),
            show_help: false,
            focus_name: false,
            salt,
        }
    }

    /// An editor for stored content.
    pub fn from_stored(
        content: &StoredContent,
        colors: &BTreeMap<String, String>,
        salt: Id,
    ) -> Result<Self, ContentError> {
        let (elements, connections) = content.to_typed()?;
        Ok(Self::new(
            Engine::new(elements, connections, T::Colors::from_overrides(colors)),
            salt,
        ))
    }

    pub fn content(&self) -> StoredContent {
        StoredContent::from_typed(self.eng.elements(), self.eng.connections())
    }

    /// Runs an action on the engine, reporting it as a change when it changed anything.
    pub fn act(&mut self, group: Option<String>, action: impl FnOnce(&mut Engine<T>)) {
        let changes = self.eng.changes();
        let elements = self.eng.elements().to_vec();
        let connections = self.eng.connections().to_vec();
        action(&mut self.eng);
        if self.eng.changes() != changes {
            self.outbox.push(Change {
                before: StoredContent::from_typed(&elements, &connections),
                group,
            });
        }
    }

    /// The changes made since last asked.
    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.outbox)
    }

    /// Replaces the content, such as for undo or a change made elsewhere. Isn't reported as a
    /// change.
    pub fn replace_content(&mut self, content: &StoredContent) -> Result<(), ContentError> {
        let (elements, connections) = content.to_typed()?;
        self.eng.replace_content(elements, connections);
        self.drag = Drag::None;
        Ok(())
    }

    pub fn set_colors(&mut self, overrides: &BTreeMap<String, String>) {
        self.eng.set_colors(T::Colors::from_overrides(overrides));
    }

    /// The diagram as stored, drawn.
    pub fn scene(&self) -> Scene {
        T::scene(&self.eng, self.eng.layout(), None)
    }

    // Layout of the editor

    /// The palette on the left, the canvas filling the rest, and the selected element's or
    /// arrow's properties on the right while there's one to edit.
    pub fn ui(&mut self, ui: &mut Ui) {
        // Content lines up with the logo, CONTENT_INSET in from the edge.
        let inset = crate::theme::CONTENT_INSET;
        if self.show_help {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: inset,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(T::HELP);
                    });
                });
            ui.add_space(4.0);
        }
        let palette_frame = egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin {
            left: inset,
            // The page's 8pt margin shows left of the bar but not right of it, as the bar
            // reaches past it to the canvas, so the right side adds it to match.
            right: inset + 8,
            top: 2,
            bottom: 2,
        });
        // The bar is just wide enough for the longest name, "Load Balancer". Its id carries
        // the default width, so a width saved before the default changed is dropped.
        const ELEMENTS_WIDTH: u16 = 149;
        egui::Panel::left(self.salt.with(("elements", ELEMENTS_WIDTH)))
            .frame(palette_frame)
            .resizable(true)
            .default_size(f32::from(ELEMENTS_WIDTH))
            .size_range(134.0..=360.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| self.palette_ui(ui));
            });
        // With nothing or several selected, the canvas gets the room.
        let editing = self.eng.selected_connection.is_some() || self.eng.selected().is_some();
        if editing {
            egui::Panel::right(self.salt.with("properties"))
                .resizable(true)
                .default_size(290.0)
                .size_range(220.0..=480.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            crate::theme::form_style(ui);
                            heading(ui, "Properties");
                            self.properties_body(ui);
                        });
                });
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            self.canvas_ui(ui, rect);
            // A polite live region that isn't drawn: screen readers hear what each change did.
            let (_, status) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
            let text = self.eng.announcement.clone();
            ui.ctx().accesskit_node_builder(status.id, |node| {
                node.set_role(egui::accesskit::Role::Status);
                node.set_live(egui::accesskit::Live::Polite);
                node.set_label(text);
            });
        });
    }

    fn palette_ui(&mut self, ui: &mut Ui) {
        heading(ui, "Elements");
        ui.label(
            RichText::new("Drag onto the canvas, or click to add.")
                .weak()
                .small(),
        );
        for (index, kind) in T::palette().iter().enumerate() {
            let id = self.salt.with(("palette", index));
            let response = ui
                .dnd_drag_source(id, PaletteDrag(index), |ui| {
                    element_button(ui, kind.label, 28.0)
                })
                .inner;
            if response.clicked() {
                let kind = kind.kind;
                self.act(None, |eng| eng.add_at_free_position(kind));
                self.focus = Focus::Element(
                    self.eng
                        .selected_id()
                        .cloned()
                        .unwrap_or_else(|| ElementId(String::new())),
                );
            }
        }
    }

    /// The selected element's or arrow's properties. Shown only while there's one.
    fn properties_body(&mut self, ui: &mut Ui) {
        let mut cx = PropsCx {
            eng: &self.eng,
            ops: Vec::new(),
            salt: self.salt.with("fields"),
        };
        let mut select_arrow: Option<ConnectionId> = None;
        let mut remove_arrow: Option<ConnectionId> = None;
        let mut add_arrow: Option<(ElementId, ElementId)> = None;
        let mut remove_selection = false;
        let mut focus_name_id = None;
        if let Some(id) = cx.eng.selected_connection.clone()
            && let Some(connection) = cx.eng.connection(&id).cloned()
        {
            let title = cx
                .eng
                .arrow_name(&connection.source_id, &connection.target_id);
            ui.label(RichText::new(title).strong());
            T::arrow_properties(ui, &mut cx, &connection);
            ui.add_space(6.0);
            if page_button(ui, "Remove arrow", false).clicked() {
                remove_arrow = Some(id);
            }
        } else if let Some(element) = cx.eng.selected().cloned() {
            ui.label(RichText::new(element.info.label).weak());
            T::element_properties(ui, &mut cx, &element);
            focus_name_id = Some(cx.field_id(element.id(), "label").with("edit"));
            if T::ID.has_arrows() {
                let arrows = cx.eng.arrows_of(element.id());
                let can_draw = Engine::<T>::can_connect_from(&element);
                if !arrows.is_empty() || can_draw {
                    heading(ui, "Arrows");
                }
                for (connection, outgoing, other) in arrows {
                    let title = T::arrow_title(cx.eng, connection, outgoing, &other);
                    if ui.link(title).clicked() {
                        select_arrow = Some(connection.id.clone());
                    }
                }
                if can_draw {
                    let options = cx.eng.arrow_target_options();
                    if !options.is_empty() {
                        let target_id = cx.field_id(element.id(), "arrow-target");
                        let mut target: Option<ElementId> =
                            ui.data_mut(|d| d.get_temp(target_id)).flatten();
                        if let Some(choice) = select(
                            ui,
                            target_id.with("select"),
                            "Add arrow to",
                            &target,
                            Some("Select a target"),
                            &options,
                        ) {
                            target = choice;
                        }
                        ui.data_mut(|d| d.insert_temp(target_id, target.clone()));
                        let add = ui.add_enabled_ui(target.is_some(), |ui| {
                            page_button(ui, "Add arrow", false)
                        });
                        if add.inner.clicked()
                            && let Some(target) = target
                        {
                            add_arrow = Some((element.id().clone(), target));
                            ui.data_mut(|d| d.insert_temp::<Option<ElementId>>(target_id, None));
                        }
                    }
                }
            }
            ui.add_space(6.0);
            if page_button(ui, &format!("Remove {}", element.el.label), false).clicked() {
                remove_selection = true;
            }
        }
        let ops = std::mem::take(&mut cx.ops);
        drop(cx);
        for (group, op) in ops {
            self.act(group, op);
        }
        if let Some(id) = select_arrow {
            self.eng.select_connection(&id);
        }
        if let Some(id) = remove_arrow {
            self.act(None, |eng| eng.remove_connection(&id));
        }
        if let Some((source, target)) = add_arrow {
            self.act(None, |eng| {
                eng.add_connection(&source, &target);
            });
        }
        if remove_selection {
            self.act(None, |eng| {
                eng.delete_selection();
            });
            self.focus = Focus::Canvas;
        }
        if self.focus_name
            && let Some(id) = focus_name_id
        {
            ui.memory_mut(|m| m.request_focus(id));
            self.focus_name = false;
        }
    }

    // The canvas

    fn displayed(&self) -> Cow<'_, [Laid<T>]> {
        match &self.drag {
            Drag::Move {
                placements,
                moved: true,
                ..
            } => Cow::Owned(self.eng.preview_placements(placements)),
            Drag::Resize { id, rect, .. } => Cow::Owned(self.eng.preview_resize(id, *rect)),
            _ => Cow::Borrowed(self.eng.layout()),
        }
    }

    fn slide(&self) -> Option<(&ConnectionId, f64)> {
        match &self.drag {
            Drag::Slide {
                id,
                moved: true,
                offset: Some(offset),
                ..
            } => Some((id, *offset)),
            _ => None,
        }
    }

    /// The element under a world point: figures and boxes over the containers they're in.
    fn element_at(layout: &[Laid<T>], p: Point) -> Option<&Laid<T>> {
        layout
            .iter()
            .rev()
            .find(|e| !e.is_container && e.rect.contains_point(p))
            .or_else(|| layout.iter().rev().find(|e| e.rect.contains_point(p)))
    }

    fn connect_handle(&self, xf: &Transform, element: &Laid<T>) -> Pos2 {
        let r = T::arrow_rect(&self.eng, element);
        let p = xf.pos(Point::new(r.right(), r.y + r.height / 2.0));
        p + Vec2::new(CONNECT_RADIUS + 6.0, 0.0)
    }

    fn handles(&self, element: &Laid<T>) -> Vec<Corner> {
        if element.is_resizable {
            vec![Corner::TopLeft, Corner::BottomRight]
        } else {
            Vec::new()
        }
    }

    fn canvas_ui(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let id = self.salt.with("canvas");
        let response = ui.interact(rect, id, Sense::click_and_drag());
        // Screen readers hear the focused element's name and details, as the prototype's
        // hidden text gave them.
        let focused_label = self
            .focused_element()
            .map(|e| self.eng.accessible_label(&e))
            .or_else(|| {
                self.eng
                    .selected_connection
                    .as_ref()
                    .and_then(|c| self.eng.connection(c))
                    .map(|c| self.eng.arrow_name(&c.source_id, &c.target_id))
            });
        let label = match focused_label {
            Some(l) => format!("Diagram canvas: {l}"),
            None => format!(
                "Diagram canvas, {} elements. Press Tab to move through them.",
                self.eng.elements().len()
            ),
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &label));
        self.view.clamp(rect);
        let painter = ui.painter_at(rect);
        let dark = ui.visuals().dark_mode;
        painter.rect_filled(
            rect,
            0.0,
            if dark {
                Color32::from_gray(40)
            } else {
                Color32::from_gray(232)
            },
        );
        let xf = self.view.transform(rect);
        let paper = xf.rect(Rect::new(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT));
        painter.rect_filled(paper, 0.0, Color32::WHITE);
        self.paint_grid(&painter, xf, rect);

        // Input that changes the view.
        let (zoom_delta, scroll_delta, hover) = ui.input(|i| {
            (
                i.zoom_delta(),
                i.smooth_scroll_delta(),
                i.pointer.hover_pos(),
            )
        });
        if response.hovered() {
            if (zoom_delta - 1.0).abs() > f32::EPSILON
                && let Some(p) = hover
            {
                self.view.zoom_around(self.view.zoom * zoom_delta, p, rect);
            } else if scroll_delta != Vec2::ZERO {
                self.view.scroll += scroll_delta;
                self.view.clamp(rect);
            }
        }
        let xf = self.view.transform(rect);

        self.pointer_input(ui, &response, xf, rect);
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
            self.keyboard_input(ui, rect);
        }

        // Paint the diagram.
        let xf = self.view.transform(rect);
        let dragging = !matches!(self.drag, Drag::None | Drag::Pan { .. });
        let displayed = self.displayed().into_owned();
        let scene = if dragging {
            T::scene(&self.eng, &displayed, self.slide())
        } else {
            match &self.scene_cache {
                Some((changes, scene)) if *changes == self.eng.changes() => scene.clone(),
                _ => {
                    let scene = T::scene(&self.eng, &displayed, None);
                    self.scene_cache = Some((self.eng.changes(), scene.clone()));
                    scene
                }
            }
        };
        paint_scene(&painter, &scene, xf, &mut self.cache, 1.0);
        self.paint_overlays(&painter, xf, &displayed, &response, ui);
        self.zoom_control(ui, rect);
    }

    fn paint_grid(&self, painter: &egui::Painter, xf: Transform, clip: egui::Rect) {
        let step = GRID as f32 * xf.zoom;
        if step < 6.0 {
            return;
        }
        let dot = Color32::from_gray(200);
        let paper = xf
            .rect(Rect::new(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT))
            .intersect(clip);
        let first_x = ((paper.min.x - xf.origin.x) / step).ceil();
        let first_y = ((paper.min.y - xf.origin.y) / step).ceil();
        let mut y = xf.origin.y + first_y * step;
        while y <= paper.max.y {
            let mut x = xf.origin.x + first_x * step;
            while x <= paper.max.x {
                painter.circle_filled(Pos2::new(x, y), 1.0, dot);
                x += step;
            }
            y += step;
        }
    }

    fn paint_overlays(
        &self,
        painter: &egui::Painter,
        xf: Transform,
        layout: &[Laid<T>],
        response: &egui::Response,
        ui: &Ui,
    ) {
        let focused = response.has_focus();
        // Selected arrow.
        let lines = T::arrow_lines(&self.eng, layout, self.slide());
        if let Some(selected) = &self.eng.selected_connection
            && let Some(line) = lines.iter().find(|l| l.id == *selected)
        {
            painter.line_segment(
                [xf.pos(line.from), xf.pos(line.to)],
                Stroke::new(6.0, SELECTION.gamma_multiply(0.45)),
            );
            for b in &line.boxes {
                painter.rect_stroke(
                    xf.rect(*b).expand(2.0),
                    3.0,
                    Stroke::new(2.0, SELECTION),
                    egui::StrokeKind::Outside,
                );
            }
        }
        for element in layout {
            let selected = self.eng.is_selected(element.id());
            let is_focused = focused && self.focus == Focus::Element(element.id().clone());
            if !selected && !is_focused {
                continue;
            }
            let r = xf.rect(element.rect).expand(3.0);
            if selected {
                painter.rect_stroke(
                    r,
                    4.0,
                    Stroke::new(2.0, SELECTION),
                    egui::StrokeKind::Outside,
                );
            }
            if is_focused {
                let corners = [
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                    r.left_top(),
                ];
                painter.extend(egui::Shape::dashed_line(
                    &corners,
                    Stroke::new(1.5, Color32::from_rgb(234, 88, 12)),
                    5.0,
                    3.0,
                ));
            }
        }
        if let Some(element) = self
            .eng
            .selected()
            .and_then(|s| layout.iter().find(|l| l.id() == s.id()))
        {
            for corner in self.handles(element) {
                let c = xf.pos(Engine::<T>::handle_position(element, corner));
                let r = egui::Rect::from_center_size(c, Vec2::splat(HANDLE));
                let focused_handle =
                    focused && self.focus == Focus::Handle(element.id().clone(), corner);
                painter.rect_filled(r, 2.0, HANDLE_FILL);
                painter.rect_stroke(
                    r,
                    2.0,
                    Stroke::new(if focused_handle { 3.0 } else { 1.5 }, SELECTION),
                    egui::StrokeKind::Inside,
                );
            }
            if T::ID.has_arrows() && Engine::<T>::can_connect_from(element) {
                let c = self.connect_handle(&xf, element);
                painter.circle(c, CONNECT_RADIUS, HANDLE_FILL, Stroke::new(2.0, SELECTION));
                painter.line_segment(
                    [c - Vec2::new(3.0, 0.0), c + Vec2::new(3.0, 0.0)],
                    Stroke::new(1.5, SELECTION),
                );
                painter.line_segment(
                    [c - Vec2::new(0.0, 3.0), c + Vec2::new(0.0, 3.0)],
                    Stroke::new(1.5, SELECTION),
                );
            }
        }
        if let Drag::Connect { source, pointer } = &self.drag
            && let Some(element) = layout.iter().find(|l| l.id() == source)
        {
            let from = T::arrow_rect(&self.eng, element).center();
            world_line(
                painter,
                xf,
                &[from, *pointer],
                Stroke::new(2.0, SELECTION),
                true,
            );
            if let Some(target) = self.eng.connect_target_at(source, *pointer)
                && let Some(t) = layout.iter().find(|l| *l.id() == target)
            {
                painter.rect_stroke(
                    xf.rect(t.rect).expand(3.0),
                    4.0,
                    Stroke::new(2.0, SELECTION),
                    egui::StrokeKind::Outside,
                );
            }
        }
        // A palette item being dragged over the canvas.
        if let Some(payload) = response.dnd_hover_payload::<PaletteDrag>()
            && let Some(p) = ui.input(|i| i.pointer.hover_pos())
            && let Some(kind) = T::palette().get(payload.0)
        {
            let size = T::new_size_of(&self.eng, kind);
            let world = xf.world(p);
            let ghost = Rect::new(
                world.x - size.width / 2.0,
                world.y - size.height / 2.0,
                size.width,
                size.height,
            );
            painter.rect_stroke(
                xf.rect(ghost),
                4.0,
                Stroke::new(1.5, SELECTION),
                egui::StrokeKind::Middle,
            );
            painter.text(
                xf.pos(ghost.center()),
                Align2::CENTER_CENTER,
                kind.label,
                egui::FontId::proportional(13.0 * xf.zoom.max(0.6)),
                SELECTION,
            );
        }
        let _ = color;
    }

    fn zoom_control(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let area = egui::Rect::from_min_size(
            rect.right_bottom() - Vec2::new(250.0, 40.0),
            Vec2::new(240.0, 32.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Zoom");
                    let mut percent = (self.view.zoom * 100.0).round();
                    let slider =
                        egui::Slider::new(&mut percent, (ZOOM_MIN * 100.0)..=(ZOOM_MAX * 100.0))
                            .step_by(f64::from(ZOOM_STEP * 100.0))
                            .suffix("%");
                    if ui.add(slider).changed() {
                        self.view.zoom_around(percent / 100.0, rect.center(), rect);
                    }
                });
            });
        });
    }

    // Pointer

    fn pointer_input(
        &mut self,
        ui: &mut Ui,
        response: &egui::Response,
        xf: Transform,
        rect: egui::Rect,
    ) {
        if let Some(payload) = response.dnd_release_payload::<PaletteDrag>()
            && let Some(p) = ui.input(|i| i.pointer.interact_pos())
            && let Some(kind) = T::palette().get(payload.0)
        {
            let kind = kind.kind;
            let world = xf.world(p);
            self.act(None, |eng| eng.palette_drop(kind, world));
            response.request_focus();
            self.focus = self
                .eng
                .selected_id()
                .cloned()
                .map_or(Focus::Canvas, Focus::Element);
            return;
        }
        let (pressed, down, released, pos, modifiers) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.interact_pos(),
                i.modifiers,
            )
        });
        let Some(pos) = pos else {
            return;
        };
        let world = xf.world(pos);
        if pressed && response.hovered() {
            response.request_focus();
            self.press(pos, world, modifiers, xf);
        } else if down {
            self.drag_to(pos, world, rect);
        }
        if released {
            self.release(world);
        }
        // Cursor hints.
        if response.hovered() {
            let cursor = match &self.drag {
                Drag::Pan { .. } => CursorIcon::Grabbing,
                Drag::Move { .. } => CursorIcon::Move,
                Drag::Resize { .. } => CursorIcon::ResizeNwSe,
                Drag::Connect { .. } => CursorIcon::Crosshair,
                Drag::Slide { .. } => CursorIcon::ResizeVertical,
                Drag::None => {
                    if Self::element_at(self.eng.layout(), world).is_some() {
                        CursorIcon::PointingHand
                    } else {
                        CursorIcon::Default
                    }
                }
            };
            ui.ctx().set_cursor_icon(cursor);
        }
    }

    fn press(&mut self, pos: Pos2, world: Point, modifiers: egui::Modifiers, xf: Transform) {
        // Handles of the selected element.
        if let Some(selected) = self.eng.selected().cloned() {
            for corner in self.handles(&selected) {
                let c = xf.pos(Engine::<T>::handle_position(&selected, corner));
                if (c - pos).length() <= HANDLE {
                    self.drag = Drag::Resize {
                        id: selected.id().clone(),
                        corner,
                        start: world,
                        rect: selected.rect,
                    };
                    self.focus = Focus::Handle(selected.id().clone(), corner);
                    return;
                }
            }
            if T::ID.has_arrows()
                && Engine::<T>::can_connect_from(&selected)
                && (self.connect_handle(&xf, &selected) - pos).length() <= CONNECT_RADIUS + 3.0
            {
                self.drag = Drag::Connect {
                    source: selected.id().clone(),
                    pointer: world,
                };
                return;
            }
        }
        // Arrows.
        let lines = T::arrow_lines(&self.eng, self.eng.layout(), None);
        let hit = lines.iter().rev().find(|l| {
            l.boxes.iter().any(|b| b.contains_point(world))
                || distance_to_segment(pos, xf.pos(l.from), xf.pos(l.to)) <= 6.0
        });
        if let Some(line) = hit {
            let id = line.id.clone();
            self.eng.select_connection(&id);
            self.focus = Focus::Canvas;
            self.drag = Drag::Slide {
                id,
                start: world,
                moved: false,
                offset: None,
            };
            return;
        }
        // Elements.
        if let Some(element) = Self::element_at(self.eng.layout(), world).cloned() {
            let id = element.id().clone();
            self.focus = Focus::Element(id.clone());
            self.selection_before_focus = None;
            if modifiers.shift || modifiers.command || modifiers.ctrl {
                self.eng.toggle_selected(&id, None);
                return;
            }
            let group = self.eng.in_group(&id);
            if !group {
                self.eng.select(&id);
            }
            self.drag = Drag::Move {
                id,
                grab: Point::new(world.x - element.rect.x, world.y - element.rect.y),
                group,
                moved: false,
                placements: Vec::new(),
            };
            return;
        }
        // Empty canvas: clear the selection, and pan.
        self.eng.clear_selection();
        self.focus = Focus::Canvas;
        self.drag = Drag::Pan {
            scroll: self.view.scroll,
            pointer: pos,
        };
    }

    fn drag_to(&mut self, pos: Pos2, world: Point, rect: egui::Rect) {
        match &mut self.drag {
            Drag::None => {}
            Drag::Pan { scroll, pointer } => {
                self.view.scroll = *scroll + (pos - *pointer);
                self.view.clamp(rect);
            }
            Drag::Move {
                id,
                grab,
                group,
                moved,
                placements,
            } => {
                let Some(element) = self.eng.laid(id).cloned() else {
                    return;
                };
                let desired = Point::new(world.x - grab.x, world.y - grab.y);
                if !*moved
                    && (desired.x - element.rect.x).abs() + (desired.y - element.rect.y).abs() < 3.0
                {
                    return;
                }
                *moved = true;
                *placements = if *group {
                    self.eng.group_placements(Point::new(
                        desired.x - element.rect.x,
                        desired.y - element.rect.y,
                    ))
                } else {
                    vec![(id.clone(), self.eng.move_placement(&element, desired))]
                };
            }
            Drag::Resize {
                id,
                corner,
                start,
                rect: r,
            } => {
                if let Some(element) = self.eng.laid(id).cloned() {
                    *r = self.eng.resize_bounds(
                        &element,
                        *corner,
                        Point::new(world.x - start.x, world.y - start.y),
                    );
                }
            }
            Drag::Connect { pointer, .. } => *pointer = world,
            Drag::Slide {
                id,
                start,
                moved,
                offset,
            } => {
                if !*moved && (world.x - start.x).abs() + (world.y - start.y).abs() > 3.0 {
                    *moved = true;
                }
                if *moved {
                    *offset = T::slide_offset(&self.eng, id, world);
                }
            }
        }
    }

    fn release(&mut self, world: Point) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Move {
                id,
                group,
                moved,
                placements,
                ..
            } => {
                if !moved {
                    if group {
                        self.eng.select(&id);
                    }
                } else if group {
                    self.act(None, |eng| eng.commit_group_move(&placements));
                } else if let Some((_, placement)) = placements.into_iter().next() {
                    self.act(None, |eng| eng.commit_move(&id, placement));
                }
            }
            Drag::Resize { id, rect, .. } => self.act(None, |eng| eng.commit_resize(&id, rect)),
            Drag::Connect { source, .. } => {
                if let Some(target) = self.eng.connect_target_at(&source, world) {
                    self.act(None, |eng| {
                        eng.add_connection(&source, &target);
                    });
                }
            }
            Drag::Slide {
                id,
                moved: true,
                offset: Some(offset),
                ..
            } => {
                self.act(None, |eng| T::commit_slide(eng, &id, offset));
            }
            _ => {}
        }
    }

    // Keyboard

    /// Everything Tab moves through: each element, and the handles of the selected one.
    fn focus_order(&self) -> Vec<Focus> {
        let mut order = Vec::new();
        for element in self.eng.layout() {
            order.push(Focus::Element(element.id().clone()));
            if self.eng.selected_id() == Some(element.id()) {
                for corner in self.handles(element) {
                    order.push(Focus::Handle(element.id().clone(), corner));
                }
            }
        }
        order
    }

    fn keyboard_input(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let shift = modifiers.shift;
            let dir = match key {
                Key::ArrowLeft => Some(Dir::Left),
                Key::ArrowRight => Some(Dir::Right),
                Key::ArrowUp => Some(Dir::Up),
                Key::ArrowDown => Some(Dir::Down),
                _ => None,
            };
            if let Some(dir) = dir {
                if self.eng.selected_connection.is_some() {
                    let mut used = false;
                    self.act(None, |eng| used = T::connection_key(eng, dir, shift));
                    if used {
                        continue;
                    }
                }
                match self.focus.clone() {
                    Focus::Element(id) if self.eng.element(&id).is_some() => {
                        if !self.eng.is_selected(&id) {
                            self.eng.select(&id);
                        }
                        self.act(None, |eng| eng.arrow_key(&id, dir, shift));
                    }
                    Focus::Handle(id, corner) if self.eng.element(&id).is_some() => {
                        self.act(None, |eng| eng.resize_key(&id, corner, dir, shift));
                    }
                    _ => {
                        let step = if shift { 200.0 } else { 40.0 };
                        let d = match dir {
                            Dir::Left => Vec2::new(step, 0.0),
                            Dir::Right => Vec2::new(-step, 0.0),
                            Dir::Up => Vec2::new(0.0, step),
                            Dir::Down => Vec2::new(0.0, -step),
                        };
                        self.view.scroll += d;
                        self.view.clamp(rect);
                    }
                }
                continue;
            }
            match key {
                Key::Tab => {
                    let order = self.focus_order();
                    if order.is_empty() {
                        continue;
                    }
                    let current = order.iter().position(|f| *f == self.focus);
                    let next = match (current, shift) {
                        (None, false) => 0,
                        (None, true) => order.len() - 1,
                        (Some(i), false) => (i + 1) % order.len(),
                        (Some(i), true) => (i + order.len() - 1) % order.len(),
                    };
                    self.focus = order[next].clone();
                    // Focusing an element selects just it, unless it's already selected.
                    if let Focus::Element(id) = &self.focus
                        && !self.eng.is_selected(id)
                    {
                        let before = self.eng.selection.clone();
                        self.eng.select(id);
                        self.selection_before_focus = Some(before);
                    }
                    if let Some(element) = self.focused_element() {
                        self.eng.announcement = self.eng.accessible_label(&element);
                        self.reveal(element.rect, rect);
                    }
                }
                Key::Space if shift || modifiers.command || modifiers.ctrl => {
                    if let Focus::Element(id) = self.focus.clone() {
                        let base = self.selection_before_focus.take();
                        self.eng.toggle_selected(&id, base);
                    }
                }
                Key::Enter => {
                    if let Focus::Element(id) = self.focus.clone() {
                        // Selecting it alone shows its properties; its name is focused then.
                        self.eng.select(&id);
                        self.focus_name = true;
                    }
                }
                Key::Escape => {
                    self.eng.clear_selection();
                }
                Key::Delete | Key::Backspace => {
                    self.act(None, |eng| {
                        eng.delete_selection();
                    });
                    if self.eng.element_exists_focus(&self.focus) {
                        continue;
                    }
                    self.focus = Focus::Canvas;
                }
                _ => {}
            }
        }
    }

    fn focused_element(&self) -> Option<Laid<T>> {
        match &self.focus {
            Focus::Element(id) | Focus::Handle(id, _) => self.eng.laid(id).cloned(),
            Focus::Canvas => None,
        }
    }

    /// Scrolls so `r` (world) is in view.
    fn reveal(&mut self, r: Rect, viewport: egui::Rect) {
        let xf = self.view.transform(viewport);
        let screen = xf.rect(r);
        let mut d = Vec2::ZERO;
        if screen.min.x < viewport.min.x {
            d.x = viewport.min.x - screen.min.x + 20.0;
        } else if screen.max.x > viewport.max.x {
            d.x = viewport.max.x - screen.max.x - 20.0;
        }
        if screen.min.y < viewport.min.y {
            d.y = viewport.min.y - screen.min.y + 20.0;
        } else if screen.max.y > viewport.max.y {
            d.y = viewport.max.y - screen.max.y - 20.0;
        }
        self.view.scroll += d;
        self.view.clamp(viewport);
    }

    pub fn has_content(&self) -> bool {
        !self.eng.elements().is_empty()
    }
}

/// Whether the focus target still exists after a removal.
trait FocusExists {
    fn element_exists_focus(&self, focus: &Focus) -> bool;
}

impl<T: DiagramType> FocusExists for Engine<T> {
    fn element_exists_focus(&self, focus: &Focus) -> bool {
        match focus {
            Focus::Element(id) | Focus::Handle(id, _) => self.element(id).is_some(),
            Focus::Canvas => true,
        }
    }
}

/// The editor without its diagram type, as the app holds it.
pub trait DiagramEditor {
    fn ui(&mut self, ui: &mut Ui);
    fn take_changes(&mut self) -> Vec<Change>;
    fn content(&self) -> StoredContent;
    fn replace_content(&mut self, content: &StoredContent) -> Result<(), ContentError>;
    fn set_colors(&mut self, overrides: &BTreeMap<String, String>);
    fn scene(&self) -> Scene;
    fn has_content(&self) -> bool;
    fn show_help(&mut self) -> &mut bool;
    fn announcement(&self) -> &str;
    fn set_announcement(&mut self, text: String);
    /// Selects the first element named `label`, such as for checking the editor.
    fn select_by_label(&mut self, label: &str);
}

impl<T: DiagramView> DiagramEditor for Editor<T> {
    fn ui(&mut self, ui: &mut Ui) {
        Editor::ui(self, ui);
    }

    fn take_changes(&mut self) -> Vec<Change> {
        Editor::take_changes(self)
    }

    fn content(&self) -> StoredContent {
        Editor::content(self)
    }

    fn replace_content(&mut self, content: &StoredContent) -> Result<(), ContentError> {
        Editor::replace_content(self, content)
    }

    fn set_colors(&mut self, overrides: &BTreeMap<String, String>) {
        Editor::set_colors(self, overrides);
    }

    fn scene(&self) -> Scene {
        Editor::scene(self)
    }

    fn has_content(&self) -> bool {
        Editor::has_content(self)
    }

    fn show_help(&mut self) -> &mut bool {
        &mut self.show_help
    }

    fn announcement(&self) -> &str {
        &self.eng.announcement
    }

    fn set_announcement(&mut self, text: String) {
        self.eng.announcement = text;
    }

    fn select_by_label(&mut self, label: &str) {
        if let Some(id) = self
            .eng
            .elements()
            .iter()
            .find(|e| e.label == label)
            .map(|e| e.id.clone())
        {
            self.eng.select(&id);
            self.focus = Focus::Element(id);
        }
    }
}

/// Clears what's typed in a field, such as after a change from elsewhere.
pub fn forget_field(ui: &Ui, id: Id) {
    widgets::reset_field(ui, id);
}
