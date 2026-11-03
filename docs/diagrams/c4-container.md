# C4 - Container diagram

**Type id:** `c4-container` · **Has arrows:** yes (relationships) · **Custom colours:** no (fixed scheme) · **Line weight:** medium by default
**Rust crate:** `crates/diagrams/hiero-c4` (shared with the other C4 types)
**Reference:** <https://c4model.com/diagrams/container> (no prototype: new in the Rust app)

The second level of the C4 model: the **containers** (applications and data stores) inside
one software system, drawn inside its **system boundary**, with the people and other software
systems they work with, and the relationships between them.

## Elements

All sizes are in canvas pixels and multiples of the 20 px grid. Everything but a person shows
its name in bold, its type in square brackets with any technology (such as
`[Container: Java and Spring Boot]`), and its description inside it.

| Kind | Label | Default size | Can contain | Drawn as |
|---|---|---|---|---|
| `person` | Person | 120×80 | – | The actor figure, filled blue, with the name underneath. No description. |
| `software-system` | Software System | 240×140 | – | A box with round corners. |
| `system-boundary` | System | 600×400 | web-application, static-content, backend, store, database, service, queue, topic | A blue line with round corners and no fill, around what it holds. Its name, `[Software System]`, and its description (one line) sit in its bottom-left corner. Header 20, footer 60, padding 20. Takes no arrows. The palette calls it *System*, as its button is narrow. |
| `web-application` | Website | 240×160 | – | A browser window: a blue bar across the top with three dots and an address bar. |
| `static-content` | Static Content | 240×160 | – | A folder, with a tab on its top-left. |
| `backend` | Backend | 240×160 | – | A box with a prompt (`>_`) in its top-left corner. |
| `store` | Store | 240×160 | – | A bucket: an elliptical top, narrowing to the bottom. |
| `database` | Database | 240×160 | – | A cylinder. |
| `service` | Service | 200×160 | – | A hexagon, pointed on its left and right, for a microservice or another service that owns one business capability (see <https://c4model.com/abstractions/microservices>). Its points stand out as a regular hexagon's would, but no more than a quarter of its width; its text is inset by half a point. Arrows end on its outline, not its box. |
| `queue` | Queue | 240×120 | – | A cylinder on its side, its open end (an ellipse 28 wide, or half the width if less) on the left: a message queue, which one consumer takes each message from. |
| `topic` | Topic | 240×120 | – | The same cylinder: a topic, which every subscriber gets each message from. |

Every element is blue unless its **Colour** is green, orange, or red (see the system context spec for the values); a boundary has no fill, so only its line takes it. Every element but a person is
resizable; the boundary grows to fit what it holds. Dropping a container on the boundary puts it
inside. Arrows go between any two elements but boundaries.

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters |
| Technology | web-application, static-content, backend, store, database, service, queue, topic | At most 60 characters. Shown as `[Container: technology]`; without it, `[Container]`. |
| Description | all but person | At most 500 characters. Inside: as many lines as fit. A boundary: one line. |
| Colour | all | Radio: *Blue*, *Green*, *Orange*, or *Red*. Stored as `color`: `"green"`, `"orange"`, or `"red"`, left out for blue. By C4's custom, blue is the software system in scope and what's in it, orange another of the organisation's own systems, and red a system outside it. A diagram from before every element had a colour stored a software system's `scope` instead: `"internal"` is read as orange and `"external"` as red, and written back as `color`. |
| Parent | containers | The system boundary, or none |

## Queues and topics

Following <https://c4model.com/abstractions/queues-and-topics>, each queue and topic is its own
container, rather than the message bus being one: services that send or publish messages point
to it ("Sends messages to", "Publishes messages to"), and services that take them point to it
too ("Subscribes to messages from") or from it ("Sends messages to"). A point-to-point coupling
can instead leave the queue out and name it in the relationship's technology, such as
`via Queue X`.

## Relationships (arrows)

As on system context diagrams (dashed black line, text on its middle), plus a technology:

| Field | Rule | Drawn as |
|---|---|---|
| Description (`description`) | At most 150 characters | Centred on the middle of the line, wrapped to 180 px |
| Technology (`technology`) | At most 60 characters, such as `JSON/HTTPS` | Under the description, in square brackets, at 11 px |

## Export

- **SVG:** the shared scene.
- **draw.io:** the boundary is a draw.io container holding its containers, with its text at the
  bottom left. A website is a rounded box with its bar, dots, and address bar as child shapes;
  static content is draw.io's `folder`; a backend a rounded box with a `>_` text child; a store a
  bucket stencil; a database draw.io's `cylinder3`; a service draw.io's `hexagon` with a fixed point
  of the same size; a queue or topic draw.io's `cylinder3` turned to face north, so its top is
  on the left. Relationships are dashed connectors with
  their description and technology as the edge label.

## Stored data

```jsonc
// element props
{ "color": "orange" }                    // any element: "green" | "orange" | "red"; omit for blue
{ "technology": "Java and Spring Boot" } // web-application, static-content, backend, store, database, service, queue, topic
// connection props
{ "description": "Makes API requests to", "technology": "JSON/HTTPS" }
```

## Checklist

- [x] Palette with all 11 kinds, the system boundary holding containers
- [x] Browser, folder, prompt, bucket, cylinder, hexagon (service), and sideways cylinder (queue and topic) shapes, with name, type and technology, and description inside
- [x] Boundary text in its bottom-left corner
- [x] Relationships with description and technology
- [x] Line thickness, canvas size, view mode, sub diagrams and links, cut/copy/paste (shared)
- [x] SVG and draw.io export
- [x] MCP: `list_diagram_types`, `create_diagram`, `edit_diagram` with the props above
