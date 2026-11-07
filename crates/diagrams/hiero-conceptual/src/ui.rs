//! The conceptual diagram's properties panel and its place in the shared editor.

use egui::Ui;
use hiero_core::engine::{Conn, DiagramType, Dir, Engine, Laid, validate_length};
use hiero_core::geometry::{Point, Rect};
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::theme::page_button;
use hiero_editor::widgets::{self, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::arrows::{self, offset_at, place_automatically, set_manual_offset, straighten};
use crate::model::{
    ARROW_LABEL_MAX, ArrowMode, CONNECTOR_MAX, ComponentType, Conceptual, ConceptualArrow,
    DESCRIPTION_MAX, TRANSFORMATION_MAX,
};
use crate::rules::{arrow_title, normalized_arrow};
use crate::scene::{drawn_arrows, drawn_rect, scene_of};

impl DiagramView for Conceptual {
    const HELP: &'static str = crate::HELP;

    fn scene(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        slide: Option<(&ConnectionId, f64)>,
    ) -> Scene {
        scene_of(eng, layout, slide)
    }

    fn arrow_lines(
        eng: &Engine<Self>,
        layout: &[Laid<Self>],
        slide: Option<(&ConnectionId, f64)>,
    ) -> Vec<ArrowLine> {
        drawn_arrows(eng, layout, slide)
            .into_iter()
            .map(|a| ArrowLine {
                id: a.id,
                from: a.look.from,
                to: a.look.to,
                boxes: a
                    .look
                    .connector
                    .iter()
                    .chain(a.look.transformation.iter())
                    .map(|(_, r)| *r)
                    .collect(),
            })
            .collect()
    }

    fn drawn_rect(eng: &Engine<Self>, element: &Laid<Self>) -> Rect {
        drawn_rect(eng, element)
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let kind = element.el.kind;
        let mut field = None;
        if cx.name_field(ui, element, &mut with) {
            field = Some("label");
        }
        if element.info.has_description
            && let Some(description) = text_field(
                ui,
                cx.field_id(element.id(), "description"),
                "Description",
                element.el.description.as_deref().unwrap_or_default(),
                true,
                |t| validate_length("Description", t, DESCRIPTION_MAX),
            )
        {
            with.description = Some(description).filter(|d| !d.trim().is_empty());
            field = Some("description");
        }
        // Bot and Group, side by side.
        if kind.can_be_agent() || kind.can_be_group() {
            ui.add_space(widgets::FIELD_GAP);
            ui.horizontal_wrapped(|ui| {
                if kind.can_be_agent() && ui.checkbox(&mut with.props.agent, "Bot").changed() {
                    field = Some("agent");
                }
                if kind.can_be_group() {
                    ui.add_space(widgets::FIELD_GAP);
                    if ui.checkbox(&mut with.props.group, "Group").changed() {
                        field = Some("group");
                    }
                }
            });
        }
        if kind.can_be_cloud_hosted()
            && widgets::checkbox(ui, &mut with.props.cloud_hosted, "Cloud hosted").changed()
        {
            field = Some("cloud");
        }
        if kind.has_component_type() {
            widgets::label(ui, "Type", false);
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [
                    (None, "Default"),
                    (Some(ComponentType::Database), "Database"),
                    (Some(ComponentType::Storage), "Storage"),
                ] {
                    if ui
                        .radio_value(&mut with.props.component_type, value, label)
                        .changed()
                    {
                        field = Some("component-type");
                    }
                }
            });
        }
        let parent = cx.parent_field(ui, element, false);
        if parent.is_some() {
            field = Some("parent");
        }
        if let Some(field) = field {
            cx.commit(element, with, parent, field);
        }
    }

    fn arrow_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, connection: &Conn<Self>) {
        let id = connection.id.clone();
        let current = connection.props.clone();
        let mut arrow = current.clone();
        let mut field: Option<&'static str> = None;
        let text =
            |ui: &mut Ui, name: &'static str, label: &str, value: &Option<String>, max: usize| {
                text_field(
                    ui,
                    cx.arrow_field_id(&id, name),
                    label,
                    value.as_deref().unwrap_or_default(),
                    false,
                    |t| validate_length(label, t, max),
                )
            };
        if let Some(v) = text(ui, "label", "Text", &current.label, ARROW_LABEL_MAX) {
            arrow.label = Some(v);
            field = Some("label");
        }
        widgets::label(ui, "Type", false);
        let mut manual = current.is_manual();
        ui.horizontal(|ui| {
            if ui.radio_value(&mut manual, false, "Automated").changed()
                || ui.radio_value(&mut manual, true, "Manual").changed()
            {
                field = Some("mode");
            }
        });
        arrow.mode = manual.then_some(ArrowMode::Manual);
        if widgets::checkbox(ui, &mut arrow.bidirectional, "Bi-directional").changed() {
            field = Some("bidirectional");
        }
        if let Some(v) = text(
            ui,
            "connector",
            "Connector",
            &current.connector,
            CONNECTOR_MAX,
        ) {
            arrow.connector = Some(v);
            field = Some("connector");
        }
        if let Some(v) = text(
            ui,
            "transformation",
            "Transformation",
            &current.transformation,
            TRANSFORMATION_MAX,
        ) {
            arrow.transformation = Some(v);
            field = Some("transformation");
        }
        if let Some(field) = field {
            let arrow = normalized_arrow(arrow);
            if Conceptual::validate_arrow(&arrow).is_ok() && arrow != current {
                let mode_changed = arrow.is_manual() != current.is_manual();
                let direction_changed = arrow.bidirectional != current.bidirectional;
                let (source, target) = (connection.source_id.clone(), connection.target_id.clone());
                let target_id = id.clone();
                cx.apply(Some(format!("{id}:{field}")), move |eng| {
                    eng.set_connection_props(&target_id, arrow.clone());
                    let name = eng.arrow_name(&source, &target);
                    if mode_changed {
                        let mode = if arrow.is_manual() {
                            "manual"
                        } else {
                            "automated"
                        };
                        eng.announcement = format!("{name} set to {mode}.");
                    } else if direction_changed {
                        let way = if arrow.bidirectional {
                            "bi-directional"
                        } else {
                            "one way"
                        };
                        eng.announcement = format!("{name} set to {way}.");
                    }
                });
            }
        }
        ui.add_space(6.0);
        let straightenable = arrows::range_for(cx.eng, &id).is_some();
        ui.horizontal(|ui| {
            if straightenable && page_button(ui, "Straighten", false).clicked() {
                let id = id.clone();
                cx.apply(None, move |eng| straighten(eng, &id));
            }
            if current.manual_offset.is_some()
                && page_button(ui, "Place automatically", false).clicked()
            {
                let id = id.clone();
                cx.apply(None, move |eng| place_automatically(eng, &id));
            }
        });
    }

    fn arrow_title(
        _eng: &Engine<Self>,
        connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        arrow_title(&connection.props, outgoing, other)
    }

    fn slide_offset(eng: &Engine<Self>, id: &ConnectionId, pointer: Point) -> Option<f64> {
        offset_at(eng, id, pointer)
    }

    fn commit_slide(eng: &mut Engine<Self>, id: &ConnectionId, offset: f64) {
        set_manual_offset(eng, id, Some(offset), "moved");
    }

    fn connection_key(eng: &mut Engine<Self>, dir: Dir, shift: bool) -> bool {
        arrows::arrow_key(eng, dir, shift)
    }
}

