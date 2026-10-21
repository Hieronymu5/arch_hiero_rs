# Deployed component diagram

**Type id:** `deployed-component` · **Has arrows:** no · **Custom colours:** no (fixed scheme)
**Rust crate:** `crates/diagrams/hiero-deployed`
**Prototype source:** `arch_hiero/src/app/diagrams/deployed-component/`,
`model/deployed-component-diagram.ts`, `model/tech-stack.ts`,
`utils/deployed-component-{layout,svg,drawio}.ts`

Shows how a system is deployed: horizontal **layers** (such as Web, API, Data), each holding its
**components** side by side. Each component shows how it's reached and secured, a bulleted
description, its tech stack logos, and its instance count.

## Elements

| Kind | Label | Can contain | Notes |
|---|---|---|---|
| `layer` | Layer | component | Header is 50 (40 with no characteristics), padding 20. Sized by its contents, so it can't be resized. |
| `component` | Component | – | Has a description. Sized by its description. |

This type has no free placement. Everything is **arranged** automatically:

- Layers stack from the top-left (40,40) with a 20 px gap. They're all as wide as the widest one
  needs, and at least 860 px wide.
- The order of layers is by vertical middle, and the order of components within a layer is by
  horizontal middle. Ties keep the list order, so the list order must be stored.
- Components in a layer sit side by side with 20 px gaps, all as tall as the tallest.
- A component outside any layer goes into the nearest layer. With no layers at all, components
  stay where they are.
- **Component size:** 260 px per description column. A description goes to two columns when it
  has more than 4 bullets, and the first column takes the extra one if the count is odd. Height
  = protocol-bar inset (10) + name (40) + 2×10 padding + rows×20 (at least 4 rows) + footer (40).

## Component anatomy (top to bottom)

1. **Protocol bar:** 20 px high, raised 10 px so it straddles the top edge. The protocols are
   side by side, separated by slanted dividers.
2. **Name:** 40 px, with a divider under it.
3. **Description:** one bullet per non-empty line, in 1 or 2 columns.
4. **Footer:** 40 px, with a divider over it.
   - Tech-stack logos: 24 px boxes holding an 18 px logo, with 6 px gaps. Only as many as fit.
   - The instance count, in an oval against the right edge, only when it's more than 1.

**Layer anatomy:** a characteristics bar straddling the top edge (only when the layer has
characteristics), then the layer name.

## Colours (fixed in the editor and in exports, regardless of app theme)

| Use | Colour |
|---|---|
| Layer fill / border | `#e6e6e6` / `#b3b3b3` |
| Bars and the instance oval | `#f5f5f5` |
| Characteristics bar border | `#666666` |
| Protocol bar border and separators | `#1b1f24` |
| Logo boxes | `#ffffff` |
| Text | `#1b1f24` |

Each component's own colours depend on its **Type**:

| Type (`dependency`) | Label | Fill | Border |
|---|---|---|---|
| (unset) | Core Component | `#d5e8d4` | `#82b366` |
| `internal` | Internal Dependency | `#fff2cc` | `#d6b656` |
| `external` | External Dependency | `#f8cecc` | `#b85450` |

These fills are also used by physical zone colours, so the table lives in
`hiero-core::palette`.

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | both | Required, at most 100 characters |
| Characteristics, separated by commas | layer | At most 200 characters, e.g. "Internet facing, PCI DSS". Stored as `string[]`. |
| Layer | component | A select of the layers |
| Type | component | Core Component / Internal Dependency / External Dependency |
| Protocols and security, separated by commas | component | At most 100 characters, e.g. "HTTPS, JWT, XSRF". Stored as `string[]`. |
| Description, a bullet on each line | component | At most 2000 characters |
| Instance count | component | Integer ≥ 1 |
| Tech stack | component | Checkboxes grouped by category (Cloud, Operating System, Database, Middleware, Frameworks, Languages, Browsers). Groups are collapsed until opened, and each group header shows how many are chosen. Stored as an array of tech ids. |

## Tech stack catalogue

The prototype has 92 technologies, each with an id, label, category, and 18×18 PNG logo stored as
a base64 data URL in `model/tech-stack.ts`. For Rust:

- A one-off script extracts them to `crates/diagrams/hiero-deployed/assets/tech/<id>.png`, plus a
  generated `tech_stack.rs` table (`id`, `label`, `category`, `include_bytes!`).
- The editor turns each logo into an egui texture once. Exports embed the PNG as a data URL, as
  the prototype does.
- The ids must match the prototype's exactly, so imported prototype files keep their logos.

## Interaction specific to this type

- Adding from the palette puts a component after the selected component, or at the end of the
  selected layer. A layer goes at the bottom.
- **Layers:** drag one up or down, or use the up and down arrow keys, to reorder it.
- **Components:** drag one to its new position. Left and right arrow keys swap it with its
  neighbour. Up and down arrow keys move it to the layer above or below, as close to its current
  position as it can go.
- Removing a layer removes its components.
- Multi-select can drag elements together and delete them together.
- The accessible description reads like "API, 2 of 3 in Web" or "Web, layer 1 of 2".

Implementation note: reordering is done by rewriting positions with `in_order(layers,
components)`, then re-running `arrange`. Order is derived from position, so storing positions
plus list order is enough.

## Export

This type has its own SVG and draw.io writers (`deployed_components_to_svg`,
`deployed_components_to_drawio`), because its anatomy doesn't match the shared element looks.
Text is fitted with `fit_text`, which truncates with an ellipsis.

## Stored data

```jsonc
// layer props
{ "characteristics": ["Internet facing", "PCI DSS"] }
// component props
{ "description": "Line 1\nLine 2", "protocols": ["HTTPS", "JWT"],
  "techStack": ["java", "docker"], "dependency": "external", "instanceCount": 3 }
```

There are no connections. The prototype leaves the `connections` key unset for this type; the
Rust version stores none and writes no `connections` key on export.

Rust shape: `Element<DeployedKind, DeployedProps>`, with `DeployedProps { characteristics, protocols, tech_stack, dependency, instance_count }` flattened beside the fieldless kind enum. There are no arrows: no kind connects to anything, so the arrow tools never appear, and the registry and MCP server refuse arrows for this type.

## Parity checklist

- [x] Automatic arrangement of layers and components, including nearest-layer adoption
- [x] Component anatomy: protocol bar, bullet columns, logos, and instance oval
- [x] Characteristics bar on layers
- [x] Keyboard and drag reordering, and moving components between layers
- [x] 92-item tech stack picker with grouped, collapsible categories
- [x] Dependency colours
- [ ] Custom SVG and draw.io export that match the prototype — *the writers are line-by-line ports of the prototype's and are unit-tested, but haven't yet been compared with files the prototype exported*
