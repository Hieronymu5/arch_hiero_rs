# C4 - System Context diagram

**Type id:** `c4-context` · **Has arrows:** yes (relationships) · **Custom colours:** no (fixed scheme) · **Line weight:** medium by default
**Rust crate:** `crates/diagrams/hiero-c4` (shared with the other C4 types)
**Reference:** <https://c4model.com/diagrams/system-context> (no prototype: new in the Rust app)

The first level of the C4 model: one **software system** in scope, the **people** who use it,
and the other software systems it works with, joined by **relationships** that say what each
does with the other. It leaves technology out, so anyone can read it.

## Elements

All sizes are in canvas pixels and multiples of the 20 px grid.

| Kind | Label | Default size | Has a description | Arrows to | Notes |
|---|---|---|---|---|---|
| `person` | Person | 120×80 | no | person, software-system | A user, actor, role, or persona. Drawn as the actor figure (as on conceptual diagrams), filled blue, with the name underneath. |
| `software-system` | Software System | 240×140 | yes | person, software-system | A box with round corners, its name in bold, `[Software System]`, and its description inside. Resizable. Coloured by its **Scope**. |

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters |
| Description | software-system | At most 500 characters, shown inside under the type, keeping line breaks; as many lines as fit |
| Scope | software-system | Radio: *In scope* (blue), *Internal* (orange: another of the organisation's own systems), *External* (red: outside the organisation). Stored as `scope`: `"internal"` or `"external"`, left out in scope. |
| Arrows list, Add arrow to | all | As on the other types |

## Colours

Fixed, taken from the other diagram types:

| Scope | Fill | Border |
|---|---|---|
| In scope (blue, as a physical diagram's blue zone) | `#dae8fc` | `#2d5f9e` |
| Internal (orange) | `#ffe6cc` | `#b46504` |
| External (red, as a physical diagram's red zone) | `#f8cecc` | `#9c3530` |

Text is the usual dark text. People are filled `#dae8fc` with a `#1e1e1e` outline.

## Relationships (arrows)

- **Drawing:** drag from the round handle on the selected element to another element, or use
  *Add arrow to*. Routing is the shared side-to-side routing: straight where the two elements'
  facing sides overlap. An arrow at a person's bottom ends just under their name.
- **Look:** a black (`#1e1e1e`) dashed line, 2 px, with a filled head. Its description sits
  centred on the middle of the line, wrapped to 180 px, over a white halo.
- **Properties:** *Description* (`description`, at most 150 characters), such as "Views account
  balances and makes payments using". System context diagrams have no technology on
  relationships.

Removing an element removes its relationships.

## Export

- **SVG:** the shared scene.
- **draw.io:** software systems are rounded boxes with HTML text (bold name, the type at 11 px,
  the description at 13 px); people use the actor stencil. Relationships are attached dashed
  connectors with their text as the edge label on the middle, on a white background.

## Stored data

```jsonc
// element props
{ "scope": "external" }            // software-system: "internal" | "external"; omit in scope
// connection props
{ "description": "Sends e-mails to" }
```

Rust shape: `Element<ContextKind, C4Props>` and `Connection<C4Arrow>`, where `C4Props { scope,
technology }` and `C4Arrow { description, technology }` are shared by the three C4 types.

## Checklist

- [x] Palette: person and software system. Click adds at a free spot, drag drops where released.
- [x] Software system scope: blue, orange, or red
- [x] Relationships: dashed black arrows with their description on the middle
- [x] Line thickness, canvas size, view mode, sub diagrams and links, cut/copy/paste (shared)
- [x] SVG and draw.io export
- [x] MCP: `list_diagram_types`, `create_diagram`, `edit_diagram` with the props above
