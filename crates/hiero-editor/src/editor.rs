//! The diagram editor every diagram type shares: a palette, a zoomable canvas where elements
//! are selected, moved, resized, connected, and removed with the pointer or the keyboard, and
//! a properties panel each type fills in. Changes are reported to the app, which saves them
//! and keeps their undo history.

use std::borrow::Cow;
use std::collections::BTreeMap;

use egui::{Align2, Color32, CursorIcon, Id, Key, Pos2, RichText, Sense, Stroke, Ui, Vec2};
use hiero_core::clipboard::Clip;
use hiero_core::engine::{
    Conn, Corner, DiagramType, Dir, El, Engine, Laid, Placement, validate_label,
};
use hiero_core::geometry::{GRID, Point, Rect, Size};
use hiero_core::layout::canvas_rect;
use hiero_core::look::{HOVER_EXTRA_WIDTH, link_mark, link_mark_corner};
use hiero_core::model::{ColorScheme, ConnectionId, DiagramId, ElementId, LineWeight};
use hiero_core::scene::{Node, Scene, first_text_color, outlines};
use hiero_core::stored::{ContentError, StoredContent};

use crate::paint::{PaintCache, Transform, color, paint_nodes, paint_scene, world_line};
use crate::theme::{element_button, page_button, palette, selection_color};
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

    /// The area an element is drawn in, for its selection outline and for picking it with the
    /// pointer. A figure's reaches down to the end of its name, which may go below its area.
    fn drawn_rect(_eng: &Engine<Self>, element: &Laid<Self>) -> Rect {
        element.rect
    }

    /// Where the mark that says an element opens a diagram goes: its top-left corner. It's
    /// usually just inside the element's top-right corner.
    fn link_mark_at(_eng: &Engine<Self>, element: &Laid<Self>) -> Point {
        link_mark_corner(element.rect)
    }

    /// Where the round handle for drawing an arrow from an element is centred, when the type
    /// puts it somewhere of its own, such as where a sequence diagram participant's lifeline
    /// starts. Unset puts it just off the element's right side, half way down.
    fn connect_handle_at(_eng: &Engine<Self>, _element: &Laid<Self>) -> Option<Point> {
        None
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

/// Something about the diagrams elements open that the editor asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagramRequest {
    /// Create a diagram for the element to open, named after it.
    Add { element: ElementId, name: String },
    /// Open a diagram, to view or to edit.
    Open { diagram: DiagramId, viewing: bool },
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
    /// Scrolling the canvas with the right button.
    Pan {
        scroll: Vec2,
        pointer: Pos2,
    },
    /// A selection box drawn from empty canvas, adding to `base` when Shift, Ctrl, or Cmd was
    /// held.
    Select {
        start: Point,
        end: Point,
        base: Vec<ElementId>,
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

/// The instructions while viewing, when nothing can be changed.
const VIEW_HELP: &str = concat!(
    "• You're viewing this diagram: nothing can be changed. Press Edit to change it.\n",
    "• Drag with any mouse button, scroll (with Shift to scroll sideways), or use the arrow keys to move around.\n",
    "• Scroll with Cmd or Ctrl held, pinch on a trackpad, press Cmd or Ctrl and + or -, or use the Zoom slider in the canvas's corner to zoom."
);

/// How far the pointer moves, in canvas pixels, before a press on empty canvas draws a box.
const SELECT_THRESHOLD: f64 = 3.0;

/// Whether the canvas is dark: it follows the app's theme. Exports stay light.
fn dark_canvas(ctx: &egui::Context) -> bool {
    ctx.theme() == egui::Theme::Dark
}

/// The paper's colour, and the colour of the canvas around it.
fn paper_colors(dark: bool) -> (Color32, Color32) {
    if dark {
        let [r, g, b] = hiero_core::dark::PAPER;
        // A little darker than the paper, as the light theme's grey is.
        (Color32::from_rgb(r, g, b), Color32::from_gray(0x0a))
    } else {
        (Color32::WHITE, Color32::from_gray(232))
    }
}
/// The size of handles on screen, whatever the zoom.
const HANDLE: f32 = 12.0;
const CONNECT_RADIUS: f32 = 7.0;

/// How far the pointer may move, in points, for a press and release to count as a click.
const CLICK_SLOP: f32 = 4.0;

/// Whether the canvas shows its grid: one setting for every diagram, kept between runs.
fn grid_id() -> Id {
    Id::unique("hiero-editor.show-grid")
}

fn show_grid(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.get_persisted(grid_id())).unwrap_or(true)
}

/// The diagram editor for diagram type `T`.
pub struct Editor<T: DiagramView> {
    pub eng: Engine<T>,
    view: View,
    drag: Drag,
    focus: Focus,
    selection_before_focus: Option<Vec<ElementId>>,
    cache: PaintCache,
    scene_cache: Option<(u64, Scene)>,
    /// The marks on the elements that open a diagram, for the content they were made from.
    link_marks: Option<(u64, Vec<Node>)>,
    outbox: Vec<Change>,
    pub show_help: bool,
    /// Only looking: no palette or properties, and the canvas only moves and zooms.
    viewing: bool,
    focus_name: bool,
    /// A message for the app to show, such as why the clipboard couldn't be pasted.
    notice: Option<String>,
    /// The names of the diagrams there are, for the elements that open one.
    diagram_names: BTreeMap<DiagramId, String>,
    /// What the app is asked to do about the diagrams elements open.
    request: Option<DiagramRequest>,
    /// The canvas's area on screen last frame, to notice the properties panel taking or giving
    /// back room on the right.
    viewport: Option<egui::Rect>,
    /// How far the canvas was scrolled left to keep the selection clear of the properties
    /// panel, in points, to scroll back when the panel closes.
    panel_shift: f32,
    /// The canvas lost room while a button was held, so the selection is to be shown once
    /// it's let go.
    reveal_due: bool,
    /// Room the canvas got back while a button was held, to scroll back once it's let go.
    room_back: f32,
    salt: Id,
}

/// How far a wheel event, from a mouse wheel or a trackpad, scrolls the view, in points:
/// sideways with Shift. (With Cmd or Ctrl it zooms instead, through egui's `zoom_delta`.)
fn wheel_scroll(
    unit: egui::MouseWheelUnit,
    delta: Vec2,
    shift: bool,
    options: &egui::InputOptions,
    viewport: egui::Rect,
) -> Vec2 {
    use egui::MouseWheelUnit as Unit;
    let points = match unit {
        Unit::Point => delta,
        Unit::Line => delta * options.line_scroll_speed,
        Unit::Page => delta * viewport.height(),
    };
    if shift && points.x == 0.0 {
        // Sideways, where the platform hasn't already turned it.
        Vec2::new(points.y, 0.0)
    } else {
        points
    }
}

/// The zoom after `steps` presses of the zoom keys (negative to zoom out): each moves to the next
/// multiple of twice the slider's step.
fn zoom_by_keys(zoom: f32, steps: i32) -> f32 {
    let step = ZOOM_STEP * 2.0;
    let at = zoom / step;
    // Snap first, so 0.35 zooms in to 0.4 and out to 0.3.
    let from = if steps > 0 {
        (at + 1e-3).floor()
    } else {
        (at - 1e-3).ceil()
    };
    #[allow(clippy::cast_precision_loss, reason = "a handful of key presses")]
    let to = (from + steps as f32) * step;
    to.clamp(ZOOM_MIN, ZOOM_MAX)
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
        let view = View::default().with_canvas(eng.canvas());
        Self {
            eng,
            view,
            drag: Drag::None,
            focus: Focus::Canvas,
            selection_before_focus: None,
            cache: PaintCache::default(),
            scene_cache: None,
            link_marks: None,
            outbox: Vec::new(),
            show_help: false,
            viewing: false,
            focus_name: false,
            notice: None,
            diagram_names: BTreeMap::new(),
            request: None,
            viewport: None,
            panel_shift: 0.0,
            reveal_due: false,
            room_back: 0.0,
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

    pub fn set_line_weight(&mut self, weight: LineWeight) {
        self.eng.set_line_weight(weight);
    }

    /// Sizes the drawing area, which the paper shows. It can't be made smaller than what's on
    /// it needs (see `min_canvas`).
    pub fn set_canvas(&mut self, size: Size) {
        self.eng.set_canvas(size);
        self.view = self.view.with_canvas(self.eng.canvas());
    }

    /// The smallest the drawing area can be: its default size, or bigger to hold everything.
    pub fn min_canvas(&self) -> Size {
        self.eng.min_canvas()
    }

    /// How the canvas is zoomed and scrolled.
    pub fn view(&self) -> View {
        self.view
    }

    pub fn viewing(&self) -> bool {
        self.viewing
    }

    /// Starts or stops viewing. Viewing drops the selection and anything being dragged, so
    /// nothing is left half done.
    pub fn set_viewing(&mut self, viewing: bool) {
        if viewing {
            self.eng.clear_selection();
            self.drag = Drag::None;
            self.focus = Focus::Canvas;
            self.selection_before_focus = None;
        }
        self.viewing = viewing;
    }

    /// The names of the diagrams elements can open, so they can show the one they open and
    /// link to others. A link to a diagram not here is dead: it counts as no link.
    pub fn set_diagram_names(&mut self, names: BTreeMap<DiagramId, String>) {
        self.diagram_names = names;
        self.link_marks = None;
    }

    /// What the app was asked to do since last asked.
    pub fn take_request(&mut self) -> Option<DiagramRequest> {
        self.request.take()
    }

    /// Makes an element open `diagram`, or nothing, as an undoable change. Returns false when
    /// there's no such element.
    pub fn link_diagram(&mut self, element: &ElementId, diagram: Option<DiagramId>) -> bool {
        if self.eng.element(element).is_none() {
            return false;
        }
        let element = element.clone();
        self.act(None, move |eng| {
            eng.replace(&element, |mut e| {
                e.sub_diagram_id = diagram;
                e
            });
        });
        true
    }

    /// The diagram an element opens, when it still exists, with its name.
    fn opens(&self, element: &El<T>) -> Option<(&DiagramId, &str)> {
        let id = element.sub_diagram_id.as_ref()?;
        self.diagram_names
            .get_key_value(id)
            .map(|(id, name)| (id, name.as_str()))
    }

    /// The topmost element under a world point that opens a diagram.
    fn opener_element_at(&self, p: Point) -> Option<&Laid<T>> {
        let layout = self.eng.layout();
        let hit = |e: &&Laid<T>| T::drawn_rect(&self.eng, e).contains_point(p);
        layout
            .iter()
            .rev()
            .filter(|e| !e.is_container)
            .chain(layout.iter().rev().filter(|e| e.is_container))
            .filter(hit)
            .find(|e| self.opens(&e.el).is_some())
    }

    /// The diagram that the topmost element under a world point opens.
    fn opener_at(&self, p: Point) -> Option<(&DiagramId, &str)> {
        self.opens(&self.opener_element_at(p)?.el)
    }

    /// The element under the pointer that opens a diagram, while nothing is being dragged:
    /// the one a click opens while viewing, or selects while editing.
    fn link_hovered(&self, ui: &Ui, response: &egui::Response, xf: Transform) -> Option<&Laid<T>> {
        if !response.hovered() || !matches!(self.drag, Drag::None) {
            return None;
        }
        let p = xf.world(ui.input(|i| i.pointer.hover_pos())?);
        if self.viewing {
            self.opener_element_at(p)
        } else {
            Self::element_at(&self.eng, p).filter(|e| self.opens(&e.el).is_some())
        }
    }

    /// The mark that says an element opens a diagram, in the colour of its name, with thicker
    /// lines when `bold`.
    fn link_mark_for(&self, element: &Laid<T>, bold: bool) -> Node {
        let scene = T::scene(&self.eng, std::slice::from_ref(element), None);
        let color = first_text_color(&scene.nodes, &scene.text_color)
            .unwrap_or_else(|| scene.text_color.clone());
        link_mark(T::link_mark_at(&self.eng, element), &color, bold)
    }

    /// Shows the mark in the top-right corner of each element in `elements` that opens a
    /// diagram. They're made again only when the content changes or while dragging.
    fn paint_link_marks(
        &mut self,
        painter: &egui::Painter,
        scene: &Scene,
        xf: Transform,
        elements: &[Laid<T>],
        dragging: bool,
    ) {
        let changes = self.eng.changes();
        let marks = match &self.link_marks {
            Some((made, marks)) if !dragging && *made == changes => marks.clone(),
            _ => {
                let marks: Vec<Node> = elements
                    .iter()
                    .filter(|e| self.opens(&e.el).is_some())
                    .map(|e| self.link_mark_for(e, false))
                    .collect();
                self.link_marks = (!dragging).then(|| (changes, marks.clone()));
                marks
            }
        };
        let dark = dark_canvas(painter.ctx());
        paint_nodes(painter, scene, &marks, xf, &mut self.cache, dark);
    }

    /// Thickens the lines of the hovered element that opens a diagram, and its mark.
    fn paint_link_hover(
        &mut self,
        painter: &egui::Painter,
        xf: Transform,
        response: &egui::Response,
        ui: &Ui,
    ) {
        let Some(element) = self.link_hovered(ui, response, xf) else {
            return;
        };
        let scene = T::scene(&self.eng, std::slice::from_ref(element), None);
        let mut nodes = outlines(&scene.nodes, HOVER_EXTRA_WIDTH);
        nodes.push(self.link_mark_for(element, true));
        paint_nodes(
            painter,
            &scene,
            &nodes,
            xf,
            &mut self.cache,
            dark_canvas(ui.ctx()),
        );
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
                        ui.label(if self.viewing { VIEW_HELP } else { T::HELP });
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
        // While viewing, the canvas has the whole width.
        if !self.viewing {
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
        }
        // With nothing or several selected, the canvas gets the room.
        let editing = !self.viewing
            && (self.eng.selected_connection.is_some() || self.eng.selected().is_some());
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
            self.follow_viewport(rect);
            self.canvas_ui(ui, rect);
            // A button let go this frame leaves a scroll waiting for the next one.
            if self.reveal_due || self.room_back > 0.0 {
                ui.ctx().request_repaint();
            }
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

    /// Keeps the selection in view when the canvas loses room on the right, as when the
    /// properties panel opens over an element near the right edge, by scrolling the canvas left;
    /// and scrolls back right by as much when the room comes back, as when the panel closes.
    ///
    /// It waits until no button is held: the panel opens as an element is pressed, and
    /// scrolling under the pointer then would turn the press into a move, dragging the element
    /// back under the panel.
    fn follow_viewport(&mut self, rect: egui::Rect) {
        if let Some(before) = self.viewport.replace(rect) {
            let change = rect.max.x - before.max.x;
            if change < -0.5 {
                self.reveal_due = true;
            } else if change > 0.5 {
                // Keeping the paper in reach may already move the canvas right into the new
                // room, at once; that counts toward scrolling back.
                let before = self.view.scroll.x;
                self.view.clamp(rect);
                let pushed = (self.view.scroll.x - before).max(0.0);
                self.panel_shift = (self.panel_shift - pushed).max(0.0);
                self.room_back += (change - pushed).max(0.0);
            }
        }
        if !matches!(self.drag, Drag::None) {
            return;
        }
        if self.room_back > 0.0 {
            let back = self.panel_shift.min(self.room_back);
            self.view.scroll.x += back;
            self.panel_shift -= back;
            self.room_back = 0.0;
            self.view.clamp(rect);
        }
        if std::mem::take(&mut self.reveal_due)
            && let Some((left, right)) = self.selection_span()
        {
            let moved = self.view.reveal_x(left, right, rect, 20.0);
            self.panel_shift = (self.panel_shift - moved).max(0.0);
        }
    }

    /// Where the selected element or arrow reaches across the canvas, in world pixels.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "canvas pixels are at most 12000"
    )]
    fn selection_span(&self) -> Option<(f32, f32)> {
        if let Some(element) = self.eng.selected() {
            let r = T::drawn_rect(&self.eng, element);
            return Some((r.x as f32, r.right() as f32));
        }
        let id = self.eng.selected_connection.as_ref()?;
        let line = T::arrow_lines(&self.eng, self.eng.layout(), None)
            .into_iter()
            .find(|l| &l.id == id)?;
        let xs = [line.from.x, line.to.x]
            .into_iter()
            .chain(line.boxes.iter().flat_map(|b| [b.x, b.right()]));
        let (left, right) = xs.fold((f64::MAX, f64::MIN), |(l, r), x| (l.min(x), r.max(x)));
        Some((left as f32, right as f32))
    }

    fn palette_ui(&mut self, ui: &mut Ui) {
        heading(ui, "Elements");
        ui.label(
            RichText::new("Drag onto the canvas, or click to add.")
                .weak()
                .small(),
        );
        for (index, kind) in T::palette().iter().enumerate() {
            if let Some(text) = kind.palette_heading {
                ui.add_space(6.0);
                heading(ui, text);
            }
            let id = self.salt.with(("palette", index));
            let response = ui
                .dnd_drag_source(id, PaletteDrag(index), |ui| {
                    element_button(ui, kind.entry_label(), 28.0)
                })
                .inner;
            if response.clicked() {
                self.act(None, |eng| eng.add_entry_at_free_position(kind));
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
        let mut request = None;
        let mut link: Option<(ElementId, Option<DiagramId>)> = None;
        if let Some(id) = cx.eng.selected_connection.clone()
            && let Some(connection) = cx.eng.connection(&id).cloned()
        {
            let title = cx
                .eng
                .arrow_name(&connection.source_id, &connection.target_id);
            ui.label(RichText::new(title).strong());
            T::arrow_properties(ui, &mut cx, &connection);
            ui.add_space(6.0);
            if page_button(ui, &format!("Remove {}", T::ID.arrow_noun().one), false).clicked() {
                remove_arrow = Some(id);
            }
        } else if let Some(element) = cx.eng.selected().cloned() {
            ui.label(RichText::new(element.info.label).weak());
            T::element_properties(ui, &mut cx, &element);
            focus_name_id = Some(cx.field_id(element.id(), "label").with("edit"));
            ui.add_space(5.0);
            heading(ui, "Diagram");
            // A link to a diagram that's gone counts as none.
            match self.opens(&element.el) {
                Some((id, name)) => {
                    ui.label(RichText::new(name).strong());
                    ui.horizontal_wrapped(|ui| {
                        if page_button(ui, "Edit diagram", false)
                            .on_hover_text("Open it to change it")
                            .clicked()
                        {
                            request = Some(DiagramRequest::Open {
                                diagram: id.clone(),
                                viewing: false,
                            });
                        }
                        if page_button(ui, "Remove link", false)
                            .on_hover_text("Stop this element opening it. The diagram is kept.")
                            .clicked()
                        {
                            link = Some((element.id().clone(), None));
                        }
                    });
                }
                None => {
                    if page_button(ui, "Add diagram", false)
                        .on_hover_text("Create a sub diagram that opens from this element")
                        .clicked()
                    {
                        request = Some(DiagramRequest::Add {
                            element: element.id().clone(),
                            name: element.el.label.clone(),
                        });
                    }
                    let mut options: Vec<(DiagramId, String)> = self
                        .diagram_names
                        .iter()
                        .map(|(id, name)| (id.clone(), name.clone()))
                        .collect();
                    options.sort_by_cached_key(|(id, name)| (name.to_lowercase(), id.clone()));
                    if !options.is_empty() {
                        ui.add_space(4.0);
                        if let Some(Some(chosen)) = select(
                            ui,
                            cx.field_id(element.id(), "link-diagram"),
                            "Link to a diagram",
                            &None,
                            Some("Select a diagram"),
                            &options,
                        ) {
                            link = Some((element.id().clone(), Some(chosen)));
                        }
                    }
                }
            }
            if T::ID.has_arrows() {
                let arrows = cx.eng.arrows_of(element.id());
                let can_draw = Engine::<T>::can_connect_from(&element);
                if !arrows.is_empty() || can_draw {
                    ui.add_space(5.0);
                    heading(ui, &T::ID.arrow_noun().many_capitalized());
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
                            &format!("Add {} to", T::ID.arrow_noun().one),
                            &target,
                            Some("Select a target"),
                            &options,
                        ) {
                            target = choice;
                        }
                        ui.data_mut(|d| d.insert_temp(target_id, target.clone()));
                        let add = ui.add_enabled_ui(target.is_some(), |ui| {
                            page_button(ui, &format!("Add {}", T::ID.arrow_noun().one), false)
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
        if request.is_some() {
            self.request = request;
        }
        if let Some((element, diagram)) = link {
            self.link_diagram(&element, diagram);
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
    fn element_at(eng: &Engine<T>, p: Point) -> Option<&Laid<T>> {
        let layout = eng.layout();
        let hit = |e: &&Laid<T>| T::drawn_rect(eng, e).contains_point(p);
        layout
            .iter()
            .rev()
            .find(|e| !e.is_container && hit(e))
            .or_else(|| layout.iter().rev().find(hit))
    }

    fn connect_handle(&self, xf: &Transform, element: &Laid<T>) -> Pos2 {
        if let Some(at) = T::connect_handle_at(&self.eng, element) {
            return xf.pos(at);
        }
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
            _ if self.viewing => format!(
                "Diagram, {} elements, view only. Use the arrow keys to move around.",
                self.eng.elements().len()
            ),
            Some(l) => format!("Diagram canvas: {l}"),
            None => format!(
                "Diagram canvas, {} elements. Press Tab to move through them.",
                self.eng.elements().len()
            ),
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &label));
        self.view.clamp(rect);
        let painter = ui.painter_at(rect);
        let dark = dark_canvas(ui.ctx());
        let (paper_color, around) = paper_colors(dark);
        painter.rect_filled(rect, 0.0, around);
        let xf = self.view.transform(rect);
        let paper = xf.rect(canvas_rect(self.eng.canvas()));
        painter.rect_filled(paper, 0.0, paper_color);
        // Viewing never shows the grid.
        if !self.viewing && show_grid(ui.ctx()) {
            self.paint_grid(&painter, xf, rect);
        }

        // Input that changes the view. Cmd or Ctrl with the wheel, and pinching, zoom through
        // egui's `zoom_delta`. Other wheel events, from a mouse wheel or a trackpad, scroll as
        // they come, sideways with Shift.
        let options = ui.ctx().options(|o| o.input_options);
        let (pinch, wheel, hover) = ui.input(|i| {
            let mut wheel = Vec2::ZERO;
            for event in &i.events {
                if let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = event
                    && !modifiers.matches_any(options.zoom_modifier)
                {
                    wheel += wheel_scroll(*unit, *delta, modifiers.shift, &options, rect);
                }
            }
            (i.zoom_delta(), wheel, i.pointer.hover_pos())
        });
        if response.hovered() {
            if pinch != 1.0
                && let Some(p) = hover
            {
                self.view.zoom_around(self.view.zoom * pinch, p, rect);
            }
            if wheel != Vec2::ZERO {
                self.view.scroll += wheel;
                self.view.clamp(rect);
            }
        }
        self.zoom_keys(ui, rect);
        let xf = self.view.transform(rect);

        if self.viewing {
            self.view_pointer_input(ui, &response, rect);
        } else {
            self.pointer_input(ui, &response, xf, rect);
        }
        // Text fields cut, copy, and paste their own text.
        if !self.viewing && (response.has_focus() || ui.memory(|m| m.focused().is_none())) {
            self.clipboard_input(ui);
        }
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
        let dragging = !matches!(
            self.drag,
            Drag::None | Drag::Pan { .. } | Drag::Select { .. }
        );
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
        paint_scene(&painter, &scene, xf, &mut self.cache, 1.0, dark);
        self.paint_link_marks(&painter, &scene, xf, &displayed, dragging);
        self.paint_link_hover(&painter, xf, &response, ui);
        if !self.viewing {
            self.paint_overlays(&painter, xf, &displayed, &response, ui);
        }
        self.zoom_control(ui, rect);
    }

    fn paint_grid(&self, painter: &egui::Painter, xf: Transform, clip: egui::Rect) {
        let step = GRID as f32 * xf.zoom;
        if step < 6.0 {
            return;
        }
        // The light grid's grey, as the dark canvas shows it.
        let gray = if dark_canvas(painter.ctx()) {
            hiero_core::dark::map([200; 3], hiero_core::scene::Tone::Element)
        } else {
            [200; 3]
        };
        let dot = Color32::from_rgb(gray[0], gray[1], gray[2]);
        let paper = xf.rect(canvas_rect(self.eng.canvas())).intersect(clip);
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
        let selection = selection_color(ui.ctx());
        // Handles are drawn as the prototype's are: in the theme's accent, filled with its
        // surface, so they're light lines on dark in the dark theme and dark on light in the
        // light one.
        let theme = palette(ui);
        let (handle_line, handle_fill) = (theme.accent, theme.surface);
        // Selected arrow.
        let lines = T::arrow_lines(&self.eng, layout, self.slide());
        if let Some(selected) = &self.eng.selected_connection
            && let Some(line) = lines.iter().find(|l| l.id == *selected)
        {
            painter.line_segment(
                [xf.pos(line.from), xf.pos(line.to)],
                Stroke::new(6.0, selection.gamma_multiply(0.45)),
            );
            for b in &line.boxes {
                painter.rect_stroke(
                    xf.rect(*b).expand(2.0),
                    3.0,
                    Stroke::new(2.0, selection),
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
            let r = xf.rect(T::drawn_rect(&self.eng, element)).expand(3.0);
            if selected {
                painter.rect_stroke(
                    r,
                    4.0,
                    Stroke::new(2.0, selection),
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
                painter.rect_filled(r, 2.0, handle_fill);
                painter.rect_stroke(
                    r,
                    2.0,
                    Stroke::new(if focused_handle { 3.0 } else { 2.0 }, handle_line),
                    egui::StrokeKind::Inside,
                );
            }
            if T::ID.has_arrows() && Engine::<T>::can_connect_from(element) {
                let c = self.connect_handle(&xf, element);
                // A circle with a plus in it: a 6px bar across and one down, 2px thick, so it
                // reads as a handle to make a new arrow from.
                painter.circle(
                    c,
                    CONNECT_RADIUS,
                    handle_fill,
                    Stroke::new(2.0, handle_line),
                );
                painter.line_segment(
                    [c - Vec2::new(3.0, 0.0), c + Vec2::new(3.0, 0.0)],
                    Stroke::new(2.0, handle_line),
                );
                painter.line_segment(
                    [c - Vec2::new(0.0, 3.0), c + Vec2::new(0.0, 3.0)],
                    Stroke::new(2.0, handle_line),
                );
            }
        }
        if let Drag::Connect { source, pointer } = &self.drag
            && let Some(element) = layout.iter().find(|l| l.id() == source)
        {
            let from = T::connect_handle_at(&self.eng, element)
                .unwrap_or_else(|| T::arrow_rect(&self.eng, element).center());
            world_line(
                painter,
                xf,
                &[from, *pointer],
                Stroke::new(2.0, selection),
                true,
            );
            if let Some(target) = self.eng.connect_target_at(source, *pointer)
                && let Some(t) = layout.iter().find(|l| *l.id() == target)
            {
                painter.rect_stroke(
                    xf.rect(t.rect).expand(3.0),
                    4.0,
                    Stroke::new(2.0, selection),
                    egui::StrokeKind::Outside,
                );
            }
        }
        // The selection box.
        if let Some(area) = self.select_area() {
            let r = xf.rect(area);
            painter.rect_filled(r, 0.0, selection.gamma_multiply(0.1));
            let corners = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            painter.extend(egui::Shape::dashed_line(
                &corners,
                Stroke::new(1.0, selection),
                4.0,
                3.0,
            ));
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
                Stroke::new(1.5, selection),
                egui::StrokeKind::Middle,
            );
            painter.text(
                xf.pos(ghost.center()),
                Align2::CENTER_CENTER,
                kind.entry_label(),
                egui::FontId::proportional(13.0 * xf.zoom.max(0.6)),
                selection,
            );
        }
        let _ = color;
    }

    fn zoom_control(&mut self, ui: &mut Ui, rect: egui::Rect) {
        // Narrower while viewing, which has no grid checkbox, so it still ends at the right.
        let width = if self.viewing { 225.0 } else { 320.0 };
        let area = egui::Rect::from_min_size(
            rect.right_bottom() - Vec2::new(width + 10.0, 40.0),
            Vec2::new(width, 32.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Zoom");
                    let mut percent = (self.view.zoom * 100.0).round();
                    let slider =
                        egui::Slider::new(&mut percent, (ZOOM_MIN * 100.0)..=(ZOOM_MAX * 100.0))
                            .step_by(f64::from(ZOOM_STEP * 100.0))
                            // Clamping would round the zoom to a step every frame, undoing
                            // smaller changes, such as a slow turn of the wheel.
                            // `zoom_around` keeps it in range.
                            .clamping(egui::SliderClamping::Never)
                            .suffix("%");
                    if ui.add(slider).changed() {
                        self.view.zoom_around(percent / 100.0, rect.center(), rect);
                    }
                    if !self.viewing {
                        ui.separator();
                        let mut grid = show_grid(ui.ctx());
                        if ui.checkbox(&mut grid, "Grid").changed() {
                            ui.data_mut(|d| d.insert_persisted(grid_id(), grid));
                        }
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
            let world = xf.world(p);
            self.act(None, |eng| eng.palette_drop_entry(kind, world));
            response.request_focus();
            self.focus = self
                .eng
                .selected_id()
                .cloned()
                .map_or(Focus::Canvas, Focus::Element);
            return;
        }
        let (pressed, pan_pressed, down, released, pos, modifiers) = ui.input(|i| {
            let p = &i.pointer;
            (
                p.primary_pressed(),
                p.secondary_pressed() || p.button_pressed(egui::PointerButton::Middle),
                p.primary_down() || p.secondary_down() || p.middle_down(),
                p.primary_released()
                    || p.secondary_released()
                    || p.button_released(egui::PointerButton::Middle),
                p.interact_pos(),
                i.modifiers,
            )
        });
        let Some(pos) = pos else {
            return;
        };
        let world = xf.world(pos);
        if pan_pressed && response.hovered() {
            // The right and middle buttons scroll from anywhere, over elements and arrows too.
            response.request_focus();
            self.drag = Drag::Pan {
                scroll: self.view.scroll,
                pointer: pos,
            };
        } else if pressed && response.hovered() {
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
                Drag::Select { .. } | Drag::Connect { .. } => CursorIcon::Crosshair,
                Drag::Slide { .. } => CursorIcon::ResizeVertical,
                Drag::None => {
                    if Self::element_at(&self.eng, world).is_some() {
                        CursorIcon::PointingHand
                    } else {
                        CursorIcon::Default
                    }
                }
            };
            ui.ctx().set_cursor_icon(cursor);
        }
    }

    /// While viewing: any button drags the canvas around, and clicking an element that opens a
    /// diagram opens it.
    fn view_pointer_input(&mut self, ui: &mut Ui, response: &egui::Response, rect: egui::Rect) {
        let (pressed, down, released, pos) = ui.input(|i| {
            let p = &i.pointer;
            (
                p.primary_pressed()
                    || p.secondary_pressed()
                    || p.button_pressed(egui::PointerButton::Middle),
                p.primary_down() || p.secondary_down() || p.middle_down(),
                p.primary_released()
                    || p.secondary_released()
                    || p.button_released(egui::PointerButton::Middle),
                p.interact_pos(),
            )
        });
        let Some(pos) = pos else {
            return;
        };
        if pressed && response.hovered() {
            response.request_focus();
            self.drag = Drag::Pan {
                scroll: self.view.scroll,
                pointer: pos,
            };
        } else if down && let Drag::Pan { scroll, pointer } = self.drag {
            self.view.scroll = scroll + (pos - pointer);
            self.view.clamp(rect);
        }
        let xf = self.view.transform(rect);
        // A click that didn't move the canvas opens the diagram of the element under it.
        if released
            && response.hovered()
            && let Drag::Pan { pointer, .. } = self.drag
            && (pos - pointer).length() < CLICK_SLOP
            && let Some((id, _)) = self.opener_at(xf.world(pos))
        {
            self.request = Some(DiagramRequest::Open {
                diagram: id.clone(),
                viewing: true,
            });
        }
        if released {
            self.drag = Drag::None;
        }
        if response.hovered() {
            let opener = match self.drag {
                Drag::Pan { .. } => None,
                _ => self
                    .opener_at(xf.world(pos))
                    .map(|(_, name)| name.to_owned()),
            };
            ui.ctx().set_cursor_icon(match (&self.drag, &opener) {
                (Drag::Pan { .. }, _) => CursorIcon::Grabbing,
                (_, Some(_)) => CursorIcon::PointingHand,
                _ => CursorIcon::Grab,
            });
            if let Some(name) = opener {
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    self.salt.with("opens"),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| ui.label(format!("Open “{name}”")));
            }
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
        if let Some(element) = Self::element_at(&self.eng, world).cloned() {
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
        // Empty canvas: clear the selection, unless Shift, Ctrl, or Cmd is held, and draw a
        // selection box.
        let base = if modifiers.shift || modifiers.command || modifiers.ctrl {
            self.eng.selection.clone()
        } else {
            self.eng.clear_selection();
            Vec::new()
        };
        self.focus = Focus::Canvas;
        self.selection_before_focus = None;
        self.drag = Drag::Select {
            start: world,
            end: world,
            base,
        };
    }

    /// The selection box being drawn, once the pointer has moved far enough to draw one.
    fn select_area(&self) -> Option<Rect> {
        let Drag::Select { start, end, .. } = &self.drag else {
            return None;
        };
        let area = Rect::new(
            start.x.min(end.x),
            start.y.min(end.y),
            (end.x - start.x).abs(),
            (end.y - start.y).abs(),
        );
        (area.width.max(area.height) >= SELECT_THRESHOLD).then_some(area)
    }

    /// Selects the elements wholly inside the box, after those already selected in `base`.
    fn select_inside(&mut self, area: Rect, base: Vec<ElementId>) {
        let inside: Vec<ElementId> = self
            .eng
            .layout()
            .iter()
            .filter(|e| area.contains_rect(&T::drawn_rect(&self.eng, e)))
            .map(|e| e.id().clone())
            .filter(|id| !base.contains(id))
            .collect();
        self.eng.clear_selection();
        self.eng.selection = base.into_iter().chain(inside).collect();
        let count = self.eng.selection.len();
        self.eng.announcement = format!(
            "{count} {} selected.",
            if count == 1 { "element" } else { "elements" }
        );
    }

    fn drag_to(&mut self, pos: Pos2, world: Point, rect: egui::Rect) {
        match &mut self.drag {
            Drag::None => {}
            Drag::Pan { scroll, pointer } => {
                self.view.scroll = *scroll + (pos - *pointer);
                self.view.clamp(rect);
            }
            Drag::Select { end, .. } => *end = world,
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
        let area = self.select_area();
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Select { base, .. } => {
                if let Some(area) = area {
                    self.select_inside(area, base);
                }
            }
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
                    let mut added = None;
                    self.act(None, |eng| added = eng.add_connection(&source, &target));
                    // Select the new arrow, or the one already there, so its properties can be
                    // edited without a second click.
                    let drawn = added.or_else(|| {
                        self.eng
                            .connections()
                            .iter()
                            .find(|c| c.source_id == source && c.target_id == target)
                            .map(|c| c.id.clone())
                    });
                    if let Some(id) = drawn {
                        self.eng.select_connection(&id);
                    }
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
                if self.viewing {
                    self.scroll_key(dir, shift, rect);
                    continue;
                }
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
                    _ => self.scroll_key(dir, shift, rect),
                }
                continue;
            }
            if self.viewing {
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

    /// Cmd or Ctrl and + (or =) zooms in a step, and Cmd or Ctrl and - zooms out, around the
    /// middle of the canvas. The keys are taken, so egui doesn't scale the whole window too.
    fn zoom_keys(&mut self, ui: &Ui, rect: egui::Rect) {
        let mut steps = 0i32;
        ui.input_mut(|i| {
            for modifiers in [egui::Modifiers::COMMAND, egui::Modifiers::CTRL] {
                for key in [Key::Plus, Key::Equals] {
                    while i.consume_key(modifiers, key) {
                        steps += 1;
                    }
                }
                while i.consume_key(modifiers, Key::Minus) {
                    steps -= 1;
                }
            }
        });
        if steps != 0 {
            self.view
                .zoom_around(zoom_by_keys(self.view.zoom, steps), rect.center(), rect);
            self.view.clamp(rect);
        }
    }

    /// Moves the canvas for an arrow key, further with Shift.
    fn scroll_key(&mut self, dir: Dir, shift: bool, rect: egui::Rect) {
        let step = if shift { 200.0 } else { 40.0 };
        self.view.scroll += match dir {
            Dir::Left => Vec2::new(step, 0.0),
            Dir::Right => Vec2::new(-step, 0.0),
            Dir::Up => Vec2::new(0.0, step),
            Dir::Down => Vec2::new(0.0, -step),
        };
        self.view.clamp(rect);
    }

    /// Cut, copy, and paste (Cmd or Ctrl and X, C, and V): the selected elements, with
    /// everything they hold and the arrows between them, as text on the system clipboard.
    fn clipboard_input(&mut self, ui: &Ui) {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Copy => match self.eng.copy_selection() {
                    Some(clip) => ui.ctx().copy_text(clip.to_text()),
                    None => self.eng.announcement = "Select elements to copy them.".into(),
                },
                egui::Event::Cut => {
                    let mut clip = None;
                    self.act(None, |eng| clip = eng.cut_selection());
                    match clip {
                        Some(clip) => ui.ctx().copy_text(clip.to_text()),
                        None => self.eng.announcement = "Select elements to cut them.".into(),
                    }
                    if !self.eng.element_exists_focus(&self.focus) {
                        self.focus = Focus::Canvas;
                    }
                }
                egui::Event::Paste(text) => match Clip::<T>::from_text(&text) {
                    Ok(clip) => {
                        self.act(None, |eng| eng.paste(&clip));
                        if let Some(first) = self.eng.selection.first() {
                            self.focus = Focus::Element(first.clone());
                        }
                    }
                    Err(message) => {
                        self.eng.announcement.clone_from(&message);
                        self.notice = Some(message);
                    }
                },
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
    fn set_line_weight(&mut self, weight: LineWeight);
    fn set_canvas(&mut self, size: Size);
    fn min_canvas(&self) -> Size;
    fn scene(&self) -> Scene;
    fn has_content(&self) -> bool;
    fn show_help(&mut self) -> &mut bool;
    /// Whether it's only being looked at, with nothing to change.
    fn viewing(&self) -> bool;
    fn set_viewing(&mut self, viewing: bool);
    fn announcement(&self) -> &str;
    fn set_announcement(&mut self, text: String);
    /// A message to show, such as why the clipboard couldn't be pasted. Taken once.
    fn take_notice(&mut self) -> Option<String>;
    fn set_diagram_names(&mut self, names: BTreeMap<DiagramId, String>);
    fn take_request(&mut self) -> Option<DiagramRequest>;
    fn link_diagram(&mut self, element: &ElementId, diagram: Option<DiagramId>) -> bool;
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

    fn set_line_weight(&mut self, weight: LineWeight) {
        Editor::set_line_weight(self, weight);
    }

    fn set_canvas(&mut self, size: Size) {
        Editor::set_canvas(self, size);
    }

    fn min_canvas(&self) -> Size {
        Editor::min_canvas(self)
    }

    fn scene(&self) -> Scene {
        Editor::scene(self)
    }

    fn has_content(&self) -> bool {
        Editor::has_content(self)
    }

    fn viewing(&self) -> bool {
        Editor::viewing(self)
    }

    fn set_viewing(&mut self, viewing: bool) {
        Editor::set_viewing(self, viewing);
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

    fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    fn set_diagram_names(&mut self, names: BTreeMap<DiagramId, String>) {
        Editor::set_diagram_names(self, names);
    }

    fn take_request(&mut self) -> Option<DiagramRequest> {
        Editor::take_request(self)
    }

    fn link_diagram(&mut self, element: &ElementId, diagram: Option<DiagramId>) -> bool {
        Editor::link_diagram(self, element, diagram)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_scrolls_sideways_with_shift() {
        let viewport = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let options = egui::InputOptions::default();
        let scroll =
            |unit, shift| wheel_scroll(unit, Vec2::new(0.0, -2.0), shift, &options, viewport);
        assert_eq!(
            scroll(egui::MouseWheelUnit::Point, false),
            Vec2::new(0.0, -2.0)
        );
        assert_eq!(
            scroll(egui::MouseWheelUnit::Point, true),
            Vec2::new(-2.0, 0.0)
        );
        assert_eq!(
            scroll(egui::MouseWheelUnit::Line, false),
            Vec2::new(0.0, -2.0 * options.line_scroll_speed)
        );
    }

    #[test]
    fn zoom_keys_step_by_ten_percent() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(near(zoom_by_keys(0.5, 1), 0.6));
        assert!(near(zoom_by_keys(0.5, -1), 0.4));
        assert!(near(zoom_by_keys(0.35, 1), 0.4));
        assert!(near(zoom_by_keys(0.35, -1), 0.3));
        assert!(near(zoom_by_keys(1.0, 1), ZOOM_MAX));
        assert!(near(zoom_by_keys(0.1, -1), ZOOM_MIN));
    }
}
