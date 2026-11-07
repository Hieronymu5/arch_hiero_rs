//! The sequence diagram's properties panels and its place in the shared editor: messages are
//! dragged up and down, or moved with the arrow keys, to change their order.

use egui::Ui;
use hiero_core::engine::{Conn, DiagramType, Dir, Engine, Laid, validate_length};
use hiero_core::geometry::{Point, Rect};
use hiero_core::model::ConnectionId;
use hiero_core::scene::Scene;
use hiero_editor::theme::page_button;
use hiero_editor::widgets::{self, text_field};
use hiero_editor::{ArrowLine, DiagramView, PropsCx};

use crate::model::{
    ComponentType, DESCRIPTION_MAX, LABEL_MAX, MessageKind, ParticipantColor, Sequence,
    SequenceKind, SequenceMessage,
};
use crate::rules::{arrow_title, normalized_arrow};
use crate::scene::{drawn_rect, scene_of, timeline_of};
use crate::timeline::{SELF_HEIGHT, SELF_WIDTH, head_bottom, head_shape, index_at, is_figure};

/// Moves the message `id` one place earlier or later. Returns whether it could.
fn step_message(eng: &mut Engine<Sequence>, id: &ConnectionId, later: bool) -> bool {
    let Some(at) = eng.connections().iter().position(|c| c.id == *id) else {
        return false;
    };
    let count = eng.connections().len();
    let name = eng
        .connection(id)
        .map(|c| eng.arrow_name(&c.source_id, &c.target_id))
        .unwrap_or_default();
    match (later, at) {
        (false, 0) => eng.announcement = format!("{name} is already first."),
        (true, _) if at + 1 == count => eng.announcement = format!("{name} is already last."),
        _ => {
            eng.move_connection(id, if later { at + 1 } else { at - 1 });
            return true;
        }
    }
    false
}

/// Adds the answer to the call `id` right after it: a return from where it went back to where
/// it came from, and selects it.
fn add_return(eng: &mut Engine<Sequence>, id: &ConnectionId) {
    let Some(call) = eng.connection(id).cloned() else {
        return;
    };
    let Some(at) = eng.connections().iter().position(|c| c.id == *id) else {
        return;
    };
    let Some(back) = eng.add_connection(&call.target_id, &call.source_id) else {
        return;
    };
    let announcement = eng.announcement.clone();
    eng.set_connection_props(
        &back,
        SequenceMessage {
            kind: Some(MessageKind::Return),
            label: None,
        },
    );
    eng.move_connection(&back, at + 1);
    eng.select_connection(&back);
    eng.announcement = announcement.replace("Call", "Return");
}

