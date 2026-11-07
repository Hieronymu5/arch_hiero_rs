//! The sequence diagram's rules: what a new diagram starts with, how the participants stand in
//! a row and change places, what a new message starts as, and how elements and messages are
//! checked and read to screen readers.

use hiero_core::engine::{
    Conn, DiagramType, Dir, El, Engine, Laid, Placement, validate_label, validate_length,
};
use hiero_core::file_check::{DescriptionRule, Field, FieldRule};
use hiero_core::geometry::{Point, Rect, Size};
use hiero_core::model::{DiagramTypeId, Element, ElementId, KindInfo, NoColors};

use crate::model::{
    ComponentType, DESCRIPTION_MAX, FIGURE, LABEL_MAX, MessageKind, PALETTE, ParticipantColor,
    Sequence, SequenceKind, SequenceMessage, SequenceProps,
};
use crate::timeline::{GAP, LEFT, TOP, box_height, ordered, room_for_text, timeline};

type SEngine = Engine<Sequence>;

const COLOR: Field = Field {
    key: "color",
    rule: FieldRule::OneOf {
        values: &["green", "red", "yellow"],
        default: Some("blue"),
    },
};
const ACTOR_FIELDS: &[Field] = &[
    COLOR,
    Field {
        key: "agent",
        rule: FieldRule::True,
    },
];
const APPLICATION_FIELDS: &[Field] = &[COLOR];
const COMPONENT_FIELDS: &[Field] = &[
    COLOR,
    Field {
        key: "componentType",
        rule: FieldRule::OneOf {
            values: &["database", "storage"],
            default: None,
        },
    },
];

const ARROW_FIELDS: &[Field] = &[
    Field {
        key: "kind",
        rule: FieldRule::OneOf {
            values: &["async", "return"],
            default: Some("call"),
        },
    },
    Field {
        key: "label",
        rule: FieldRule::Text {
            name: "Text",
            max: LABEL_MAX,
        },
    },
];

/// The participants a new diagram starts with: someone, and the application they use.
const STARTING: [(SequenceKind, &str); 2] = [
    (SequenceKind::Actor, "User"),
    (SequenceKind::Application, "Application"),
];

/// A message as stored: its text trimmed, and left out when blank.
pub fn normalized_arrow(arrow: SequenceMessage) -> SequenceMessage {
    SequenceMessage {
        label: arrow
            .label
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty()),
        ..arrow
    }
}

/// Such as "2. Return to User: Order", for the list of a participant's messages.
pub fn arrow_title(eng: &SEngine, message: &Conn<Sequence>, outgoing: bool, other: &str) -> String {
    let number = eng
        .connections()
        .iter()
        .position(|c| c.id == message.id)
        .map_or(0, |i| i + 1);
    let kind = MessageKind::label(message.props.kind);
    let mut out = if message.source_id == message.target_id {
        format!("{number}. {kind} to itself")
    } else {
        format!(
            "{number}. {kind} {} {other}",
            if outgoing { "to" } else { "from" }
        )
    };
    if let Some(label) = &message.props.label {
        out.push_str(&format!(": {label}"));
    }
    out
}

/// The width an element takes: as resized, or its kind's.
fn width_of(element: &El<Sequence>) -> f64 {
    element
        .width
        .unwrap_or_else(|| Sequence::natural_size(element, Sequence::info(element.kind)).width)
}

fn center_of(element: &El<Sequence>) -> f64 {
    element.x + width_of(element) / 2.0
}

/// The participants from left to right, as indexes into `elements`.
fn left_to_right(elements: &[El<Sequence>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..elements.len()).collect();
    order.sort_by(|&a, &b| {
        center_of(&elements[a])
            .total_cmp(&center_of(&elements[b]))
            .then(a.cmp(&b))
    });
    order
}

/// Where a participant stands in the row, from 1, and how many there are.
fn place_in_row(eng: &SEngine, id: &ElementId) -> (usize, usize) {
    let elements = eng.elements();
    let order = left_to_right(elements);
    let place = order
        .iter()
        .position(|&i| elements[i].id == *id)
        .map_or(0, |p| p + 1);
    (place, elements.len())
}

