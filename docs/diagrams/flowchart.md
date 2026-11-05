# Flowchart

**Type id:** `flowchart` · **Sub types:** none · **Has arrows:** yes (flows) · **Custom colours:** no (a fixed colour per kind) · **Line weight:** medium by default
**Rust crate:** `crates/diagrams/hiero-flowchart`; its shapes go in `hiero-core` next to C4's
**Reference:** ISO 5807 flowchart symbols (no prototype: new in the Rust app)

Maps out a process step by step: where it starts, the steps and decisions along the way, and
where it ends. A flowchart can be **free-form**, with its steps anywhere on the canvas, or use
**swimlanes**, with the steps in lanes that show who does each one. Both are the same type:
swimlanes are just a **Pool** element, which a flowchart can have none, one, or several of. A
free-form flowchart becomes a swimlane one when a pool is added, and goes back when it's
removed, with no change of type or sub type.

## Elements

All sizes are in canvas pixels and multiples of the 20 px grid; every default height is a
multiple of 40, so any two elements can be centred level with each other on the grid and an
arrow between them runs straight into a diamond's point. Every element is resizable and has
only a name, shown centred in its shape (14, semibold, wrapped to fit) unless the table says
otherwise. Arrows end on each shape's drawn outline, not its box.

### Steps

| Kind | Label | Default size | Drawn as | Arrows |
|---|---|---|---|---|
| `start` | Start | 160×40 | A pill (fully round ends), green | Out only |
| `end` | End | 160×40 | A pill with a heavier border, red | In only |
| `process` | Process | 160×80 | A box with slightly round corners | In and out |
| `decision` | Decision | 160×120 | A diamond, amber; its name is a question | In, and two or more out, each labelled |
| `input-output` | Input / Output | 160×80 | A parallelogram leaning right | In and out |
| `subprocess` | Subprocess | 160×80 | A box with a second line inside each side | In and out. Opens its sub diagram. |
| `document` | Document | 160×80 | A box with a wavy bottom | In and out |
| `data-store` | Data Store | 120×80 | A cylinder | Data arrows only (below) |
| `manual` | Manual Step | 160×80 | A trapezoid, wide at the top | In and out |
| `delay` | Delay | 160×80 | A D: flat left side, round right side | In and out |
| `connector` | Connector | 40×40 | A circle holding a short label, such as `A` | In and out |
| `note` | Note | 160×80 | A box with a folded top-right corner, pale yellow, its text (13, regular) top-left | A dotted line to one element |

A connector's name is at most 3 characters. Two connectors with the same name are the two
ends of one jump, so a long flow doesn't have to cross the canvas. A new connector takes the
name of the first connector without its other end yet, or else the first letter no connector
has, so they're added in pairs: A, A, B, B. Nothing checks that they pair up.

### Swimlanes

| Kind | Label | Default size | Can contain | Drawn as |
|---|---|---|---|---|
| `pool` | Pool | 1200×440 | lane | A square frame with a 40 px band across its top holding its name (top-left, 20 bold), in `#e6e9f0`; its lanes cover the rest |
| `lane` | Lane | fills its pool: 200 tall as a row, 300 wide as a column | every step, notes, and connectors | A square band of the pool with its name in its top-left corner, over a 40 px header its steps stay below. Lanes alternate between `#ffffff` and `#f3f5f8`. |

Names read across, never turned, so a lane's header runs across its top whichever way the
lanes run.

The palette offers the steps, then under a **Swimlanes** heading **Pool**, which adds a pool
holding two lanes, *Lane 1* and *Lane 2*, and **Lane**, which goes after the last lane of the
pool it's dropped on (or of the first pool, when added with a click), named after its place,
such as *Lane 3*. A lane added where there's no pool adds a pool with its two lanes instead.
An agent adds a lane with the pool as its `parent_id`.

## Layout rules

These are the type's `rules` hooks, UI-free and unit-tested. Lanes in a pool behave like zones
in a physical diagram's network: `lanes.rs` began as a copy of that code, kept in this crate on
purpose so the two can differ (lanes can run either way, for one).

1. **New diagram:** a **Start** at (80, 80) and an **End** to its right, at (480, 80), with no
   arrow between them. No pool. A sub diagram added from any element starts the same way.
2. **Lanes fill their pool:** they're stacked from the pool's header, in order, across its
   full width (or height, for vertical lanes), with the last lane taking any room left. What a
   lane holds moves with it.
3. **Orientation:** a pool's lanes run **horizontally** (rows, flow going right) by default,
   or **vertically** (columns, flow going down). Changing it turns the pool and its lanes, and
   keeps each step in its lane at the same place along the flow: how far along the lane it
   was becomes how far down the column it is, and the other way round. The lanes keep their
   order, and the pool is resized round them. It's one undo step.
4. **Resizing:** resizing a pool shares its new size among its lanes in proportion, snapped to
   the grid. Resizing a lane resizes the pool to match. A pool can't shrink past what its lanes
   need to hold their steps.
5. **Moving lanes:** a lane stays in its pool and only moves along the stack. Dragging it takes
   the place where its middle lands; the arrow keys swap it with its neighbour.
6. **Steps in lanes:** a step dropped in a lane belongs to it and stays within it. Dragging a
   step across a lane line moves it to the other lane. A step dropped outside every pool
   belongs to none, so free-form steps and pools can share a diagram.
7. **Notes and connectors** can be in a lane, or sit on the canvas across a lane line.
8. **Removing** a lane gives its steps to the lane before it (or after it, for the first lane),
   where they keep their place along the lane, and the lane grows to hold them if it must;
   removing a pool's last lane removes the pool. Removing a pool removes its lanes too and
   keeps their steps on the diagram, free-form, with their arrows.