impl DiagramView for Sequence {
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
        timeline_of(eng, layout, slide)
            .messages
            .into_iter()
            .map(|m| {
                if m.to_self {
                    ArrowLine {
                        id: m.id,
                        from: m.from,
                        to: Point::new(m.from.x + SELF_WIDTH, m.from.y),
                        boxes: vec![Rect::new(m.from.x, m.from.y, SELF_WIDTH, SELF_HEIGHT)],
                    }
                } else {
                    ArrowLine {
                        id: m.id,
                        from: m.from,
                        to: m.to,
                        boxes: Vec::new(),
                    }
                }
            })
            .collect()
    }

    fn drawn_rect(eng: &Engine<Self>, element: &Laid<Self>) -> Rect {
        drawn_rect(eng, element)
    }

    /// Where the lifeline meets the participant, clear of the resize handles on its corners.
    fn connect_handle_at(eng: &Engine<Self>, element: &Laid<Self>) -> Option<Point> {
        Some(Point::new(
            element.rect.center().x,
            head_bottom(eng, element),
        ))
    }

    fn element_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, element: &Laid<Self>) {
        let mut with = element.el.clone();
        let mut field = None;
        if cx.name_field(ui, element, &mut with) {
            field = Some("label");
        }
        let kind = element.el.kind;
        // A box shows its description; a figure only its name.
        if element.info.has_description
            && !is_figure(head_shape(kind, element.el.props.component_type))
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
        if kind == SequenceKind::Actor
            && widgets::checkbox(ui, &mut with.props.agent, "Bot").changed()
        {
            field = Some("agent");
        }
        if kind == SequenceKind::Component {
            widgets::label(ui, "Drawn as", false);
            ui.horizontal_wrapped(|ui| {
                for value in ComponentType::CHOICES {
                    if ui
                        .radio_value(
                            &mut with.props.component_type,
                            value,
                            ComponentType::label(value),
                        )
                        .changed()
                    {
                        field = Some("component-type");
                    }
                }
            });
        }
        widgets::label(ui, "Colour (and its lifeline's)", false);
        ui.horizontal_wrapped(|ui| {
            for value in ParticipantColor::CHOICES {
                if ui
                    .radio_value(&mut with.props.color, value, ParticipantColor::label(value))
                    .changed()
                {
                    field = Some("color");
                }
            }
        });
        if let Some(field) = field {
            cx.commit(element, with, None, field);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (dir, text) in [(Dir::Left, "Move left"), (Dir::Right, "Move right")] {
                if page_button(ui, text, false).clicked() {
                    let id = element.id().clone();
                    cx.apply(None, move |eng| eng.arrow_key(&id, dir, false));
                }
            }
        });
    }

    fn arrow_properties(ui: &mut Ui, cx: &mut PropsCx<'_, Self>, connection: &Conn<Self>) {
        let id = connection.id.clone();
        let current = connection.props.clone();
        let mut message = current.clone();
        let mut field: Option<&'static str> = None;
        if let Some(v) = text_field(
            ui,
            cx.arrow_field_id(&id, "label"),
            if current.is_return() {
                "Text (what it returns)"
            } else {
                "Text"
            },
            current.label.as_deref().unwrap_or_default(),
            false,
            |t| validate_length("Text", t, LABEL_MAX),
        ) {
            message.label = Some(v);
            field = Some("label");
        }
        widgets::label(ui, "Type", false);
        ui.horizontal_wrapped(|ui| {
            for value in MessageKind::CHOICES {
                if ui
                    .radio_value(&mut message.kind, value, MessageKind::label(value))
                    .changed()
                {
                    field = Some("kind");
                }
            }
        });
        if let Some(field) = field {
            let message = normalized_arrow(message);
            if Sequence::validate_arrow(&message).is_ok() && message != current {
                let target = id.clone();
                cx.apply(Some(format!("{id}:{field}")), move |eng| {
                    eng.set_connection_props(&target, message);
                });
            }
        }
        ui.add_space(6.0);
        let at = cx
            .eng
            .connections()
            .iter()
            .position(|c| c.id == id)
            .unwrap_or(0);
        widgets::label(
            ui,
            &format!("Message {} of {}", at + 1, cx.eng.connections().len()),
            false,
        );
        ui.horizontal(|ui| {
            for (later, text) in [(false, "Move up"), (true, "Move down")] {
                if page_button(ui, text, false).clicked() {
                    let id = id.clone();
                    cx.apply(None, move |eng| {
                        step_message(eng, &id, later);
                    });
                }
            }
        });
        if current.kind.is_none()
            && connection.source_id != connection.target_id
            && page_button(ui, "Add return", false).clicked()
        {
            let id = id.clone();
            cx.apply(None, move |eng| add_return(eng, &id));
        }
    }

    fn arrow_title(
        eng: &Engine<Self>,
        connection: &Conn<Self>,
        outgoing: bool,
        other: &str,
    ) -> String {
        arrow_title(eng, connection, outgoing, other)
    }

    /// Dragging a message up or down: it goes where it's let go.
    fn slide_offset(_eng: &Engine<Self>, _id: &ConnectionId, pointer: Point) -> Option<f64> {
        Some(pointer.y)
    }

    fn commit_slide(eng: &mut Engine<Self>, id: &ConnectionId, offset: f64) {
        let index = index_at(eng, eng.layout(), id, offset);
        eng.move_connection(id, index);
    }

    /// Up and down move the selected message earlier or later.
    fn connection_key(eng: &mut Engine<Self>, dir: Dir, _shift: bool) -> bool {
        let Some(id) = eng.selected_connection.clone() else {
            return false;
        };
        match dir {
            Dir::Up | Dir::Down => {
                step_message(eng, &id, dir == Dir::Down);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiero_core::model::ElementId;

    #[test]
    fn the_help_says_how_to_order_messages() {
        assert!(Sequence::HELP.contains("Add return"));
        assert!(Sequence::HELP.contains("up or down"));
    }

    #[test]
    fn calls_get_their_returns_and_messages_step_through_the_order() {
        let mut eng = crate::test_engine();
        let (user, app) = (ElementId::from("id1"), ElementId::from("id2"));
        let first = eng.add_connection(&user, &app).unwrap();
        let second = eng.add_connection(&user, &app).unwrap();
        add_return(&mut eng, &first);
        let back = eng.connections()[1].clone();
        assert_eq!(back.props.kind, Some(MessageKind::Return));
        assert_eq!((back.source_id, back.target_id), (app, user));
        assert_eq!(eng.selected_connection, Some(back.id.clone()));
        assert_eq!(eng.connections()[2].id, second);
        // Up and down move the selected message.
        eng.select_connection(&second);
        assert!(Sequence::connection_key(&mut eng, Dir::Up, false));
        assert_eq!(eng.connections()[1].id, second);
        assert!(!step_message(&mut eng, &first, false));
        assert_eq!(
            eng.announcement,
            "Message from User to Application is already first."
        );
        // Dragging it to the bottom puts it last.
        Sequence::commit_slide(&mut eng, &second, 10_000.0);
        assert_eq!(eng.connections()[2].id, second);
    }
}