/// Keeps the arrow type in scope for callers that build arrows by hand.
pub type Arrow = ConceptualArrow;

#[cfg(test)]
mod tests {
    use egui::{Event, Id, Modifiers, PointerButton, Pos2, RawInput, Vec2};
    use hiero_core::engine::Engine;
    use hiero_core::geometry::Point;
    use hiero_core::look::DiagramColors;
    use hiero_core::model::ElementId;
    use hiero_core::text::EstimateText;
    use hiero_editor::Editor;
    use hiero_editor::view::View;

    use crate::Conceptual;

    static ESTIMATE: EstimateText = EstimateText;

    /// An editor in a window, driven by pointer events a frame at a time.
    struct Window {
        ctx: egui::Context,
        editor: Editor<Conceptual>,
        salt: Id,
        /// The text last put on the system clipboard.
        copied: Option<String>,
    }

    impl Window {
        fn new() -> Self {
            Self::with(serde_json::json!([
                { "id": "u1", "kind": "actor", "label": "Rep", "x": 40, "y": 40 },
                { "id": "u2", "kind": "actor", "label": "Agent", "x": 240, "y": 40 },
                { "id": "a1", "kind": "application", "label": "CRM", "x": 40, "y": 300 },
            ]))
        }

        fn with(elements: serde_json::Value) -> Self {
            let elements = serde_json::from_value(elements).unwrap();
            let eng = Engine::<Conceptual>::new(elements, vec![], DiagramColors::default())
                .with_measure(&ESTIMATE);
            let salt = Id::unique("test");
            let mut window = Self {
                ctx: egui::Context::default(),
                editor: Editor::new(eng, salt),
                salt,
                copied: None,
            };
            window.frame(vec![], Modifiers::NONE);
            window
        }

