# Conceptual diagram

**Type id:** `conceptual` · **Has arrows:** yes · **Custom colours:** yes (8)
**Rust crate:** `crates/diagrams/hiero-conceptual`
**Prototype source:** `arch_hiero/src/app/diagrams/conceptual/`, `model/conceptual-diagram.ts`

Shows the business view of a system landscape: actors, domains, the applications in them, the
components and data inside those applications, and the arrows between them.

## Elements

All sizes are in canvas pixels and are multiples of the 20 px grid. *Layer* sets the drawing
order for elements that aren't inside a container (lower layers are drawn first, behind higher
ones). Elements inside a container are always drawn on top of it.

| Kind | Label | Default size | Fill colour | Can contain | Arrows to | Layer | Notes |
|---|---|---|---|---|---|---|---|
| `actor` | Actor | 120×80 | applications (person) / connectors (bot) | – | application, component, domain, job | 3 | A person figure with the name underneath. The **Bot** option draws it as a bot. |
| `domain` | Domain | 480×180 | domains | application | – | 0 | A filled `region` with its name as a header. Resizable. |
| `application` | Application | 200×100 | applications | component, process, entity, file | application, component, job | 1 | Has a description. The container look is `box`: it looks like a plain box until it holds elements, then its name and description move to the top. Header is 60. Resizable. Has a **Cloud hosted** option. |
| `component` | Component | 160×60 (120×80 as a figure) | applications (data for database/storage) | process, entity, file | application, component, job | 2 | Container look is `box`. The **Type** option is Default, Database (cylinder) or Storage (a stack of drives). A database or storage component is a figure with its name underneath, and can't contain anything. |
| `process` | Process | 140×40 | processes (dark blue, light text) | – | – | 3 | |
| `job` | Job | 160×40 | connectors | – | application, component | 3 | Something that runs on a schedule. |
| `entity` | Entity | 140×40 | data | – | – | 3 | |
| `file` | File | 120×80 | data | – | – | 3 | Data kept as a file, such as an export. Placed as an entity is, but drawn as a figure: an upright page with its upper-left corner folded down and five lines of text, with the name underneath. |
| `section` | Section | 480×40 | – | – | – | 3 | A horizontal line with short end ticks and the name centred on it, in the *section names* colour. It names the part of the diagram below it. Resizes across only, from handles on its two ends. |

Containers grow to fit what they hold: a 40 px header (60 for an application) and 20 px of
padding around their children.

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters |
| Description | application | At most 500 characters, shown under the name |
| Bot | actor | Checkbox, stored as `agent: true` |
| Cloud hosted | application | Checkbox. The application is drawn as a cloud with its text inside. |
| Type | component | Radio: Default / Database / Storage, stored as `componentType` (unset means Default) |
| Parent (Domain, or "Application or component") | anything that can be held | A select of the containers that can hold this kind, named "Name (Kind)" when more than one kind of container can hold it. Choosing a parent moves the element to a free spot inside it. Choosing none moves it out onto the canvas. |
| Arrows list | kinds with `connectsTo`, and their targets | Lists each arrow to or from the element. Selecting one opens the arrow's own properties. |
| Add arrow to | kinds with `connectsTo` | A select of the valid targets this element doesn't already point to. It's the keyboard alternative to dragging from the connect handle. |

## Arrows

- **Drawing:** drag from the round handle on the selected element to a valid target (see *Arrows
  to* in the table), or use *Add arrow to* in the properties panel.
- **Routing (shared code):** an arrow runs between the sides of its two elements that face each
  other. Arrows that meet the same side are spread evenly along it at positions 1/(n+1), and
  sorted by where their other end is, so they don't cross. An arrow from an element to itself
  isn't drawn.
- **Straightening (conceptual only):** drag an arrow sideways, or select it and use the arrow
  keys, to run it straight. Its position is stored as `manualOffset`, measured from the source's
  top edge (left/right arrows) or left edge (top/bottom arrows), and kept within the range where
  the two facing sides overlap, less a 10 px margin. The **Straighten** button centres it in
  that range. **Place automatically** clears `manualOffset`.

