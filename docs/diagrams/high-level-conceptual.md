# Conceptual - High Level diagram

**Type id:** `high-level-conceptual` · **Has arrows:** yes (wide block arrows) · **Custom colours:** no (five fixed colours per element) · **Line weight:** medium by default
**Rust crate:** `crates/diagrams/hiero-high-level`
**Reference:** none (no prototype: new in the Rust app)

A landscape at a glance, as an enterprise architecture overview or a layered architecture
diagram draws one: applications and the people who use them in groups, groups inside other
groups, and wide arrows
between them only where they're needed. A group can also be empty, as a boundary for something
shown without its parts. Every element has its own colour and font size, and every
application and group a solid or dashed border.

## Elements

The palette offers **Application**, **Group**, and **Actors**. All sizes are in canvas pixels
and multiples of the 20 px grid. All are resizable and have a name; an application and actors
can also have a description.

| Kind | Label | Default size | Can contain | Drawn as |
|---|---|---|---|---|
| `application` | Application | 160×80 | – | A box with round corners (radius 8), its name wrapped in its middle, in bold, with any description under it (2 px below) in regular weight at 85% of the name's size, rounded, at least 8 px. The description keeps its line breaks and is cut with “…” where the box has no more room. |
| `group` | Group | 400×240 | application, group, actors | A box with round corners (radius 8), drawn behind what it holds. Its name, in bold, sits on one line centred across its top while it holds something, and wrapped in its middle while it's empty. |
| `actors` | Actors | 160×120 | – | People, such as stakeholders or a team: three person figures (48×46 each) centred 4 px below its top, two side by side at the back, 32 px apart, and one in front, 10 px lower and centred between them, covering most of each, in its colour's fill and border. Its name, in bold, is wrapped under them, 6 px below, with any description under it as an application's is. It has no box or border. |

A group grows to hold what's in it, with 20 px of padding round it and its header above. The
header fits one line of the group's name, with 8 px above and below it, rounded up to the grid
and at least 40 px: 40 for names up to 20 px, 60 up to 36 px, and 80 above. An element can't be
put inside itself or anything it holds.

Groups not in another group are drawn behind applications not in one.

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters |
| Description | application, actors | Optional, multi-line, at most 500 characters. Stored as the element's `description`, left out when blank. |
| Colour | all | Radio: *Blue*, *Green*, *Red*, *Yellow*, or *Grey*. Stored as `color`, left out for the kind's own: blue for an application or actors, grey for a group. |
| Font size | all | A select of 10, 11, 12, 13, 14, 16, 18, 20, 24, 28, and 32 px. Stored as `fontSize`, left out for the kind's own: 14 for an application or actors, 16 for a group. Agents and files can set any whole size from 8 to 48. |
| Border | application, group | Radio: *Solid* or *Dashed* (8 on, 4 off). Stored as `dashed: true`, left out for solid. |
| Parent | all | The group it sits in, or none |

The diagram's **line thickness** (thin, medium, or thick, in Edit Details, as for every type)
sets the width of every border and of the arrows' outlines.

## Colours

The colours are fixed: the same blue, green, red, and yellow as the other types, and a grey.
Text is the usual dark text.

| Colour | Fill | Border |
|---|---|---|
| Blue | `#dae8fc` | `#2d5f9e` |
| Green | `#d5e8d4` | `#356b24` |
| Red | `#f8cecc` | `#9c3530` |
| Yellow | `#fff2cc` | `#9a7210` |
| Grey | `#f5f5f5` | `#666666` |

## Arrows

- **Drawing:** drag from the round handle on the selected element to another, or use *Add
  arrow to*. Arrows join any two elements, except a group and anything inside it.
- **Routing:** the shared side-to-side routing: straight where the two elements' facing sides
  overlap, and direct otherwise. Each end stops 6 px short of its element.
- **Look:** a wide block arrow, filled `#d9d9d9` with a `#8c8c8c` outline: a 16 px shaft and a
  head 36 px wide and 18 px long at the target, or at both ends.

| Field | Rule |
|---|---|
| Both ways (`twoWay`) | Checkbox: a head at each end. Left out for one. |

Removing an element removes its arrows. Removing a group keeps what it holds on the diagram,
outside it.

## Export

- **SVG:** the scene.
- **draw.io:** groups are rounded draw.io containers, with what they hold as their children;
  actors a borderless container holding three person-stencil cells (each the 48×46 figure,
  its proportions kept), with their text under them;
  applications rounded boxes, with the name in bold and any description under it in a regular
  `<span>` at its own size. Each keeps its colours, border, font size, and where its name
  sits. Arrows are draw.io `flexArrow` edges attached to their elements at the same points, with
  the same shaft and heads: their style numbers take off what draw.io adds for the line width,
  and shorten the heads of a short arrow as the scene does, so two heads never meet.

## Stored data

```jsonc
// element props, for every kind (but dashed, which actors don't have)
{ "color": "green", "fontSize": 20, "dashed": true }
// color: "blue" | "green" | "red" | "yellow" | "grey", omitted for the kind's own
// (blue for an application or actors, grey for a group); fontSize omitted for 14 or 16;
// dashed omitted for solid
// connection props
{ "twoWay": true }                                // omit for one head, at the target
```

Rust shape: `Element<HighLevelKind, HighLevelProps>` and `Connection<BlockArrow>`.

## Checklist

- [x] Palette: Application, Group
- [x] Groups hold applications and other groups, and can be empty
- [x] A group's name at its top, or in its middle while it's empty
- [x] A colour on every element: blue, green, red, yellow, or grey
- [x] A font size on every element, a group's header growing to fit it
- [x] Actors: three people, with their name and an optional description
- [x] Bold names, and an optional description on applications and actors
- [x] Solid or dashed borders
- [x] Wide arrows, one way or both
- [x] Line thickness, canvas size, view mode, sub diagrams and links, cut/copy/paste (shared)
- [x] SVG and draw.io export
- [x] MCP: `list_diagram_types`, `create_diagram`, `edit_diagram`