        fn frame(&mut self, events: Vec<Event>, modifiers: Modifiers) {
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1600.0, 1000.0),
                )),
                events: [Event::ModifiersChanged(modifiers)]
                    .into_iter()
                    .chain(events)
                    .collect(),
                ..RawInput::default()
            };
            let editor = &mut self.editor;
            // Nothing draws the output, so its new textures are let go.
            let mut output = self.ctx.run_ui(input, |ui| editor.ui(ui));
            output.textures_delta.clear();
            for command in output.platform_output.commands {
                if let egui::OutputCommand::CopyText(text) = command {
                    self.copied = Some(text);
                }
            }
        }

        /// Where a canvas point is on screen, with the canvas scrolled by `scroll`.
        fn screen(&self, p: Point, scroll: Vec2) -> Pos2 {
            let canvas = self
                .ctx
                .read_response(self.salt.with("canvas"))
                .unwrap()
                .rect;
            View {
                scroll,
                ..View::default()
            }
            .transform(canvas)
            .pos(p)
        }

        fn drag(&mut self, from: Pos2, to: Pos2, button: PointerButton, modifiers: Modifiers) {
            let press = |pos, pressed| Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            };
            self.frame(vec![Event::PointerMoved(from)], modifiers);
            self.frame(vec![press(from, true)], modifiers);
            for t in [0.25, 0.5, 0.75, 1.0] {
                self.frame(vec![Event::PointerMoved(from + (to - from) * t)], modifiers);
            }
            self.frame(vec![press(to, false)], modifiers);
            self.frame(vec![], Modifiers::NONE);
        }

        fn selection(&self) -> Vec<&str> {
            let mut ids: Vec<&str> = self
                .editor
                .eng
                .selection
                .iter()
                .map(ElementId::as_str)
                .collect();
            ids.sort_unstable();
            ids
        }
    }

    #[test]
    fn dragging_empty_canvas_selects_what_the_box_holds() {
        let mut w = Window::new();
        let at = |w: &Window, x, y| w.screen(Point::new(x, y), Vec2::ZERO);
        // Both actors, but not the application below them.
        let (from, to) = (at(&w, 20.0, 20.0), at(&w, 400.0, 160.0));
        w.drag(from, to, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(w.selection(), ["u1", "u2"]);
        // With Shift, the application is added to them.
        let (from, to) = (at(&w, 20.0, 280.0), at(&w, 400.0, 500.0));
        w.drag(from, to, PointerButton::Primary, Modifiers::SHIFT);
        assert_eq!(w.selection(), ["a1", "u1", "u2"]);
        // Without, a box around only part of an element selects nothing.
        let (from, to) = (at(&w, 20.0, 20.0), at(&w, 100.0, 100.0));
        w.drag(from, to, PointerButton::Primary, Modifiers::NONE);
        assert!(w.selection().is_empty());
    }

    #[test]
    fn the_properties_panel_moves_the_canvas_off_what_it_would_cover_and_back() {
        let mut w = Window::with(serde_json::json!([
            { "id": "a1", "kind": "application", "label": "CRM", "x": 40, "y": 40 },
            { "id": "a2", "kind": "application", "label": "Billing", "x": 1200, "y": 40 },
        ]));
        let canvas = |w: &Window| w.ctx.read_response(w.salt.with("canvas")).unwrap().rect;
        let wide = canvas(&w);
        let billing = w.editor.eng.laid(&"a2".into()).unwrap().rect;
        let on_screen = |w: &Window| w.editor.view().transform(canvas(w)).rect(billing);
        // Billing is in view until it's selected and the panel opens over it.
        assert!(on_screen(&w).max.x < wide.max.x);
        let click = w.screen(billing.center(), Vec2::ZERO);
        w.drag(click, click, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(w.selection(), ["a2"]);
        assert_eq!(
            w.editor.eng.laid(&"a2".into()).unwrap().rect,
            billing,
            "a click moved it"
        );
        let narrow = canvas(&w);
        assert!(narrow.max.x < wide.max.x - 200.0, "the panel took room");
        let shown = on_screen(&w);
        assert!(
            shown.max.x <= narrow.max.x - 20.0,
            "{shown:?} is under the panel"
        );
        let shifted = w.editor.view().scroll;
        assert!(shifted.x < 0.0);
        // Now in view, it moves with the pointer, and a little jiggle in a click doesn't.
        let grab = w.screen(billing.center(), shifted);
        w.drag(
            grab,
            grab + Vec2::new(1.0, 1.0),
            PointerButton::Primary,
            Modifiers::NONE,
        );
        assert_eq!(w.editor.eng.laid(&"a2".into()).unwrap().rect, billing);
        w.drag(
            grab,
            grab - Vec2::new(100.0, 0.0),
            PointerButton::Primary,
            Modifiers::NONE,
        );
        assert_eq!(
            w.editor.eng.laid(&"a2".into()).unwrap().rect.x,
            billing.x - 100.0
        );
        assert_eq!(w.editor.view().scroll, shifted);
        // With the panel open, the canvas still moves out from under it, to the paper's edge.
        let from = narrow.center();
        w.drag(
            from,
            from - Vec2::new(5000.0, 0.0),
            PointerButton::Secondary,
            Modifiers::NONE,
        );
        let paper = w.editor.view().paper_size().x;
        assert_eq!(w.editor.view().scroll.x, narrow.width() - paper);
        w.drag(
            from,
            from + Vec2::new(shifted.x - (narrow.width() - paper), 0.0),
            PointerButton::Secondary,
            Modifiers::NONE,
        );
        assert_eq!(w.editor.view().scroll.x, shifted.x);
        // Selecting nothing closes the panel, and the canvas moves back to the right.
        let empty = w.screen(Point::new(600.0, 600.0), shifted);
        w.drag(empty, empty, PointerButton::Primary, Modifiers::NONE);
        assert!(w.selection().is_empty());
        assert_eq!(canvas(&w), wide);
        assert_eq!(w.editor.view().scroll.x, 0.0);
    }

    #[test]
    fn closing_the_properties_panel_at_the_right_edge_scrolls_back_once() {
        let mut w = Window::with(serde_json::json!([
            { "id": "a1", "kind": "application", "label": "CRM", "x": 40, "y": 40 },
            { "id": "a2", "kind": "application", "label": "Billing", "x": 2180, "y": 40 },
        ]));
        let canvas = |w: &Window| w.ctx.read_response(w.salt.with("canvas")).unwrap().rect;
        let wide = canvas(&w);
        let paper = w.editor.view().paper_size().x;
        // Scrolled all the way right, Billing is at the paper's right edge.
        let from = wide.center();
        w.drag(
            from,
            from - Vec2::new(5000.0, 0.0),
            PointerButton::Secondary,
            Modifiers::NONE,
        );
        let edge = w.editor.view().scroll;
        assert_eq!(edge.x, wide.width() - paper);
        let billing = w.editor.eng.laid(&"a2".into()).unwrap().rect;
        let click = w.screen(billing.center(), edge);
        w.drag(click, click, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(w.selection(), ["a2"]);
        let narrow = canvas(&w);
        assert_eq!(w.editor.view().scroll.x, narrow.width() - paper);
        // Selecting nothing puts the canvas back where it was, not further right.
        let empty = w.screen(Point::new(1600.0, 600.0), w.editor.view().scroll);
        w.drag(empty, empty, PointerButton::Primary, Modifiers::NONE);
        assert!(w.selection().is_empty());
        assert_eq!(w.editor.view().scroll, edge);
    }

    #[test]
    fn dragging_with_the_right_button_scrolls_over_elements() {
        let mut w = Window::new();
        let u1 = w.editor.eng.laid(&"u1".into()).unwrap().rect;
        let u2 = w.editor.eng.laid(&"u2".into()).unwrap().rect;
        let from = w.screen(u1.center(), Vec2::ZERO);
        let pan = Vec2::new(-100.0, -50.0);
        w.drag(from, from + pan, PointerButton::Secondary, Modifiers::NONE);
        // Starting on the actor neither selected nor moved it.
        assert!(w.selection().is_empty());
        assert_eq!(w.editor.eng.laid(&"u1".into()).unwrap().rect, u1);
        // The canvas moved with the pointer: the other actor is where it's now drawn.
        let click = w.screen(u2.center(), pan);
        w.drag(click, click, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(w.selection(), ["u2"]);
    }

    #[test]
    fn the_scroll_wheel_scrolls_and_zooms_with_cmd_or_ctrl() {
        use egui::{MouseWheelSource as Source, MouseWheelUnit as Unit};

        let scroll = |w: &mut Window, unit, source, y: f32, modifiers: Modifiers| {
            let wheel = Event::MouseWheel {
                unit,
                delta: Vec2::new(0.0, y),
                phase: egui::TouchPhase::Move,
                source,
                modifiers,
            };
            let at = w.screen(Point::new(400.0, 400.0), w.editor.view().scroll);
            w.frame(vec![Event::PointerMoved(at), wheel], modifiers);
            // Zooming with the wheel is spread over a few frames.
            for _ in 0..30 {
                w.frame(vec![], modifiers);
            }
            w.editor.view()
        };
        let mut w = Window::new();
        let start = w.editor.view();
        // A mouse wheel turned towards the user scrolls down, as does a trackpad, and neither
        // zooms.
        let down = scroll(&mut w, Unit::Line, Source::Wheel, -1.0, Modifiers::NONE);
        assert_eq!(down.zoom, start.zoom);
        assert!(down.scroll.y < start.scroll.y, "{down:?}");
        let pad = scroll(
            &mut w,
            Unit::Point,
            Source::Trackpad,
            -30.0,
            Modifiers::NONE,
        );
        assert_eq!(pad.zoom, start.zoom);
        assert!(pad.scroll.y < down.scroll.y, "{pad:?}");
        // With Shift it scrolls sideways.
        let sideways = scroll(&mut w, Unit::Line, Source::Wheel, -1.0, Modifiers::SHIFT);
        assert_eq!(sideways.zoom, start.zoom);
        assert!(sideways.scroll.x < pad.scroll.x, "{sideways:?}");
        // With Cmd, or Ctrl, it zooms out towards the user and in the other way.
        let out = scroll(&mut w, Unit::Line, Source::Wheel, -1.0, Modifiers::COMMAND);
        assert!(out.zoom < start.zoom, "{out:?}");
        let back = scroll(&mut w, Unit::Line, Source::Wheel, 1.0, Modifiers::CTRL);
        assert!(back.zoom > out.zoom, "{back:?}");
        let ctrl = scroll(&mut w, Unit::Line, Source::Wheel, -1.0, Modifiers::CTRL);
        assert!(ctrl.zoom < back.zoom, "{ctrl:?}");
    }

    #[test]
    fn cmd_or_ctrl_and_plus_or_minus_zoom() {
        let key = |w: &mut Window, key, modifiers| {
            let press = Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            };
            w.frame(vec![press], modifiers);
            w.frame(vec![], Modifiers::NONE);
            w.editor.view().zoom
        };
        let mut w = Window::new();
        w.frame(vec![], Modifiers::NONE);
        let start = w.editor.view().zoom;
        let out = key(&mut w, egui::Key::Minus, Modifiers::COMMAND);
        assert!(out < start, "{out}");
        let further = key(&mut w, egui::Key::Minus, Modifiers::CTRL);
        assert!(further < out, "{further}");
        let back = key(&mut w, egui::Key::Plus, Modifiers::COMMAND);
        assert!((back - out).abs() < 1e-4, "{back}");
        let equals = key(&mut w, egui::Key::Equals, Modifiers::CTRL);
        assert!((equals - start).abs() < 1e-4, "{equals}");
        // The window itself isn't scaled.
        assert_eq!(w.ctx.zoom_factor(), 1.0);
        // Without Cmd or Ctrl, the keys don't zoom.
        assert_eq!(key(&mut w, egui::Key::Minus, Modifiers::NONE), equals);
    }

    #[test]
    fn dragging_with_the_middle_button_moves_the_canvas() {
        for viewing in [false, true] {
            let mut w = Window::new();
            w.editor.set_viewing(viewing);
            w.frame(vec![], Modifiers::NONE);
            let u1 = w.editor.eng.laid(&"u1".into()).unwrap().rect;
            let start = w.editor.view().scroll;
            // From over an element, which stays where it is.
            let from = w.screen(u1.center(), start);
            let pan = Vec2::new(-100.0, -50.0);
            w.drag(from, from + pan, PointerButton::Middle, Modifiers::NONE);
            assert_eq!(w.editor.view().scroll, start + pan, "viewing: {viewing}");
            assert_eq!(w.editor.eng.laid(&"u1".into()).unwrap().rect, u1);
            assert!(w.selection().is_empty());
        }
    }

    #[test]
    fn viewing_only_moves_the_canvas() {
        use hiero_editor::DiagramEditor;

        let mut w = Window::new();
        let u1 = w.editor.eng.laid(&"u1".into()).unwrap().rect;
        let u2 = w.editor.eng.laid(&"u2".into()).unwrap().rect;
        w.editor.eng.select(&"u1".into());
        w.editor.set_viewing(true);
        assert!(w.selection().is_empty(), "viewing drops the selection");
        // The canvas takes the palette's room.
        w.frame(vec![], Modifiers::NONE);
        // Dragging an element with the left button moves the canvas, not the element.
        let from = w.screen(u1.center(), Vec2::ZERO);
        let pan = Vec2::new(-100.0, -50.0);
        w.drag(from, from + pan, PointerButton::Primary, Modifiers::NONE);
        assert!(w.selection().is_empty());
        assert_eq!(w.editor.eng.laid(&"u1".into()).unwrap().rect, u1);
        // Clicking selects nothing, and deleting, cutting, and pasting change nothing.
        let click = w.screen(u2.center(), pan);
        w.drag(click, click, PointerButton::Primary, Modifiers::NONE);
        assert!(w.selection().is_empty());
        let delete = Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        w.frame(vec![delete, Event::Cut], Modifiers::NONE);
        w.frame(vec![Event::Paste("Hello".into())], Modifiers::NONE);
        assert_eq!(w.editor.eng.elements().len(), 3);
        assert!(w.editor.take_changes().is_empty());
        assert!(w.editor.take_notice().is_none());
        // Editing again, a click selects.
        w.editor.set_viewing(false);
        // The palette comes back, and the canvas settles beside it.
        w.frame(vec![], Modifiers::NONE);
        w.frame(vec![], Modifiers::NONE);
        let click = w.screen(u2.center(), pan);
        w.drag(click, click, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(w.selection(), ["u2"]);
    }

    #[test]
    fn cut_and_paste_go_through_the_system_clipboard() {
        use hiero_editor::DiagramEditor;

        let mut w = Window::new();
        let rep = ElementId::from("u1");
        w.editor.eng.select(&rep);
        w.frame(vec![Event::Cut], Modifiers::NONE);
        let text = w.copied.clone().expect("cutting copies");
        assert!(w.editor.eng.element(&rep).is_none());
        assert_eq!(w.editor.take_changes().len(), 1);

        w.frame(vec![Event::Paste(text)], Modifiers::NONE);
        assert_eq!(w.editor.take_changes().len(), 1);
        let pasted = w
            .editor
            .eng
            .selected()
            .expect("the pasted actor is selected");
        assert_eq!((pasted.el.label.as_str(), pasted.rect.x), ("Rep", 40.0));

        // Other text isn't pasted, and the app is told why.
        w.frame(vec![Event::Paste("Hello".into())], Modifiers::NONE);
        assert!(w.editor.take_changes().is_empty());
        assert_eq!(
            w.editor.take_notice().as_deref(),
            Some("The clipboard doesn’t hold diagram elements.")
        );
    }
}