/// The participants in a row along the top, in the order of their centres, each far enough
/// from the one before for the messages between them to fit their text, and each box as tall
/// as its name and description need.
pub fn arrange(eng: &SEngine, elements: Vec<El<Sequence>>) -> Vec<El<Sequence>> {
    let order = left_to_right(&elements);
    let mut out = elements.clone();
    let mut previous: Option<(&ElementId, f64, f64)> = None;
    for i in order {
        let element = &elements[i];
        let width = width_of(element);
        let center = match previous {
            None => LEFT + width / 2.0,
            Some((left, left_center, left_width)) => {
                left_center
                    + (left_width / 2.0 + GAP + width / 2.0)
                        .max(room_for_text(eng, left, &element.id))
                        .ceil()
            }
        };
        out[i].x = center - width / 2.0;
        out[i].y = TOP;
        out[i].height = box_height(element, width, eng.measure());
        out[i].parent_id = None;
        previous = Some((&element.id, center, width));
    }
    out
}

/// Such as "an arrow to Billing: getOrder(id), 2 of 5", for each of a participant's messages.
fn message_details(eng: &SEngine, element: &Laid<Sequence>) -> Vec<String> {
    let label = |id: &ElementId| eng.label_of(id).unwrap_or_default().to_owned();
    let count = eng.connections().len();
    eng.connections()
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let kind = MessageKind::label(c.props.kind).to_lowercase();
            let text = c
                .props
                .label
                .as_ref()
                .map(|l| format!(": {l}"))
                .unwrap_or_default();
            let at = format!(", {} of {count}", i + 1);
            if c.source_id == *element.id() && c.target_id == *element.id() {
                Some(format!("{kind} to itself{text}{at}"))
            } else if c.source_id == *element.id() {
                Some(format!("{kind} to {}{text}{at}", label(&c.target_id)))
            } else if c.target_id == *element.id() {
                Some(format!("{kind} from {}{text}{at}", label(&c.source_id)))
            } else {
                None
            }
        })
        .collect()
}

/// Moves a participant one place left or right in the row.
fn step_in_row(eng: &mut SEngine, id: &ElementId, dir: Dir) {
    let elements = eng.elements().to_vec();
    let order = left_to_right(&elements);
    let Some(at) = order.iter().position(|&i| elements[i].id == *id) else {
        return;
    };
    let neighbour = match dir {
        Dir::Left if at > 0 => order[at - 1],
        Dir::Right if at + 1 < order.len() => order[at + 1],
        _ => {
            let name = &elements[order[at]].label;
            eng.announcement = format!(
                "{name} is already {}.",
                if dir == Dir::Left { "first" } else { "last" }
            );
            return;
        }
    };
    // Just past the neighbour's centre, so the row puts it on the other side.
    let past = center_of(&elements[neighbour]) + if dir == Dir::Left { -1.0 } else { 1.0 };
    eng.replace(id, |mut e| {
        e.x = past - width_of(&e) / 2.0;
        e
    });
    let (place, count) = place_in_row(eng, id);
    let name = eng.label_of(id).unwrap_or_default().to_owned();
    eng.announcement = format!("{name} moved to {place} of {count}.");
}

fn new_participant(eng: &mut SEngine, kind: SequenceKind, label: &str, x: f64) -> El<Sequence> {
    Element {
        id: ElementId(eng.new_id()),
        kind,
        label: label.to_owned(),
        description: None,
        parent_id: None,
        sub_diagram_id: None,
        props: SequenceProps::default(),
        width: None,
        height: None,
        x,
        y: TOP,
    }
}

impl DiagramType for Sequence {
    const ID: DiagramTypeId = DiagramTypeId::Sequence;
    type Kind = SequenceKind;
    type Props = SequenceProps;
    type Arrow = SequenceMessage;
    type Colors = NoColors;

    const ARROW_FIELDS: &'static [Field] = ARROW_FIELDS;
    const REPEATED_ARROWS: bool = true;
    const SELF_ARROWS: bool = true;
    const ARROWS_ARRANGE: bool = true;
    const DESCRIPTION: DescriptionRule = DescriptionRule {
        max: DESCRIPTION_MAX,
        max_lines: None,
    };