9. **Arrow keys** on a lane move it earlier or later in its pool (up and down for rows, left
   and right for columns); the other two do nothing. On a step against its lane's edge they
   move it across, into the next lane or out of the pool.
10. **No arrows** go to or from a pool or a lane.

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters (connector: at most 3) |
| Orientation | pool | Radio: *Horizontal* or *Vertical*. Stored as `orientation`: `"vertical"`, left out for horizontal. |
| Lane | steps, note, connector | The lane it's in, or none |
| Arrows list and Add arrow to | steps | Same as the other types |

## Arrows

Arrows are straight lines between the facing sides of what they join, with the shared routing
(straight across where the sides overlap), and ignore lane lines: an arrow that crosses one is
a handoff from one role to another. Their look follows from what they join, so there's no
style to pick:

| Joins | Look | Meaning |
|---|---|---|
| Two steps | Solid line, filled head | Flow: what happens next |
| A step and a data store or document | Dashed line, filled head | Data read or written; drawn either way |
| A note and anything | Dotted line, no head | The note is about that element |

- **Label:** every arrow has an optional label of at most 40 characters, on a background box
  on the middle of the line. A new arrow out of a decision is labelled *Yes*, then *No*, then
  nothing; they can be changed.
- **Refused:** an arrow into a Start or a note, out of an End, from a step to itself, to or from
  a pool or lane, or from a note to more than one element. A data store's arrows out go only to
  steps that do something (process, decision, input / output, subprocess, manual step, delay).
  A decision's arrow to a data store or document isn't one of its ways out, so it isn't
  labelled Yes or No.
- **Not refused** (a flowchart can be half drawn): a decision with fewer than two arrows out,
  arrows out of a decision without labels, a step no arrow reaches.

## Colours

Fixed per kind, the same in the editor and the exports. Text is the usual dark text; the
fills come from `hiero-core::palette` where a match exists.

| Kinds | Fill | Border |
|---|---|---|
| Start | `#d5e8d4` | `#356b24` |
| End | `#f8cecc` | `#9c3530` (2× line weight) |
| Decision | `#fff2cc` | `#9a7210` |
| Process, subprocess, manual step, delay | `#dae8fc` | `#2d5f9e` |
| Input / output | `#e1d5e7` | `#6c4fb3` |
| Document, data store | `#d9eef1` | `#22727d` |
| Connector | `#ffffff` | `#333333` |
| Note | `#fbf8e3` | `#8f8640` |
| Pool, lanes | `#ffffff` and `#f3f5f8`, headers `#e6e9f0` | `#333333` |
| Arrows | – | `#333333` |

## Sub diagrams

A subprocess is the step meant to open a flowchart of its own: **Add diagram** on it starts a
new flowchart named after it, with the usual Start and End, and links the subprocess to it.
Any element can link to a diagram as in the other types, and shows the shared mark when it
does. On every step it sits off the top-right corner, up and to the right, clear of the
outline, so it never touches a slant, a curve, or a subprocess's inner line. A pool or lane has
it inside its top-right corner, as containers do in the other types.

## Export

- **SVG:** the shared scene.
- **draw.io:** start and end are `rounded=1;arcSize=50` boxes; decision `rhombus`; input /
  output `shape=parallelogram`; subprocess `shape=process`; document `shape=document`; data
  store `shape=cylinder3`; manual step `shape=trapezoid` turned upside down; delay
  `shape=delay`; connector `ellipse`; note `shape=note`. Arrows are attached connectors with
  their label near the start, dashed for data, and dotted with no head for a note. A pool is a draw.io
  container holding its lanes as containers, each step a child of its lane, so they move
  together in draw.io too.

## Stored data

```jsonc
// element props
{ "orientation": "vertical" }                        // pool; omit for horizontal (steps have none)
// connection props
{ "label": "Yes" }                                   // omit when blank
```

Rust shape: `Element<FlowchartKind, FlowchartProps>` and `Connection<FlowchartArrow { label }>`.
A lane's position in its pool is its order in the elements, as with zones.

## Checklist

- [x] Crate `hiero-flowchart` with the type registered in `hiero-registry` and the app (store migration 8)
- [x] Steps: start, end, process, decision, input / output, subprocess, document, data store, manual step, delay, connector, note
- [x] Arrows end on each shape's outline
- [x] Arrow looks follow what they join; labels; Yes/No defaults out of a decision
- [x] Refused arrows (into a start, out of an end, and the rest)
- [x] Connectors named in pairs
- [x] Pools and lanes: lanes fill the pool, resize in proportion, reorder, hold their steps
- [x] Vertical lanes, and turning a pool keeps each step's place along the flow
- [x] Free-form steps and pools on the same diagram
- [x] Add diagram on a subprocess (shared: any element)
- [x] Line thickness, canvas size, view mode, sub diagrams and links, cut/copy/paste (shared)
- [x] SVG and draw.io export
- [x] MCP: `list_diagram_types`, the `arch-hiero://types/flowchart` resource, `edit_diagram`

## Later

- **Elbow arrows:** arrows that turn corners, such as a decision's *No* going down and back.
  Every type's arrows are straight today.
- **Open heads on data arrows.**
- **Parallel fork / join:** a thick bar for "do these at the same time".
- **Group:** a dashed frame round related steps, such as a phase.
- **Checks:** flag a decision with one way out, or a step nothing reaches, without refusing it.
