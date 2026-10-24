---
name: arch-hiero-diagrams
description: Build architecture diagrams in Arch Hiero through its local MCP server (arch-hiero-mcp) and export them as SVG and draw.io. Use when the user asks for a conceptual, physical, or deployed component diagram made with Arch Hiero, or to lay out, tidy, or export one.
argument-hint: "[the diagram to build, and where to save it]"
---

# Arch Hiero diagrams

Arch Hiero is a desktop app that draws three types of architecture diagram. Its MCP server, `arch-hiero-mcp`, reads and edits the same SQLite database as the app, so whatever you build shows up in the app straight away, and the user can undo it there. You build a diagram with a few tool calls, check how it came out, fix it, and export it.

| Type | Shows | Arrows |
| --- | --- | --- |
| `conceptual` | Actors, domains, applications, the components, processes, entities, and files in them, and jobs | Labelled, with a connector (interface), manual or automated, one- or two-way |
| `physical` | Users, servers, databases, load balancers, firewalls, gateways, and storage in zones and networks | Protocol and port |
| `deployed-component` | Components in layers, such as network zones, with protocols and tech stacks | None |

## 1. Connect to the server

The tools come from an MCP server named `arch-hiero`, such as `mcp__arch-hiero__edit_diagram`. If they're missing, ask the user to add the server: the binary is `ArchHiero.app/Contents/MacOS/arch-hiero-mcp` in an installed app, or `target/release/arch-hiero-mcp` in a build of this repo (`cargo build -p hiero-mcp --release`), run over stdio. It uses the app's database unless given `--db <path>` or `ARCH_HIERO_DB`.

| Tool | Use |
| --- | --- |
| `list_diagram_types` | Each type's kinds of element, with their size, what they can contain and connect to, and what their `props` can be. Read it rather than guessing. The `arch-hiero://types/{type}` resource has the full rules. |
| `list_diagrams`, `get_diagram` | Find a diagram, and read its elements, arrows, `revision`, and `layout`: each element's drawn `x`, `y`, `width`, and `height`. |
| `create_diagram` | `name`, `type`, and an optional `description`. A diagram's type can't be changed later. |
| `edit_diagram` | A batch of `ops` (below), applied with the type's rules, all or nothing. |
| `update_diagram_details` | The name, description, or colours. |
| `save_version`, `list_versions` | Record the diagram as a numbered version, with a short description. |
| `export_diagram` | `svg` or `drawio` (returned as text, and written to `path` if given), or `zip` (the diagram file, to `path`). Add `overwrite: true` to replace a file. |

`delete_diagram`, `revert_to_version`, and `delete_version` are destructive: use them only when the user asks. The user's diagrams live in that database, so don't touch diagrams you weren't asked about. Before changing an existing diagram, call `save_version` with a description such as "Before agent edits".

### Edits

`edit_diagram` takes `id` and `ops`, a list of edits applied in order. If one fails, none are applied, and the error says which one and why.

| `op` | Fields |
| --- | --- |
| `add_element` | `kind`, `label`, optional `description`, `parent_id`, `x`, `y`, `props`, and `ref` |
| `update_element` | `id`, and any of `label`, `description`, `parent_id` (`""` takes it out of its container), and `props` (merged; `null` clears a field) |
| `move_element` | `id`, `x`, `y` |
| `resize_element` | `id`, `width`, `height` |
| `remove_element` | `id`, which also removes its arrows and anything inside it |
| `add_connection` | `source_id`, `target_id`, optional `props` and `ref` |
| `update_connection` | `id`, `props` (merged) |
| `remove_connection` | `id` |

Give each new element a `ref`, such as `"orderEntry"`, and use it as an id in later edits of the same batch, for `parent_id`, `source_id`, and the like. The result lists the real id given to each `ref` under `created`, and includes the diagram and its `layout` afterwards. Element fields are snake case (`parent_id`); `props` are camel case (`componentType`), as `list_diagram_types` lists them.

## 2. Plan the layout before adding anything

1. Call `list_diagram_types`, then `create_diagram`. A new physical diagram already has zones and a network: read their ids from the result.
2. Sketch the diagram as rows and columns, and work out each element's `x` and `y`. Positions are the element's top-left corner, in pixels, snapped to a 20-pixel grid.
3. Line elements up so arrows run straight (see below).

### How arrows attach

An arrow joins the sides of its two elements that face each other. The arrows meeting one side of an element are spread evenly along it: one arrow meets the middle of the side, two meet it at a third and two thirds, and so on, ordered by where their other ends are, so they don't cross. An arrow is straight when its two ends line up.

- **One arrow between two elements in a column:** give them the same centre, such as the same `x` for elements of the same width, and the arrow runs straight down the middle of both.
- **Several elements above a wide one:** each arrow meets the wide element's side at its share of the side, not under the element it comes from, so it's drawn at an angle. On a conceptual diagram, pin it straight with `manualOffset` (below). On a physical diagram, give each element its own target, or accept the angle.
- **`manualOffset` (conceptual arrows only)** runs an arrow straight at a position you choose: the arrow's x (for an arrow running up or down) or y (across) minus the source element's left (or top) edge. It's kept within where the two facing sides overlap, 10 pixels clear of their corners. For example, an actor at `x` 40, 120 wide, above an application at `x` 40, 420 wide: `"manualOffset": 60` runs the arrow down the actor's middle, at x 100. Set it to `null` to place the arrow automatically again.
- **A user reaching several sites**, such as one user to two data centres, can't have every arrow straight. Centre the user between them.

## 3. Build