    fn file_fields(kind: SequenceKind) -> &'static [Field] {
        match kind {
            SequenceKind::Actor => ACTOR_FIELDS,
            SequenceKind::Application => APPLICATION_FIELDS,
            SequenceKind::Component => COMPONENT_FIELDS,
        }
    }

    fn normalized_arrow(arrow: SequenceMessage) -> SequenceMessage {
        normalized_arrow(arrow)
    }

    fn palette() -> &'static [KindInfo<SequenceKind>] {
        &PALETTE
    }

    fn starting_elements(eng: &mut Engine<Self>) -> Vec<El<Self>> {
        let mut x = LEFT;
        let mut out = Vec::new();
        for (kind, label) in STARTING {
            let element = new_participant(eng, kind, label, x);
            x += width_of(&element) + GAP;
            out.push(element);
        }
        arrange(eng, out)
    }

    fn arrange(eng: &Engine<Self>, elements: Vec<El<Self>>) -> Vec<El<Self>> {
        arrange(eng, elements)
    }

    /// A component drawn as a database or storage is a figure, as an actor is.
    fn natural_size(element: &El<Self>, info: &'static KindInfo<SequenceKind>) -> Size {
        if element.kind == SequenceKind::Component && element.props.component_type.is_some() {
            FIGURE
        } else {
            info.size()
        }
    }

    /// At the end of the row.
    fn preferred_position(
        eng: &Engine<Self>,
        kind: &'static KindInfo<SequenceKind>,
    ) -> Option<Point> {
        let right = eng
            .layout()
            .iter()
            .map(|e| e.rect.right() + GAP)
            .fold(LEFT, f64::max);
        Some(Point::new(
            right.min(eng.canvas().width - kind.size().width),
            TOP,
        ))
    }

    /// Along the row only: where it's let go sets its place in it.
    fn constrain_move(
        _eng: &Engine<Self>,
        _element: &Laid<Self>,
        placement: Placement,
    ) -> Placement {
        Placement {
            position: Point::new(placement.position.x, TOP),
            parent_id: None,
        }
    }

    /// Left and right change places with the participant beside it; up and down do nothing,
    /// as the participants stay in their row.
    fn element_key(eng: &mut Engine<Self>, id: &ElementId, dir: Dir, _shift: bool) -> bool {
        if dir.is_across() {
            step_in_row(eng, id, dir);
        }
        true
    }

    /// Such as "Billing, 2 of 4".
    fn describe(eng: &Engine<Self>, element: &El<Self>) -> String {
        let (place, count) = place_in_row(eng, &element.id);
        format!("{}, {place} of {count}", element.label)
    }

    /// Such as "Actor (bot)" or "Component (database), green".
    fn kind_description(_eng: &Engine<Self>, element: &Laid<Self>) -> String {
        let props = &element.el.props;
        let mut out = element.info.label.to_owned();
        if element.el.kind == SequenceKind::Actor && props.agent {
            out.push_str(" (bot)");
        }
        if let Some(t) = props
            .component_type
            .filter(|_| element.el.kind == SequenceKind::Component)
        {
            out.push_str(&format!(
                " ({})",
                ComponentType::label(Some(t)).to_lowercase()
            ));
        }
        out.push_str(&format!(
            ", {}",
            ParticipantColor::label(props.color).to_lowercase()
        ));
        out
    }

    fn extra_details(eng: &Engine<Self>, element: &Laid<Self>) -> Vec<String> {
        message_details(eng, element)
    }

    /// Any participant to any other, or to itself.
    fn can_connect(_eng: &Engine<Self>, source: &Laid<Self>, target: &Laid<Self>) -> bool {
        source.info.connects_to.contains(&target.el.kind)
    }

    /// A call, unless it answers the last message, a call the other way: then a return.
    fn new_arrow(eng: &Engine<Self>, source: &ElementId, target: &ElementId) -> SequenceMessage {
        let answers = eng.connections().last().is_some_and(|c| {
            c.source_id == *target
                && c.target_id == *source
                && source != target
                && c.props.kind.is_none()
        });
        SequenceMessage {
            kind: answers.then_some(MessageKind::Return),
            label: None,
        }
    }

    /// Such as "Call from User to Application added, 3 of 3."
    fn arrow_added(eng: &Engine<Self>, connection: &Conn<Self>) -> String {
        let name = |id: &ElementId| eng.label_of(id).unwrap_or("?").to_owned();
        let count = eng.connections().len() + 1;
        format!(
            "{} from {} to {} added, {count} of {count}.",
            MessageKind::label(connection.props.kind),
            name(&connection.source_id),
            name(&connection.target_id)
        )
    }

    /// The participant and its lifeline, so a message can be dropped anywhere along it.
    fn connect_area(eng: &Engine<Self>, element: &Laid<Self>) -> Rect {
        let order = ordered(eng, eng.layout(), None);
        let bottom = timeline(eng, eng.layout(), &order).bottom();
        Rect {
            height: (bottom - element.rect.y).max(element.rect.height),
            ..element.rect
        }
    }

    fn validate_element(element: &El<Self>) -> Result<(), String> {
        validate_label(&element.label)?;
        validate_length(
            "Description",
            element.description.as_deref().unwrap_or_default(),
            DESCRIPTION_MAX,
        )
    }

    fn validate_arrow(arrow: &SequenceMessage) -> Result<(), String> {
        validate_length(
            "Text",
            arrow.label.as_deref().unwrap_or_default(),
            LABEL_MAX,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_engine as engine;

    fn row(eng: &SEngine) -> Vec<String> {
        let elements = eng.elements();
        left_to_right(elements)
            .into_iter()
            .map(|i| elements[i].label.clone())
            .collect()
    }

    #[test]
    fn a_new_diagram_has_a_user_and_an_application_in_a_row() {
        let eng = engine();
        assert_eq!(row(&eng), ["User", "Application"]);
        let laid = eng.layout();
        assert!(laid.iter().all(|e| e.rect.y == TOP));
        assert_eq!(laid[0].rect.x, LEFT);
        assert_eq!(laid[1].rect.x, LEFT + laid[0].rect.width + GAP);
    }

    #[test]
    fn added_participants_go_at_the_end_of_the_row() {
        let mut eng = engine();
        eng.add_at_free_position(SequenceKind::Component);
        assert_eq!(row(&eng), ["User", "Application", "Component"]);
        assert_eq!(eng.announcement, "Component, 3 of 3 added.");
    }

    #[test]
    fn participants_change_places_by_dragging_or_with_the_arrow_keys() {
        let mut eng = engine();
        eng.add_at_free_position(SequenceKind::Component);
        // Dragged to the left of the user, and down: it stays in the row, first.
        let component = eng.laid(&"id3".into()).unwrap().clone();
        let placement = eng.move_placement(&component, Point::new(0.0, 300.0));
        eng.commit_move(&"id3".into(), placement);
        assert_eq!(row(&eng), ["Component", "User", "Application"]);
        assert!(eng.layout().iter().all(|e| e.rect.y == TOP));
        // The arrow keys move it one place at a time.
        eng.arrow_key(&"id3".into(), Dir::Right, false);
        assert_eq!(row(&eng), ["User", "Component", "Application"]);
        assert_eq!(eng.announcement, "Component moved to 2 of 3.");
        eng.arrow_key(&"id1".into(), Dir::Left, false);
        assert_eq!(eng.announcement, "User is already first.");
        eng.arrow_key(&"id1".into(), Dir::Down, false);
        assert_eq!(row(&eng), ["User", "Component", "Application"]);
    }

    #[test]
    fn messages_repeat_go_to_the_sender_and_answer_calls() {
        let mut eng = engine();
        let (user, app) = (ElementId::from("id1"), ElementId::from("id2"));
        let call = eng.add_connection(&user, &app).unwrap();
        assert_eq!(
            eng.announcement,
            "Call from User to Application added, 1 of 1."
        );
        // The answer to a call starts as a return.
        let back = eng.add_connection(&app, &user).unwrap();
        assert_eq!(
            eng.connection(&back).unwrap().props.kind,
            Some(MessageKind::Return)
        );
        // Another call the same way, and one to itself.
        assert!(eng.add_connection(&user, &app).is_some());
        let own = eng.add_connection(&app, &app).unwrap();
        assert_eq!(eng.connection(&own).unwrap().props.kind, None);
        assert_eq!(eng.connections().len(), 4);
        eng.move_connection(&own, 0);
        assert_eq!(eng.connections()[0].id, own);
        assert_eq!(eng.connections()[1].id, call);
    }

    #[test]
    fn participants_move_apart_to_fit_the_text_between_them() {
        let mut eng = engine();
        let (user, app) = (ElementId::from("id1"), ElementId::from("id2"));
        let before = eng.laid(&app).unwrap().rect.x;
        let id = eng.add_connection(&user, &app).unwrap();
        eng.set_connection_props(
            &id,
            SequenceMessage {
                kind: None,
                label: Some("x".repeat(60)),
            },
        );
        let after = eng.laid(&app).unwrap().rect.x;
        assert!(after > before);
        let centers: Vec<f64> = eng.layout().iter().map(|e| e.rect.center().x).collect();
        let text = 60.0 * crate::timeline::LABEL_SIZE * 0.55;
        assert!(centers[1] - centers[0] >= text);
    }
}