Arrow properties:

| Field | Rule | Drawn as |
|---|---|---|
| Text (`label`) | At most 100 characters | Text beside the start of the arrow |
| Type (`mode`) | Automated (solid) or Manual (dashed). Unset means automated. | Line style |
| Bi-directional | Checkbox | A small return arrow next to the start |
| Connector | At most 20 characters, e.g. API, REST, Web | A small box where the arrow meets the target |
| Transformation | At most 40 characters, e.g. "Map fields" | A box on the middle of the line |

Removing an element removes its arrows.

## Colours

Each diagram can override any of these. Only colours that differ from the defaults are stored.

| Key | Default | Used for |
|---|---|---|
| `domains` | `#f5f5f5` | Domain fill |
| `applications` | `#d0e6fb` | Applications, components, and people |
| `connectors` | `#cfd4da` | Connector and transformation boxes, jobs, and bots |
| `data` | `#ffe599` | Entities, files, databases, and storage |
| `processes` | `#1466b3` | Processes |
| `borders` | `#1e1e1e` | Element, figure, and arrow-box borders |
| `lines` | `#0066cc` | Arrow lines and heads |
| `sectionNames` | `#0b3d91` | Section names |

Text on a fill is black or white, whichever has more contrast (WCAG). This uses
`hiero_core::color::readable_text`.

## Figures (shared with other types)

- **Actor:** a person, or a bot. Physical diagrams use the same person for users.
- **Database cylinder:** physical diagrams use it too.
- **Storage stack**
- **File:** a page with a folded corner and lines of text.
- **Cloud outline:** draw.io's cloud shape, moved down 5% so it touches the bottom edge, with
  text inset 16%.

These live in `hiero-core::figures` because more than one diagram type uses them.

## Interaction specific to this type

Everything else (palette, select, multi-select, move, snap, resize, delete, pan and zoom) is the
shared editor engine described in [the plan](../implementation-plan.md#shared-editor-engine).

- Dropping an application on a domain puts it inside the domain. Dropping a component on an
  application puts it inside the application. Dropping a process, entity, or file on an
  application or component puts it inside that.
- An application or component that holds elements shows its name and description at the top.

## Export

- **SVG:** uses the shared scene. Each element maps to an `ExportLook`: box, cloud, container,
  section, actor, database, storage, or file.
- **draw.io:** uses the shared writer. Containers become draw.io containers and arrows become
  attached connectors. Connector and transformation boxes become labelled cells. The heading
  (diagram name and description) goes in the top-left corner.

## Stored data (`elements.props` / `connections.props` JSON)

```jsonc
// element props, per kind (the common columns are in the elements table)
{ "description": "…" }          // application
{ "agent": true }                // actor
{ "cloudHosted": true }          // application
{ "componentType": "database" }  // component: "database" | "storage"
// connection props
{ "label": "…", "mode": "manual", "bidirectional": true,
  "connector": "API", "transformation": "Map fields", "manualOffset": 40 }
```

Rust shape: `Element<ConceptualKind, ConceptualProps>`, where `ConceptualKind` is a fieldless enum serialised in kebab-case (`"kind": "application"`) and `ConceptualProps { agent, component_type, cloud_hosted }` is flattened into the same JSON object in camelCase. The description is a common field of every element. Arrows are `Connection<ConceptualArrow>`. The JSON is the same as the prototype's.

## Parity checklist

- [x] Palette with all 8 kinds. Click adds the element at a free spot. Drag drops it where released.
- [x] Containment rules and the container `box` look for application and component
- [x] Resize domain and application from two corners, and section across only
- [x] Bot, cloud-hosted, and component-type rendering
- [x] Arrow draw, route, select, straighten, and place automatically. Arrow keys nudge the selected arrow.
- [x] Arrow text, mode, bidirectional, connector, and transformation
- [x] Per-diagram colours with "Use the default colors"
- [ ] SVG and draw.io export that match the prototype (snapshot tests) — *the writers are line-by-line ports of the prototype's and are unit-tested, but haven't yet been compared with files the prototype exported*
