# Sequence

**Type id:** `sequence` · **Sub types:** none · **Has arrows:** yes (messages) · **Custom colours:** no (each participant picks one of four) · **Line weight:** medium by default
**Rust crate:** `crates/diagrams/hiero-sequence`
**Reference:** UML sequence diagrams (no prototype: new in the Rust app)

Shows how participants work together over time: they stand in a **row along the top**, each
with a **lifeline** running down from it, and the **messages** they send each other go across
between the lifelines, the first at the top and each later one below it. The participants are
the conceptual diagram's actors, applications, and components, drawn the same way.

## Participants

| Kind | Label | Default size | Drawn as | Props |
|---|---|---|---|---|
| `actor` | Actor | 120×80 | A person (or a bot) with its name under it | `color`, `agent: true` for a bot |
| `application` | Application | 160×80, resizable in width | A box with its name in bold, and any description under it | `color` |
| `component` | Component | 160×80, resizable in width | A box as an application is, or a database or storage figure (120×80) with its name under it | `color`, `componentType: "database" \| "storage"` |

Every participant has a **colour**: blue (the default, left out), `green`, `red`, or
`yellow`, from the shared palette (`hiero_core::palette`). The participant is filled with its
light shade and bordered with its dark one, and its lifeline and activations take the same
colour.

Every participant has a **name**: required, at most 100 characters, drawn in bold.
Applications and components also have an optional **description** (the element's
`description`, at most 500 characters, left out when blank), drawn under the name in regular
13 px type, keeping its line breaks and cut with "…" after three lines. A box is 80 px tall,
or taller when its name (up to two lines) and description need it, with 10 px above and below
them, rounded up to the next 20 px; it can't be made taller or shorter by hand. A component
drawn as a database or storage, like an actor, shows only its name, so its description is
kept but not drawn, and its properties panel offers none. The draw.io file has the name in
`<b>` and the description in a 13 px `<span>` under it.

A new diagram starts with an actor named *User* and an application named *Application*.

### The row

- Participants stand in one row: their tops are at y 40, and the first starts at x 40.
- Their order is the order of their centres. Dragging one left or right and letting it go
  between two others puts it there; it can't leave the row, so its y is ignored. The arrow
  keys (or **Move left** and **Move right** in its properties) swap it with its neighbour;
  up and down do nothing.
- Each stands at least 60 px from the one before. Where the messages between two neighbours,
  or the messages the left one sends itself, have text too wide for that, they're moved apart
  until it fits, so the row is arranged again whenever a message changes.
- A participant added with a click goes at the end of the row; one dropped from the palette
  goes where it's dropped.
- Removing a participant removes its messages, and the row closes up.

## Messages

Messages are the type's arrows. Any participant can send any number of messages to any
other, or to itself. The order of a diagram's messages (the order of its connections, kept
in the database's `ord` column and in files) is time: the first is drawn at the top.

| `kind` | Name | Drawn as |
|---|---|---|
| left out | Call | A solid line with a filled head: the sender waits for the answer |
| `async` | Asynchronous | A solid line with an open head: the sender carries on |
| `return` | Return | A dashed line with an open head: the answer to a call, back to its caller |

A message's `label` (≤ 100 characters) is shown centred above its line, over a gap in any
lifeline it crosses: the call, such as `getOrder(id)`, or for a return what comes back, such
as `Order`. A message a participant sends itself goes out 40 px to the right of its lifeline,
down 30 px, and back, with its text to the right of the loop.

- A new message goes last. A message drawn straight back after a call (from the call's target
  to its sender) starts as a return; any other starts as a call.
- **Add return** on a call adds its return right after it.
- **Reordering:** drag a message up or down and let it go: it goes before the first other
  message below that height. Or select it and press the up and down arrow keys, or use **Move
  up** and **Move down** in its properties. An agent uses `move_connection {id, index}`.
- A message can be dropped anywhere on a lifeline, not only on its participant.

### Spacing

From the bottom of the tallest participant (a figure's name may go to a second line) to the
first message is 60 px, and each message is 50 px below the one before; a message to itself
takes 30 px more. The lifelines run 40 px past the last message, or 160 px when there are
none.

### Activations

A call opens an **activation** on its receiver: a 12 px bar on its lifeline, in its colour,
from the call down to the receiver's next return, which closes it. A call that's never
answered lasts until the last message to or from its receiver. Activations nest: a call
received while one is open draws its bar 6 px to the right, over the first. Messages start and
end on the side of the innermost open activation facing the other participant. Asynchronous
messages, and messages to the sender itself, open none. Every bar is at least 20 px long.

## Exports

The SVG export draws the scene. The draw.io file has the participants as cells (figures with
the same stencils as the conceptual diagram's), the lifelines as dashed edges between points,
the activations as boxes, and the messages as edges between points on the lifelines (`block`
heads for calls, `open` for the others, returns dashed), with loops for messages to the sender.

## Checklist

- [x] Actors, applications, and components in a row, coloured blue, green, red, or yellow with
      their lifelines
- [x] Bold names, and optional descriptions on applications and components, their boxes
      growing to fit
- [x] Calls, asynchronous messages, and returns, with text, and messages to the sender
- [x] Reorder participants by dragging, arrow keys, and buttons
- [x] Reorder messages by dragging, arrow keys, and buttons; agents with `move_connection`
- [x] Activations
- [x] SVG and draw.io export; MCP server
- [ ] Notes, and fragments (`alt`, `loop`, `opt`)
- [ ] Creating and destroying participants partway down
