//! Where a sequence diagram's lifelines, activations, and messages go. The participants stand
//! in a row along the top (see `rules::arrange`); everything below them follows from their
//! places and the order of the messages, so it's worked out here, for the scene, the editor,
//! and the draw.io export alike.

use std::collections::HashMap;

use hiero_core::engine::{Conn, El, Engine, Laid};
use hiero_core::geometry::{Point, Rect};
use hiero_core::look::{ARROW_LABEL_SIZE, Shape, box_text_height, component_name_bottom};
use hiero_core::model::{ConnectionId, ElementId};
use hiero_core::palette::FillColors;
use hiero_core::text::MeasureText;
use hiero_core::text::Weight;

use crate::model::{
    BOX_SIZE, ComponentType, MessageKind, ParticipantColor, Sequence, SequenceKind, SequenceMessage,
};

/// Where the row of participants starts: their tops, and the first one's left side.
pub const TOP: f64 = 40.0;
pub const LEFT: f64 = 40.0;
/// The least space between two participants side by side.
pub const GAP: f64 = 60.0;
/// From the bottom of the tallest participant to the first message.
pub const FIRST_MESSAGE: f64 = 60.0;
/// From one message to the next.
pub const STEP: f64 = 50.0;
/// How far a message to its own sender goes out to the side, and down before it comes back.
pub const SELF_WIDTH: f64 = 40.0;
pub const SELF_HEIGHT: f64 = 30.0;
/// How far the lifelines run on below the last message.
pub const TAIL: f64 = 40.0;
/// How long a lifeline is with no messages, below its participant.
pub const EMPTY_LIFELINE: f64 = 160.0;
/// How wide an activation, the bar on a lifeline while a call is being answered, is.
pub const BAR_WIDTH: f64 = 12.0;
/// How long an activation is at least.
pub const BAR_MIN: f64 = 20.0;
/// How far a message's text is above its line, to the text's baseline.
pub const LABEL_LIFT: f64 = 6.0;
/// The room kept on either side of a message's text between two lifelines.
pub const LABEL_MARGIN: f64 = 16.0;
/// The size and weight of a message's text.
pub const LABEL_SIZE: f64 = ARROW_LABEL_SIZE;
pub const LABEL_WEIGHT: Weight = Weight::Regular;

/// What a participant is drawn as.
pub fn head_shape(kind: SequenceKind, component_type: Option<ComponentType>) -> Shape {
    match (kind, component_type) {
        (SequenceKind::Actor, _) => Shape::Actor,
        (SequenceKind::Component, Some(ComponentType::Database)) => Shape::Database,
        (SequenceKind::Component, Some(ComponentType::Storage)) => Shape::Storage,
        _ => Shape::Box,
    }
}

/// The description a participant shows: a box's, when it has some text. A figure shows only
/// its name, so a component drawn as a database or storage keeps its description unseen.
pub fn shown_description(element: &El<Sequence>) -> Option<&str> {
    let shape = head_shape(element.kind, element.props.component_type);
    if is_figure(shape) {
        return None;
    }
    element
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

/// The space above and below a box's text.
pub const BOX_PADDING_Y: f64 = 10.0;
/// A box's height is a multiple of this, so a taller one stays on the grid.
const BOX_STEP: f64 = 20.0;

/// The height a participant `width` wide is drawn at, when its name and description need more
/// than its kind's: unset otherwise, and for a figure.
pub fn box_height(element: &El<Sequence>, width: f64, measure: &dyn MeasureText) -> Option<f64> {
    if is_figure(head_shape(element.kind, element.props.component_type)) {
        return None;
    }
    let text = box_text_height(&element.label, shown_description(element), width, measure);
    let height = ((text + BOX_PADDING_Y * 2.0) / BOX_STEP).ceil() * BOX_STEP;
    (height > BOX_SIZE.height).then_some(height)
}

/// Whether a participant is drawn as a figure with its name under it.
pub fn is_figure(shape: Shape) -> bool {
    matches!(shape, Shape::Actor | Shape::Database | Shape::Storage)
}

/// The bottom of a participant as drawn: a figure's name can go on to a second line, below
/// its area.
pub fn head_bottom(eng: &Engine<Sequence>, element: &Laid<Sequence>) -> f64 {
    let shape = head_shape(element.el.kind, element.el.props.component_type);
    if is_figure(shape) {
        element.rect.bottom().max(component_name_bottom(
            element.rect,
            &element.el.label,
            shape,
            eng.measure(),
        ))
    } else {
        element.rect.bottom()
    }
}

/// A participant's line down the diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct Lifeline {
    pub id: ElementId,
    pub x: f64,
    /// Where it starts, under its participant, and where it ends.
    pub top: f64,
    pub bottom: f64,
    pub colors: FillColors,
}

