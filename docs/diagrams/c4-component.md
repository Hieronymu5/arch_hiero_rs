# C4 - Component diagram

**Type id:** `c4-component` · **Has arrows:** yes (relationships) · **Custom colours:** no (fixed scheme) · **Line weight:** medium by default
**Rust crate:** `crates/diagrams/hiero-c4` (shared with the other C4 types)
**Reference:** <https://c4model.com/diagrams/component> (no prototype: new in the Rust app)

The third level of the C4 model: the **components** inside one container, drawn inside its
**container boundary**, which can sit inside the **system boundary** of the software system
it belongs to, with the other containers, people, and software systems they work with.

## Elements

All sizes are in canvas pixels and multiples of the 20 px grid. Everything but a boundary shows
its name in bold, its type in square brackets with any technology (such as
`[Component: Spring MVC]`), and its description inside it.

| Kind | Label | Default size | Can contain | Drawn as |
|---|---|---|---|---|
| `person` | Person | 200×160 | – | The actor figure, filled in the person's colour, with the name, the type (`[Person]` unless set), and the description under it, in the same sizes as other elements'. Resizable. |
| `software-system` | Software System | 240×140 | – | A box with round corners. |
| `system-boundary` | System | 800×560 | container-boundary, web-application, database, store | A blue line around what it holds, with its name, `[Software System]`, and description in its bottom-left corner. Takes no arrows. |
| `container-boundary` | Container | 600×400 | component, database, store | The same, with `[Container]`. It can sit inside a system boundary. The palette calls the two *System* and *Container*, as their buttons are narrow. |
| `web-application` | UI Container | 240×160 | – | A browser window, as on container diagrams, typed `[Container]`. |
| `component` | Component | 240×140 | – | A box with round corners and two small tabs sticking out of its upper-left edge. |
| `database` | Database | 240×160 | – | A cylinder, typed `[Container]`. |
| `store` | Store | 240×160 | – | A bucket, typed `[Container]`. |

Boundaries have header 20, footer 60, and padding 20, are drawn behind what they hold (system
boundaries behind container boundaries), and grow to fit it. Everything is resizable, boundaries no smaller than
what they hold. Arrows go between any two elements but boundaries.

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters |
| Technology | web-application, component, database, store | At most 60 characters, shown in the type line |
| Description | all | At most 500 characters |
| Colour | all | Radio: *Blue*, *Green*, *Orange*, or *Red*. Stored as `color`: `"green"`, `"orange"`, or `"red"`, left out for blue. By C4's custom, blue is the software system in scope and what's in it, orange another of the organisation's own systems, and red a system outside it. A diagram from before every element had a colour stored a software system's `scope` instead: `"internal"` is read as orange and `"external"` as red, and written back as `color`. |
| Type | person, software-system | At most 40 characters, shown in square brackets in place of `[Person]` or `[Software System]`, such as `[Customer]` or `[External System]`. Blank shows the usual type. Stored as `typeName`. |
| Parent | what a boundary can hold | The boundary it sits in, or none |

## Relationships (arrows)

As on container diagrams: a dashed black line with a head, its description (≤ 150) centred on
its middle and its technology (≤ 60) in square brackets under it.

## Export

- **SVG:** the shared scene.
- **draw.io:** boundaries are draw.io containers, nested as on the canvas. A component is a
  rounded box with its two tabs as child shapes. Other shapes as on container diagrams.

## Stored data

```jsonc
// element props
{ "color": "red" }               // any element: "green" | "orange" | "red"; omit for blue
{ "typeName": "Customer" }               // person, software-system; omit for the usual type
{ "technology": "Spring Bean" }  // web-application, component, database, store
// connection props
{ "description": "Validates credentials using", "technology": "JSON/HTTPS" }
```

## Checklist

- [x] Palette with all 8 kinds; a system boundary holding a container boundary holding components
- [x] Component shape with its two tabs; browser, cylinder, and bucket shapes
- [x] Relationships with description and technology
- [x] Line thickness, canvas size, view mode, sub diagrams and links, cut/copy/paste (shared)
- [x] SVG and draw.io export
- [x] MCP: `list_diagram_types`, `create_diagram`, `edit_diagram` with the props above