Put the whole diagram, or each large part, in one `edit_diagram` batch.

- **`add_element`** with `parent_id` and `x`/`y` puts the element at that spot inside the container. A container is kept to its current size, so add it and resize it before adding what goes inside.
- **`add_element`** with only `x`/`y` puts the element in whichever container its centre lands on. Prefer giving `parent_id`. With neither, it goes at a free spot.
- Each type's rules may move what you place: positions snap to the grid, zones fill their network, and layers stack. Elements aren't kept apart, so two given the same spot overlap. Compare the `layout` in the result with your plan.
- **`resize_element`** resizes containers, and applications and other resizable kinds, keeping their top-left corner where it is.
- **`update_element`**, **`move_element`**, **`remove_element`**, and **`remove_connection`** fix things without starting over.

## 4. Check, fix, and export

1. **Bent arrows:** from the `layout`, check that the two ends of each arrow line up (see above). Fix the ones that don't by moving an element or, on a conceptual diagram, with `manualOffset`.
2. **Look at it:** the user can see the diagram in Arch Hiero as you build it. To see it yourself, take a screenshot with the app. With `ARCH_HIERO_OPEN` set to the diagram's name and `ARCH_HIERO_SCREENSHOT` to a PNG path, it opens the diagram, saves the window, and quits:

   ```bash
   ARCH_HIERO_OPEN="Order Entry" ARCH_HIERO_SCREENSHOT=/tmp/order-entry.png \
     ARCH_HIERO_SCREENSHOT_AFTER=2 <ArchHiero.app/Contents/MacOS/arch-hiero or target/debug/arch-hiero>
   ```

   Pass the same `ARCH_HIERO_DB` as the server if it uses its own database. The window shows the top-left of the canvas at 100%, so check a large diagram in parts, or export it as SVG and render that with `rsvg-convert` if it's installed. Look for cut-off names, labels and arrows that overlap, and arrows that run through text, such as a horizontal arrow through a "×2" instance badge, which sits to the right of a figure.
3. **Shorten long names:** a figure's name gets two lines under it, about 12 characters each, so use "HTTPD Proxy" rather than "Apache HTTP Reverse Proxy". Split a deployed component's description into short lines.
4. **Save:** export SVG and draw.io with `export_diagram` and a `path` where the user asked, named after the diagram, such as `physical-3-tier-web-app.svg`. Then call `save_version` with a description of what you built, so the diagram shows as Saved. Say which arrows aren't straight and why.

Start from the example closest to what you're building, in `examples/`.

## Conventions by diagram type

### Conceptual

- Put actors and downstream systems (those that take the central application's data, such as a data lake fed by an extract job) on top, the central application in the middle, and upstream systems (those it gets data from, such as an inventory API it calls) below.
- Lay out the story left to right in the order things happen, where there's an order. Put the applications in a flow side by side, the first one on the left. In a row of elements above or below an application, put what happens first on the left, such as the user entering a request to the left of the data lake the application later exports that request to.
- Make the central application only as wide as the dependencies above or below it span, and run arrows vertically.
- Put what's inside an application with `parent_id`: components, processes, entities, and files. A `component` with `"componentType": "database"` is drawn as a cylinder, and `"storage"` as a stack of drives, each 120 × 80 with the name underneath, and then it can't hold anything. A `file` is data kept as a file, such as an export or a CSV: a page with a folded corner, 120 × 80, in the data colour, with no arrows. An application's header, with its name and description, is 60 tall. An arrow from above to something inside an application crosses the header, whose text is centred, so put that thing to one side, such as the right third of the application.
- Give each arrow to an application its interface as `connector`, such as `Web`, `REST API`, or `JDBC`, and a short `label`.
- Use `"mode": "manual"` (dashed) for something a person does, and `"bidirectional": true` when data comes back, such as a job pulling data.
- Use a `job` for scheduled work, such as an extract. It can have arrows to applications and components.

### Physical

- A new diagram has an **End User** zone at the top and an **Intranet** network with **Perimeter**, **Business**, and **Restricted** zones stacked in it. Rename the network for a data centre, and add a `network` beside it for each other one. A new network copies the existing one's zones and their sizes. The End User zone widens to span the networks.
- Zones stack top to bottom in their network and span its width; only their height changes. A zone 200 tall holds one row of figures. Make it 320 tall to stack two, such as a load balancer above the servers behind it.
- Figures are 120 × 100. Give a tier's figures the same `x` for vertical arrows.
- Use `instanceCount` for a cluster of identical servers, which draws one figure with "×4" beside it. Use separate figures when the copies play different roles, such as a primary and a replica.
- `serverType` is `virtual-machine` or `container`, or omitted for a physical server. `databaseType` sets the database's logo and its arrows' usual port.
- Give arrows a `protocol` and `port`, such as `HTTPS` and `443`.

### Deployed component

- Add layers in order from top to bottom, such as End User, Perimeter, Business, and Restricted zones. Their `characteristics`, such as "Internet facing" or "Behind firewall", are drawn across their top.
- Add components to a layer with `parent_id`, in order from left to right. Layers and components size themselves, so positions and `resize_element` only set the order.
- Each line of a component's description is a one-line bullet, so keep lines short and use several.
- `dependency` is `internal` (yellow) or `external` (red, such as a third-party identity provider), or omitted for a core component (green).
- `protocols` are drawn across the top of a component, and `techStack` as logos along the bottom. The `techStack` ids, such as `springboot`, `angular`, and `mysql`, are in [tech-stack-ids.md](tech-stack-ids.md). An unknown id is accepted but draws no logo, so use only those.