/// An activation: a bar on a lifeline from a call it receives to the answer it returns.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub participant: ElementId,
    pub rect: Rect,
    pub colors: FillColors,
}

/// A message as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub id: ConnectionId,
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub kind: Option<MessageKind>,
    pub label: Option<String>,
    /// Its line's ends. A message to its sender goes out to the side and back, from `from`
    /// down to `to`.
    pub from: Point,
    pub to: Point,
    pub to_self: bool,
}

impl Placed {
    /// The height it's drawn at: its line's, or where a message to itself leaves.
    pub fn y(&self) -> f64 {
        self.from.y
    }

    /// The loop of a message to its sender.
    pub fn self_loop(&self) -> Rect {
        Rect::new(self.from.x, self.from.y, SELF_WIDTH, SELF_HEIGHT)
    }

    /// Where its text goes: centred above a line, or beside a loop, as (x, baseline, whether
    /// it's centred).
    pub fn label_at(&self) -> (Point, bool) {
        if self.to_self {
            (
                Point::new(
                    self.from.x + SELF_WIDTH + LABEL_LIFT,
                    self.from.y + SELF_HEIGHT / 2.0 + LABEL_SIZE / 3.0,
                ),
                false,
            )
        } else {
            (
                Point::new(
                    f64::midpoint(self.from.x, self.to.x),
                    self.from.y - LABEL_LIFT,
                ),
                true,
            )
        }
    }
}

/// Everything below the participants.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    pub lifelines: Vec<Lifeline>,
    pub bars: Vec<Bar>,
    pub messages: Vec<Placed>,
}

impl Timeline {
    /// The lowest point drawn: the lifelines' ends.
    pub fn bottom(&self) -> f64 {
        self.lifelines.iter().map(|l| l.bottom).fold(0.0, f64::max)
    }

    pub fn message(&self, id: &ConnectionId) -> Option<&Placed> {
        self.messages.iter().find(|m| m.id == *id)
    }
}

/// The participant's colours.
pub fn colors_of(element: &Laid<Sequence>) -> FillColors {
    ParticipantColor::colors(element.el.props.color)
}

/// One participant's activations still open, innermost last, each its start and its place
/// in the stack.
#[derive(Default)]
struct Column {
    x: f64,
    open: Vec<f64>,
    /// The last height a message touched it at.
    last: f64,
}

impl Column {
    /// Where a message leaves or meets it: its line, or the side of its innermost activation
    /// facing `toward`.
    fn edge(&self, toward_right: bool) -> f64 {
        match self.open.len() {
            0 => self.x,
            depth => {
                let center = self.x + (depth - 1) as f64 * BAR_WIDTH / 2.0;
                if toward_right {
                    center + BAR_WIDTH / 2.0
                } else {
                    center - BAR_WIDTH / 2.0
                }
            }
        }
    }

    /// The bar a call starting at `start` makes, `depth` deep, ending at `end`.
    fn bar(&self, start: f64, depth: usize, end: f64) -> Rect {
        let center = self.x + depth as f64 * BAR_WIDTH / 2.0;
        Rect::new(
            center - BAR_WIDTH / 2.0,
            start,
            BAR_WIDTH,
            (end - start).max(BAR_MIN),
        )
    }
}

/// The diagram's lifelines, activations, and messages, with the participants at `layout`'s
/// places and the messages in the order of `messages`.
///
/// A call opens an activation on the participant it's sent to, which a return from that
/// participant closes; one never answered lasts until the last message to or from it.
/// Asynchronous messages and messages to the sender itself open none.
pub fn timeline(
    eng: &Engine<Sequence>,
    layout: &[Laid<Sequence>],
    messages: &[&Conn<Sequence>],
) -> Timeline {
    let participants: Vec<&Laid<Sequence>> = layout.iter().collect();
    let heads_bottom = participants
        .iter()
        .map(|p| head_bottom(eng, p))
        .fold(TOP, f64::max);
    let first = heads_bottom + FIRST_MESSAGE;
    let mut columns: HashMap<&ElementId, Column> = participants
        .iter()
        .map(|p| {
            (
                p.id(),
                Column {
                    x: p.rect.center().x,
                    open: Vec::new(),
                    last: first,
                },
            )
        })
        .collect();
    let mut bars: Vec<(ElementId, Rect)> = Vec::new();
    let mut placed = Vec::new();
    let mut y = first;
    let mut last = None;
    for c in messages {
        if !columns.contains_key(&c.source_id) || !columns.contains_key(&c.target_id) {
            continue;
        }
        let kind = c.props.kind;
        if c.source_id == c.target_id {
            let column = columns.get_mut(&c.source_id).expect("checked");
            let x = column.edge(true);
            column.last = y + SELF_HEIGHT;
            placed.push(place(
                c,
                Point::new(x, y),
                Point::new(x, y + SELF_HEIGHT),
                true,
            ));
            last = Some(y + SELF_HEIGHT);
            y += STEP + SELF_HEIGHT;
            continue;
        }
        let (source_x, target_x) = (columns[&c.source_id].x, columns[&c.target_id].x);
        let rightward = target_x > source_x;
        let from = columns[&c.source_id].edge(rightward);
        if kind == Some(MessageKind::Return) {
            let column = columns.get_mut(&c.source_id).expect("checked");
            if let Some(start) = column.open.pop() {
                let depth = column.open.len();
                bars.push((c.source_id.clone(), column.bar(start, depth, y)));
            }
        }
        if kind.is_none() {
            columns.get_mut(&c.target_id).expect("checked").open.push(y);
        }
        let to = columns[&c.target_id].edge(!rightward);
        for end in [&c.source_id, &c.target_id] {
            columns.get_mut(end).expect("checked").last = y;
        }
        placed.push(place(c, Point::new(from, y), Point::new(to, y), false));
        last = Some(y);
        y += STEP;
    }
    // Calls never answered last until the last message to or from their participant.
    for p in &participants {
        let column = columns.get_mut(p.id()).expect("every participant");
        while let Some(start) = column.open.pop() {
            let depth = column.open.len();
            bars.push((p.id().clone(), column.bar(start, depth, column.last)));
        }
    }
    let bars_bottom = bars.iter().map(|(_, r)| r.bottom()).fold(0.0, f64::max);
    let bottom = match last {
        Some(last) => (last + TAIL).max(bars_bottom + TAIL / 2.0),
        None => heads_bottom + EMPTY_LIFELINE,
    };
    let colors: HashMap<&ElementId, FillColors> = participants
        .iter()
        .map(|p| (p.id(), colors_of(p)))
        .collect();
    // Outer activations first, so inner ones are drawn over them.
    bars.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.1.y.total_cmp(&b.1.y)));
    Timeline {
        lifelines: participants
            .iter()
            .map(|p| Lifeline {
                id: p.id().clone(),
                x: p.rect.center().x,
                top: head_bottom(eng, p),
                bottom,
                colors: colors_of(p),
            })
            .collect(),
        bars: bars
            .into_iter()
            .map(|(id, rect)| Bar {
                colors: colors[&id],
                participant: id,
                rect,
            })
            .collect(),
        messages: placed,
    }
}

fn place(c: &Conn<Sequence>, from: Point, to: Point, to_self: bool) -> Placed {
    let SequenceMessage { kind, label } = c.props.clone();
    Placed {
        id: c.id.clone(),
        source_id: c.source_id.clone(),
        target_id: c.target_id.clone(),
        kind,
        label,
        from,
        to,
        to_self,
    }
}

/// The messages in time order, with `moving`'s message moved to where it's being dragged:
/// before the first other message below that height.
pub fn ordered<'a>(
    eng: &'a Engine<Sequence>,
    layout: &[Laid<Sequence>],
    moving: Option<(&ConnectionId, f64)>,
) -> Vec<&'a Conn<Sequence>> {
    let mut order: Vec<&Conn<Sequence>> = eng.connections().iter().collect();
    if let Some((id, y)) = moving {
        let index = index_at(eng, layout, id, y);
        if let Some(from) = order.iter().position(|c| c.id == *id) {
            let moved = order.remove(from);
            order.insert(index.min(order.len()), moved);
        }
    }
    order
}

/// Where in the order of messages the message `id` goes when dragged to height `y`: after
/// every other message above it.
pub fn index_at(
    eng: &Engine<Sequence>,
    layout: &[Laid<Sequence>],
    id: &ConnectionId,
    y: f64,
) -> usize {
    let all: Vec<&Conn<Sequence>> = eng.connections().iter().collect();
    timeline(eng, layout, &all)
        .messages
        .iter()
        .filter(|m| m.id != *id && m.y() < y)
        .count()
}

/// How far apart the centres of two participants side by side must be for the text of the
/// messages between them, and of the messages the left one sends itself, to fit.
pub fn room_for_text(eng: &Engine<Sequence>, left: &ElementId, right: &ElementId) -> f64 {
    let width = |c: &Conn<Sequence>| {
        c.props
            .label
            .as_deref()
            .map_or(0.0, |l| eng.measure().width(l, LABEL_SIZE, LABEL_WEIGHT))
    };
    eng.connections()
        .iter()
        .map(|c| {
            let between = (c.source_id == *left && c.target_id == *right)
                || (c.source_id == *right && c.target_id == *left);
            if between {
                width(c) + LABEL_MARGIN * 2.0 + BAR_WIDTH
            } else if c.source_id == *left && c.target_id == *left {
                SELF_WIDTH + LABEL_LIFT + width(c) + LABEL_MARGIN + BAR_WIDTH
            } else {
                0.0
            }
        })
        .fold(0.0, f64::max)
}
