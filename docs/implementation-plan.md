# Implementation plan: Angular prototype → Rust

Goal: rebuild the Angular prototype's feature set (`../arch_hiero`) as a native Rust/egui app
that stores diagrams in SQLite. The code for each diagram type lives in its own crate, and the
code they share lives in common crates.

Diagram type specs:

- [Conceptual](diagrams/conceptual.md)
- [Physical](diagrams/physical.md)
- [Deployed component](diagrams/deployed-component.md)

---

## Status (20 Oct 2026)

All phases are implemented. What's built differs from the plan below in these places:

- **Element model.** Each type has a fieldless `Kind` enum and one flattened `Props` struct
  (`Element<K, P>`), not a tagged enum with per-kind fields. That mirrors the prototype's
  optional fields and keeps the same JSON. Each type's `apply_element` clears fields its kind
  doesn't have.
- **Engine.** The `DiagramType` hooks receive the whole `Engine` and can call its `base_*`
  methods, as the prototype's overrides called `super`. The editing logic lives in
  `hiero-core::engine`; `hiero-editor` only maps input to it.
- **Text.** Measured with `harfrust` and the bundled Inter font (the shaper egui uses), so
  widths agree everywhere. Exported SVGs name Inter first, then the system fonts.
- **Undo history** lives in the app (`App::undo`, one stack per diagram), holding whole
  `DiagramSnapshot`s. Editors report each change with the content before it.
- **Store thread.** The app reads on its own connection and writes through a worker thread
  that tracks its own revisions, so a run of autosaves doesn't conflict with itself.
- **Exports** are ported writers with unit tests and visual checks. They haven't yet been
  compared with files the prototype exported (the golden-fixture idea in phase 9).
- **Tech stack**: the prototype has 92 technologies, not 93.
- **Packaging** uses `scripts/bundle-macos.sh` rather than `cargo-packager`. There's no icon
  yet.
- **Dev hooks**: `ARCH_HIERO_*` environment variables open, create, select, and screenshot,
  for checking the app without clicking (see AGENTS.md).

Not done yet: AccessKit beyond labels and live regions (each element isn't its own
accessibility node yet; the canvas announces the focused one), an app icon, and Windows and
Linux CI.

## 1. Feature inventory (what "parity" means)

| Area | Prototype behaviour | Rust home |
|---|---|---|
| Diagram list | Sorted by name, with type, "last updated <date>", and a **Saved/Unsaved** badge with the latest version number (new in Rust). Each row has Download and Delete. Delete asks for confirmation, focused on Cancel. Status/alert messages show the outcome. | `hiero-app::pages::list` |
| Add diagram | Form with Name (required, ≤100), Type (required), and Description (≤1000). A physical diagram starts with its standard zones. Opens the editor afterwards. | `hiero-app::pages::details` |
| Edit details | The same form plus per-type colour pickers (conceptual 8, physical 12) and "Use the default colors". Shows created/updated dates. **The type is read-only after creation** (a change from the prototype). | `hiero-app::pages::details` + each type's `Colors` |
| Import / export file | A `.zip` holding `diagram.json` = `{format:"arch-hiero-diagram", version:1, diagram}`. Importing replaces the diagram with the same id, or adds it. Errors are specific and readable. | `hiero-core::file` + `hiero-store` |
| Editor page chrome | Name, type, **Saved/Unsaved status, Save, Undo, Redo, History**, Help toggle, Export SVG, Export draw.io, Edit details, Back. Shows a "couldn't save" banner when an autosave fails. | `hiero-editor::page` |
| Canvas | Fixed 2400×1600 world, 20 px dotted grid. Pan by dragging with the right mouse button or with arrow keys; dragging empty canvas with the left draws a box that selects the elements wholly inside it (adding to the selection with Shift, Ctrl, or Cmd). Zoom **10–100%** from a slider, kept centred (the prototype allows 50–100%). The limits are constants, to adjust after trying it. | `hiero-editor::canvas` |
| Palette | Click adds at a free spot (inside the selected container if it fits). Drag drops where released. | `hiero-editor::palette` |
| Selection | Click to select. Shift/Ctrl/Cmd+click toggles. Shift+Space adds the focused element. Clicking empty canvas clears the selection. | `hiero-core::edit` + `hiero-editor` |
| Move | Drag, or arrow keys (Shift for bigger steps). Snaps to the grid and stays on the canvas. Groups move together. Containment is re-evaluated on drop. | same |
| Containers | Kinds declare `can_contain`. Containers grow to fit, with header, padding, and footer. A removed container's children stay. | `hiero-core::layout` |
| Resize | Top-left and bottom-right handles (or width-only), by drag or arrow keys. Minimum sizes. Stays around its children. | same |
| Delete | Delete/Backspace removes the selection, unless focus is in a text field. | same |
| **Cut, copy, paste** (new) | ⌘X / ⌘C / ⌘V (Ctrl elsewhere), unless focus is in a text field. Copies the selected elements with everything they hold and the arrows between them, as JSON text (`{format:"arch-hiero-elements", version:1, type, elements, connections}`) on the system clipboard, so it pastes into any diagram of the same type. Pasted text is checked as strictly as an imported file. Pasted elements get new ids and are selected; they go into the selected container when it can hold them all, or else where they were copied from, a grid step down and right for each copy already there, landing in the container their middle is over. Cut removes what it copied. Undoable. | `hiero-core::clipboard` + `hiero-editor` |
| Properties panel | Right of the canvas, shown only while one element or arrow is selected. Per-type fields. Applied while valid. Escape returns focus to the canvas. | per-type `ui` + `hiero-editor::properties` |
| Arrows | Drag from the connect handle, or "Add arrow to". Facing-side routing with even spread. Select, edit, delete. | `hiero-core::routing` + `hiero-editor::arrows` |
| Help | Per-type help text above the canvas | per-type `HELP` constant |
| Exports | SVG cropped to the content. draw.io with containers and attached connectors, and a name/description heading. | `hiero-core::export` + per-type overrides |
| Theme | System / Light / Dark picker, remembered | `hiero-app::theme` |
| Accessibility | Every change announced in an ARIA live region, full keyboard operation, WCAG AA contrast | AccessKit (egui) + `describe()` hooks |
| Autosave | Each finished change saves straight away to the current record | `hiero-store`, called by `hiero-editor` |
| **Undo / redo** (new) | ⌘Z / ⇧⌘Z (Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y elsewhere) and toolbar buttons. Covers every content change in the editor. | `hiero-core::history` + `hiero-editor` |
| **Versions** (new) | A Save button (⌘S) records the current diagram as a new numbered version. The status shows *Saved* when the current record matches the latest version, otherwise *Unsaved*. | `hiero-core::history` + `hiero-store` + `hiero-editor` |
| **History view** (new) | Lists versions with number, date, and description. Each version can be reverted to (which records a new version), exported as .zip, SVG, or draw.io, or deleted. | `hiero-app::pages::history` |

Out of scope for now, but the design should make it easy later: systems catalogue entities.
There will be no REST API: the app is local-first, and agents use the MCP server. Usernames are dropped: the Rust version records dates only, with no "by <user>".

---

## 2. Code organisation strategy

### Principles

1. **Each diagram type is a crate.** Diagram crates never depend on each other. Code that a
   second type needs moves down into a common crate, the same rule as the prototype's "move to
   `common/`". For example, the actor figure and the dependency colours are used by more than one
   type, so they go in `hiero-core`.
2. **The engine is generic and diagram types plug into it.** The prototype's `ElementEditor`
   base class with overridable hooks (`arrange`, `created`, `constrainMove`, `resized`,
   `resizeLimits`, `dropPosition`, `remove`, …) becomes a Rust **trait with default methods**.
   The engine is generic over that trait.
3. **Rules are separate from pixels.** Everything that decides *where things go* is pure,
   UI-free, and unit-tested. Only the painting and the properties forms touch egui.
4. **Render once, draw twice.** Each type builds a `Scene` (a list of primitive shapes and
   text) from its laid-out content. The egui backend paints the scene and the SVG backend
   serialises it, so figures are written once. The prototype shares figure geometry, but draws
   each figure twice: once in its Angular templates and again in `diagram-svg.ts`. draw.io stays
   semantic (cells and edges), so it has its own writer.
5. **Storage doesn't know about types.** SQLite stores the common columns, plus a JSON `props`
   column that each type defines. `hiero-store` never imports a diagram crate.

### Crate layout

```
crates/
  hiero-core/            UI-free, no egui, no SQLite
    geometry.rs          Point, Size, Rect, snapping, overlap
    ids.rs               DiagramId, ElementId, ConnectionId (UUID newtypes)
    audit.rs             Timestamps { created_at, updated_at } (no usernames)
    history.rs           DiagramSnapshot, content_hash(), UndoStack<T> (see section 4)
    diagram.rs           DiagramMeta, DiagramTypeId, Element<K>, Connection<A>, Content<T>
    kind.rs              KindInfo (label, size, can_contain, container_look, header/footer/
                         padding, resizable, layer, has_description, connects_to)
    rules.rs             trait DiagramType (hooks with defaults, see below)
    layout.rs            fit_container, find_free_position, stacked_placement, distribute,
                         clamp_into, content_area, resize_from_top_left … (port of diagram-layout.ts)
    edit.rs              Pure editing operations: add, move, group-move, resize, reparent,
                         remove, select. They return a new Content plus an Announcement.
    routing.rs           route_connections, facing_side, straight_range, label/connector/
                         tag placement (port of diagram-connections.ts)
    color.rs             luminance, contrast_ratio, readable_text, tint
    palette.rs           Shared colour tables (dependency fills, defaults reused by >1 type)
    figures.rs           Shared figure geometry: actor, cylinder, storage stack, cloud path
    scene.rs             Scene primitives: Rect, RoundedRect, Path, Line, Text, Image, Dash
    text.rs              trait MeasureText, FontMetrics (the bundled font read with ttf-parser),
                         and an estimating implementation for tests
    export/svg.rs        Scene → SVG string, cropped to the content
    export/drawio.rs     mxfile writer: cells, containers, edges, heading
    file.rs              .zip diagram file read/write. Same format as the prototype.
  hiero-store/           SQLite (rusqlite, bundled). Depends on hiero-core only.
    schema/              migrations (rusqlite_migration, user_version)
    repo.rs              DiagramRepository: list, get, create, update_meta, save_content,
                         delete, import (upsert by id), save_version, list_versions,
                         get_version, revert_to_version, delete_version,
                         status
    seed.rs              First-run sample diagrams (ported from public/data/diagrams.json)
  hiero-editor/          egui. Depends on hiero-core.
    canvas.rs            World↔screen transform, grid, pan/zoom, hit-testing, focus/keys
    paint.rs             Scene → egui::Painter
    engine.rs            Editor<T: DiagramType + DiagramView>: input → hiero_core::edit ops
    arrows.rs            Connect handle, pending arrow, arrow selection
    palette.rs, properties.rs, page.rs (chrome), widgets.rs (text field w/ validation, etc.)
                         (Text is measured with hiero_core::text::FontMetrics, and egui renders the
                         same bundled font, so the app, the MCP server, and exports agree.)
  diagrams/
    hiero-conceptual/    model.rs (ConceptualKind, ConceptualArrow, Colors), rules.rs (impl
                         DiagramType), scene.rs, drawio.rs, ui.rs (impl DiagramView), help.rs
                         ui.rs and the hiero-editor dependency sit behind a default `ui` feature.
    hiero-physical/      same layout. rules.rs holds the zone/network stacking logic.
    hiero-deployed/      same layout, plus tech_stack.rs and assets/tech/*.png, and its own
                         svg.rs and drawio.rs
  hiero-registry/        UI-free registry of diagram types: id → label, has_arrows,
                         new content, apply ops, export. Used by the app and the MCP server.
                         Depends on the diagram crates with `default-features = false`.
  hiero-app/             The GUI binary `arch-hiero`: navigation, list/details pages, theme,
                         the store thread, and the external-change watcher
  hiero-mcp/             The MCP server binary `arch-hiero-mcp` (stdio). Built on rmcp + tokio.
                         Has no egui dependency.
```

**Allowed dependencies** (CI checks these with `cargo metadata`):

```
hiero-core       ◄── everything (depends on nothing in the workspace)
hiero-store      ──► hiero-core
hiero-editor     ──► hiero-core
diagrams/hiero-* ──► hiero-core, and hiero-editor only with the "ui" feature (default on)
hiero-registry   ──► diagrams/hiero-* (default-features = false), hiero-core
hiero-app        ──► hiero-registry, hiero-store, hiero-editor, diagrams/hiero-* (with "ui")
hiero-mcp        ──► hiero-registry, hiero-store            (no egui anywhere in its tree)
```

Inside a diagram crate, only `ui.rs` (and its submodules) may `use egui`, and only under
`#[cfg(feature = "ui")]`. CI builds `hiero-mcp` on its own and checks that `egui` isn't in its
dependency tree (`cargo tree -p hiero-mcp -i egui` must fail). That proves the rules, scene, and
export code really are UI-free.

### Core types

```rust
// hiero-core::diagram
pub struct Element<K> {
    pub id: ElementId,
    pub label: String,
    pub parent_id: Option<ElementId>,
    pub x: f32, pub y: f32,
    pub width: Option<f32>, pub height: Option<f32>,   // set only after a resize
    #[serde(flatten)] pub kind: K,                     // per-type enum: tag + per-kind props
}
pub struct Connection<A> {
    pub id: ConnectionId, pub source_id: ElementId, pub target_id: ElementId,
    #[serde(flatten)] pub props: A,                    // () / ConceptualArrow / PhysicalArrow
}
pub struct Content<T: DiagramType> {
    pub elements: Vec<Element<T::Kind>>,               // order matters: draw order and ties
    pub connections: Vec<Connection<T::Arrow>>,
}
```

Per-type kinds are serde-tagged enums, so the JSON is byte-compatible with the prototype's flat
shape:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum PhysicalKind {
    User,
    Server { #[serde(default, skip_serializing_if = "Option::is_none")] server_type: Option<ServerType>,
             #[serde(default, skip_serializing_if = "Option::is_none")] instance_count: Option<u32> },
    // …
}
```

### The `DiagramType` trait (UI-free hooks)

This mirrors the prototype's `ElementEditor` overrides. Every method has a default, so a simple
type implements only what it needs.

```rust
pub trait DiagramType: Sized + 'static {
    const ID: DiagramTypeId;            // "conceptual" | "physical" | "deployed-component"
    const LABEL: &'static str;
    type Kind: ElementKind;             // fn info(&self) -> &'static KindInfo; fn template(kind) -> Self
    type Arrow: ArrowProps;             // NoArrows (uninhabited) turns the arrow tool off
    type Colors: ColorScheme;           // NoColors for the deployed component type

    fn palette() -> &'static [KindInfo];
    fn starting_content(ids: &mut impl IdGen) -> Content<Self> { Content::default() }

    fn arrange(c: &mut Content<Self>) {}
    fn natural_size(e: &Element<Self::Kind>) -> Size { e.kind.info().size }
    fn holds_elements(e: &Element<Self::Kind>) -> bool { !e.kind.info().can_contain.is_empty() }
    fn created(c: &Content<Self>, new: Element<Self::Kind>, ids: &mut impl IdGen) -> Created<Self> { … }
    fn preferred_position(c: &Content<Self>, kind: &KindInfo) -> Option<Point> { None }
    fn constrain_move(c: &LaidOut<Self>, id: ElementId, p: Placement) -> Placement { p }
    fn commit_move(c: &mut Content<Self>, id: ElementId, p: Placement) { edit::default_move(c, id, p) }
    fn resized(c: &mut Content<Self>, id: ElementId, r: Rect) { edit::default_resize(c, id, r) }
    fn resize_limits(c: &LaidOut<Self>, id: ElementId) -> ResizeLimits { … }
    fn drop_position(kind: &KindInfo, at: Point, parent: &LaidOutElement<Self>) -> Point { … }
    fn remove(c: &mut Content<Self>, id: ElementId) { edit::default_remove(c, id) }
    fn key_move(c: &mut Content<Self>, id: ElementId, dir: Dir) -> KeyMove { KeyMove::Default }
    fn can_connect(src: &Element<Self::Kind>, dst: &Element<Self::Kind>) -> bool { false }
    fn new_arrow(src: &Element<Self::Kind>, dst: &Element<Self::Kind>) -> Self::Arrow;
    fn describe(c: &LaidOut<Self>, id: ElementId) -> String;     // accessibility text
    fn scene(c: &LaidOut<Self>, colors: &Self::Colors, m: &dyn MeasureText) -> Scene;
    fn to_drawio(meta: &DiagramMeta, c: &LaidOut<Self>, colors: &Self::Colors, m: &dyn MeasureText) -> String;
}
```

### The `DiagramView` trait (egui, in each type's `ui.rs`)

```rust
pub trait DiagramView: DiagramType {
    const HELP: &'static str;
    fn element_properties(ui: &mut egui::Ui, e: &Element<Self::Kind>, cx: &PropsCx<Self>) -> Option<Edit<Self>>;
    fn arrow_properties(ui: &mut egui::Ui, a: &Connection<Self::Arrow>, cx: &PropsCx<Self>) -> Option<Edit<Self>>;
    fn colors_form(ui: &mut egui::Ui, colors: &mut Self::Colors) -> bool { false }
    fn arrow_drag(…) { }   // conceptual: sideways drag straightens the arrow
}
```

### App-level registry (type erasure)

The UI-free half lives in `hiero-registry`, so the GUI and the MCP server share it:
`TypeEntry { id, label, has_arrows, palette(), new_content(), apply(ops), to_svg(), to_drawio(),
schema() }`. `hiero-editor` provides an object-safe `trait DiagramEditor { fn ui(&mut self, ui) -> Outcome;
fn export_svg(&self) -> String; fn export_drawio(&self) -> String; fn take_dirty(&mut self) ->
Option<StoredContent>; }`, which every `Editor<T>` implements. The app keeps one table:

```rust
pub static DIAGRAM_TYPES: &[TypeEntry] = &[
    TypeEntry::of::<Conceptual>(),          // label, id, has_arrows, open(), new_content(), colors
    TypeEntry::of::<Physical>(),
    TypeEntry::of::<DeployedComponent>(),
];
```

**Adding a fourth diagram type** means a new crate, one `impl DiagramType` and one `impl
DiagramView`, plus one line in `hiero-registry` and one in the app. The MCP server picks the new
type up automatically.

---

## 3. SQLite storage

Library: `rusqlite` 0.40 with the `bundled` feature (so there's no system SQLite version drift),
and `rusqlite_migration` 2.6 for migrations. The database is at
`directories::ProjectDirs("", "", "ArchHiero").data_dir()/arch-hiero.sqlite3`, which is
`~/Library/Application Support/ArchHiero/` on macOS. Connection settings:
`PRAGMA journal_mode=WAL; foreign_keys=ON; busy_timeout=5000`.

### Schema (migration 1)

```sql
CREATE TABLE diagrams (
  id          TEXT PRIMARY KEY,                       -- UUID, same ids as prototype files
  type        TEXT NOT NULL CHECK (type IN ('conceptual','physical','deployed-component')),
  name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors      TEXT CHECK (colors IS NULL OR json_valid(colors)),  -- only overridden colours
  content_hash TEXT NOT NULL,                         -- SHA-256 of the current snapshot (section 4)
  revision    INTEGER NOT NULL DEFAULT 0,             -- +1 on every write; optimistic concurrency
  next_version INTEGER NOT NULL DEFAULT 1,            -- version numbers are never reused, even after a delete
  created_at  TEXT NOT NULL,                          -- RFC 3339 UTC; no usernames
  updated_at  TEXT NOT NULL
) STRICT;

-- A diagram's type is set when it's created and never changes.
CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;

CREATE TABLE elements (
  diagram_id TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
  id         TEXT NOT NULL,
  ord        INTEGER NOT NULL,                        -- list order: draw order and ties
  kind       TEXT NOT NULL,
  label      TEXT NOT NULL,
  parent_id  TEXT,
  x REAL NOT NULL, y REAL NOT NULL,
  width REAL, height REAL,
  props      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(props)),  -- type-specific fields
  PRIMARY KEY (diagram_id, id),
  FOREIGN KEY (diagram_id, parent_id) REFERENCES elements(diagram_id, id) DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE connections (
  diagram_id TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
  id         TEXT NOT NULL,
  ord        INTEGER NOT NULL,
  source_id  TEXT NOT NULL,
  target_id  TEXT NOT NULL,
  props      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(props)),
  PRIMARY KEY (diagram_id, id),
  FOREIGN KEY (diagram_id, source_id) REFERENCES elements(diagram_id, id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (diagram_id, target_id) REFERENCES elements(diagram_id, id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
) STRICT;

-- Versions recorded by the Save button. Immutable once written.
CREATE TABLE diagram_versions (
  diagram_id   TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
  version      INTEGER NOT NULL CHECK (version >= 1),   -- 1, 2, 3 … per diagram
  saved_at     TEXT NOT NULL,                           -- RFC 3339 UTC
  description  TEXT CHECK (description IS NULL OR length(description) <= 200),  -- optional, from the Save prompt
  content_hash TEXT NOT NULL,
  snapshot     TEXT NOT NULL CHECK (json_valid(snapshot)),  -- the diagram.json payload
  PRIMARY KEY (diagram_id, version)
) STRICT;

CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
```

Each version is stored as one JSON snapshot, not as normalised rows. Versions never change, are
read only as a whole (to revert or export), and the snapshot is already in the file format, so
exporting a version needs no conversion. The current record stays normalised, because it's the
one that is edited and queried.

Why the **common columns plus JSON props** design:

- The engine only needs the common columns. Each type owns the shape of its `props` through
  serde, so a new optional field doesn't need a migration (`#[serde(default)]`).
- The "information management system" later needs queries across diagrams, such as "every
  diagram that shows the CRM application". That works on the common columns, and SQLite's
  `json_extract` can reach into props.
- The tables map 1:1 to the prototype's JSON, so import and export are a direct conversion.

The alternative, one table per type, would put type knowledge into the store. That breaks
principle 5, and every new field would need a migration.

### Write strategy

- **Autosave:** after each finished change (drag end, resize end, add, remove, or a valid
  properties edit), the editor marks the content dirty. Properties typing is debounced by
  300 ms.
- A dedicated **store thread** owns the `Connection`. The UI sends
  `SaveContent { id, content }` over a channel and gets `Saved` or `Failed` back. The UI thread
  never blocks on disk, and a failure shows the prototype's "Your last change couldn't be saved"
  banner.
- `save_content` runs in one transaction: `DELETE` the diagram's elements and connections,
  `INSERT` them all again with prepared statements, then update `updated_at`, `content_hash`,
  and `revision`.
  Diagrams have hundreds of rows at most, so a full rewrite is simpler and safer than diffing.
  If profiling says otherwise, switch to diffing.
- **Import** (`.zip`) upserts by id: an existing diagram is replaced completely, and fields
  missing from the file are cleared. The timestamps are set fresh, as in the prototype. A file
  must follow every rule in `hiero_core::file_check`, the same rules the prototype applies
  (its `import-file-sync.md`); otherwise the import is refused with the first rule it breaks,
  such as "“Web”: “agent” isn’t a field a Server has." If the existing diagram has a
  different type, the import is refused: "“CRM” is a conceptual diagram, but the file holds a
  physical diagram."
- **Seed:** on the first run (an empty `diagrams` table), insert the prototype's sample
  diagrams from an embedded copy of `diagrams.json`, each with version 1.
- **One database, two processes.** The GUI and the MCP server (section 5) open the same file.
  WAL mode lets them read concurrently, and `busy_timeout` queues writes.
  - Every write passes the `revision` it read. `UPDATE … WHERE id = ? AND revision = ?`
    changing no rows means someone else wrote first, so the write fails with `Conflict`.
  - **Noticing outside changes:** the GUI's store thread checks `PRAGMA data_version` every
    500 ms. It's a cheap counter that changes only when *another* connection commits. When it
    changes, the list is refreshed, and an open diagram whose `revision` moved is reloaded.
  - The reload becomes one undo step labelled "Changes from an agent", with the notice "This
    diagram was changed outside the editor". ⌘Z takes the outside change back out.
  - If the GUI's own save hits `Conflict`, it reloads first. The user's change stays on the
    redo stack, and a notice says so, so nothing is silently overwritten.

---

## 4. Versions, Saved/Unsaved, and undo/redo

Two kinds of saving work side by side:

| | Current record | Versions |
|---|---|---|
| What | `diagrams` + `elements` + `connections` | `diagram_versions` |
| When written | Automatically, after every finished change | Only when the user clicks **Save** (⌘S / Ctrl+S) |
| Purpose | Never lose work, even after a crash | Named checkpoints to go back to or export |

### Saved / Unsaved

- `hiero-core::history::DiagramSnapshot` holds everything a version captures: name,
  description, colours, elements, and connections. The id and type aren't included, because
  they never change.
- `content_hash(&snapshot)` is the SHA-256 of the snapshot's serde_json serialisation. The
  serialisation is deterministic because snapshot types are structs and enums with a fixed
  field order, never `HashMap`s (use `BTreeMap` if a map is ever needed). Floats are already
  grid-snapped.
- The status is **Saved** when `diagrams.content_hash` equals the latest version's hash, and
  **Unsaved** otherwise. The check compares content, so undoing back to exactly the last saved
  state shows *Saved* again.

  ```sql
  SELECT d.content_hash = (SELECT v.content_hash FROM diagram_versions v
                           WHERE v.diagram_id = d.id ORDER BY v.version DESC LIMIT 1)
  FROM diagrams d WHERE d.id = ?1;
  ```
- The status appears beside the diagram name in the editor and as a badge in the diagram list
  ("Saved · v4" / "Unsaved · v4"). The **Save** button is disabled while Saved.
- Creating a diagram (including the seeds and a new diagram from import) records version 1
  right away, so a new diagram starts *Saved*.
- Editing details (name, description, colours) changes the current record, so the status
  becomes *Unsaved* until the next Save.
- **Save prompt:** clicking Save, or pressing ⌘S / Ctrl+S, opens a small popover titled "Save
  version 5". It has one optional field, **Description** (at most 200 characters, e.g. "Before
  the network redesign"), with focus in it, and **Save version** / **Cancel** buttons. Enter
  saves and Escape cancels, so saving without a description is still quick: ⌘S then Enter. A
  blank description is stored as `NULL`.
- **Save** runs in one transaction: read the current record, build its snapshot and hash, insert
  version `next_version` with the description, and increment `next_version`. It does nothing if the hash already
  equals the latest version's.
- There's no warning when leaving the editor or quitting while Unsaved, because autosave has
  already stored everything in the current record. *Unsaved* only means "not recorded as a
  version".

### Undo / redo

- `hiero-core::history::UndoStack<T>` has `past` and `future` lists of `DiagramSnapshot`s,
  capped at 500 steps. It's UI-free and unit-tested.
- Each finished change in the editor (the same events that trigger autosave) pushes the
  previous snapshot. Typing in one property field counts as one step, ending when the field
  loses focus or after the 300 ms debounce. A details edit made from the editor is one step.
- Undo and redo swap in a snapshot, re-run the type's `arrange`, and autosave, so the
  Saved/Unsaved status updates as you go.
- Shortcuts: ⌘Z / ⇧⌘Z on macOS, and Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y elsewhere. There are also
  toolbar buttons. The shortcuts are ignored while a text field has focus, so egui's own text
  undo still works there.
- **Lifetime:** the app keeps one stack per diagram, in memory, for as long as the app runs.
  Closing a diagram and opening it again keeps its undo history. Quitting the app clears all of
  it. Saving a version doesn't clear it.
- Every change to a diagram while the app is running goes onto that diagram's stack, wherever
  it comes from: the editor, a details edit, a revert or import from the list, or an outside
  change picked up from the MCP server. A stack is created the first time it's needed.
- Each step holds a full snapshot. Diagrams are tens of KB, so 500 steps is a few MB at worst.
  If profiling shows a problem, share unchanged elements between snapshots with `Arc`.
- Undo and redo only change the current record. They never add or remove versions.

### History view

Opened from **History** in the editor, or from a diagram's row in the list. It's a page that
lists the versions, newest first:

| Version | Saved | Description | |
|---|---|---|---|
| v5 · *matches current* | 20 Oct 2026, 18:05 | Reverted to v3 | Export ▾ · Delete |
| v4 | 20 Oct 2026, 17:42 | Before the network redesign | Revert · Export ▾ (.zip / SVG / draw.io) · Delete |
| v3 | 18 Oct 2026, 09:10 | – | Revert · Export ▾ · Delete |

- **Revert to vN** never rewrites history. It works forward, in one transaction:
  1. Replace the current record with version N's snapshot, re-running the type's `arrange`.
  2. Record that content as a **new version** `next_version`, with the description "Reverted
     to vN". The confirmation dialog shows this description and lets you edit it.
  3. Push the previous content onto the diagram's undo stack.

  Afterwards the status is *Saved*, and every earlier version is still listed. Revert isn't
  offered on the row that already matches the current record.
- **The confirmation dialog** has Cancel focused. It says "Revert to v3? This records the
  content of v3 as v6."
  - If the status is Unsaved, it adds: "Your current changes haven't been saved as a version.
    You can bring them back with Undo until you quit the app, or save them first."
  - The choices are **Save as v6 first** (only when Unsaved; uses the same optional
    Description field), **Revert**, and **Cancel**.
- **Undoing a revert** puts the previous content back into the current record, so the status
  becomes *Unsaved* again. The "Reverted to v3" version stays in the history.
- **Delete vN** removes one version, after the confirmation "Delete v3? This can't be undone."
  with Cancel focused. Deleting versions isn't undoable.
  - Version numbers are never reused. Deleting v6 and then saving gives v7, so an exported
    "CRM v6.svg" never refers to two different contents.
  - The status always compares against the newest remaining version. If every version is
    deleted, the status is *Unsaved* and the list badge reads "Unsaved · no versions".
  - Deleting a version never changes the current record.
- **Export vN** writes the snapshot as:
  - `.zip`: the standard diagram file. It keeps the diagram's id, so importing it later
    restores over that diagram.
  - `.svg` / `.drawio`: rendered from the snapshot by the type's `scene` / `to_drawio`. Those are
    UI-free, so no editor needs to be open.

  Files are named "<name> v3.<ext>".
- Versions are deleted one at a time with **Delete**, or all together when their diagram is
  deleted (`ON DELETE CASCADE`).

---

## 5. Local MCP server

An agent such as Claude Code, Codex, or Cursor can read and edit diagrams through a local MCP
server, while the user watches the changes appear in the GUI.

### Shape

- A separate binary, **`arch-hiero-mcp`**, speaks MCP over **stdio**. The agent's client
  launches it as a child process, so it works whether or not the GUI is open, and there's no
  network port or authentication to manage. It's shipped inside the app bundle
  (`ArchHiero.app/Contents/MacOS/arch-hiero-mcp`).
- It's built with the official Rust SDK, **`rmcp` 3.6** (`server`, `transport-io`, `macros`),
  on tokio. Tool input schemas are generated with `schemars` from the same serde types the app
  uses, so they can't drift.
- It opens the **same SQLite database** as the GUI (section 3), and `--db <path>` overrides the
  location for tests. Edits go through `hiero-registry`, which runs each diagram type's own
  rules (`arrange`, `created`, `can_connect`, `remove`, …). An agent therefore can't produce a
  layout the editor wouldn't: zones still fill their network, layers still stack, and arrows
  still respect `connects_to`.
- Client setup, documented in the README:

  ```sh
  claude mcp add arch-hiero -- /Applications/ArchHiero.app/Contents/MacOS/arch-hiero-mcp
  ```

  or in `.mcp.json`:
  `{"mcpServers":{"arch-hiero":{"command":"/Applications/ArchHiero.app/Contents/MacOS/arch-hiero-mcp"}}}`

  During development: `cargo run -p hiero-mcp`.

### Tools

Names use snake_case. Inputs and outputs are JSON in the diagram file format, so an agent sees
the same shapes as a `.zip`'s `diagram.json`. Tools that write return the new `revision`, and
take an optional `expected_revision` that is checked as described in section 3.

| Tool | Purpose | Annotations |
|---|---|---|
| `list_diagram_types` | Each type with its palette: kinds, which kinds each can contain or connect to, the per-kind property schema, and `propValues`, the values of props with a fixed list (a deployed component's `techStack` ids) | read-only |
| `list_diagrams` | id, name, type, updated_at, Saved/Unsaved, latest version | read-only |
| `get_diagram` | The full current diagram (elements + connections), with each element's laid-out `x/y/width/height` | read-only |
| `create_diagram` | name, type, description. Physical diagrams get their starting zones. Records v1. | – |
| `update_diagram_details` | name, description, colours. The type can't be changed. | – |
| `delete_diagram` | Deletes a diagram and its versions | **destructive** |
| `edit_diagram` | A batch of operations applied in one transaction: `add_element{kind, label, parent_id?, x?, y?, props?}` (no position means a free spot, as with a palette click), `update_element{id, label?, props?}`, `move_element{id, x, y \| parent_id}`, `resize_element`, `remove_element`, `add_connection{source_id, target_id, props?}`, `update_connection`, `remove_connection`. Earlier ops in a batch can refer to new elements by a client-chosen `ref`. Returns the ids created and the laid-out result. Any invalid op rejects the whole batch with a readable reason ("A firewall can't go inside a zone"). | – |
| `save_version` | Records a version, with an optional description (≤ 200) | – |
| `list_versions` | Version number, saved_at, description, and whether it matches the current record | read-only |
| `revert_to_version` | Same semantics as the history view: it records the old content as a new version, with an optional description (the default is "Reverted to vN"). Fails if the diagram is Unsaved, unless `discard_unsaved: true`. The agent should normally call `save_version` first. With the GUI open, the revert is undoable there. | **destructive** |
| `delete_version` | Deletes one saved version. Version numbers aren't reused. | **destructive** |
| `export_diagram` | `format: zip \| svg \| drawio`, optional `version`. SVG and draw.io are returned as text, and also written to `path` if one is given. A zip is written to `path`, and is required for zip. Won't overwrite an existing file unless `overwrite: true`. | read-only (writes only to the given path) |
| `import_diagram` | Imports a `.zip` from `path`, with the same rules as the GUI (upsert by id, refuse a type mismatch) | – |

### Resources and prompts

- `arch-hiero://diagrams/{id}` gives the current diagram as JSON, and
  `arch-hiero://diagrams/{id}/versions/{n}` a saved version. `arch-hiero://diagrams/{id}.svg`
  gives a rendering, so an agent can see the layout it produced.
- `arch-hiero://types/{type}` gives that type's spec, built from the `docs/diagrams/*.md` files
  with `include_str!`. The specs stay the single source of truth for both humans and agents.
- The server `instructions` tell agents to:
  - call `list_diagram_types` before editing
  - call `save_version` with a description such as "Before agent edits" before large changes
  - batch related operations into one `edit_diagram` call

### Safety

- **Local only:** stdio, no network listener, and it runs with the user's own permissions.
- **Every agent change can be recovered:** writes go to the current record, which becomes
  *Unsaved*. Saved versions are never changed by `edit_diagram`. If the GUI is open, each batch
  is one undo step there.
- **Destructive tools** (`delete_diagram`, `revert_to_version`, `delete_version`) carry `destructiveHint`, so
  clients ask the user before running them.
- **File access** is limited to the paths an agent passes to import/export. Nothing is
  overwritten without `overwrite: true`, and only `.zip`, `.svg`, and `.drawio` extensions are
  accepted.
- **Logging** goes to stderr only (stdout carries the protocol), with `RUST_LOG` to set the
  level.

### Testing

- Unit tests call the tool handlers directly against an in-memory database.
- An integration test starts `arch-hiero-mcp --db <tmp>` with rmcp's client
  (`transport-child-process`) and runs a scripted session: create a physical diagram, add a
  server to a zone, connect it to a database, save a version, export SVG, and revert.
- An integration test for running both processes at once: the GUI's store and the MCP server
  write to the same database file, and a conflict comes back as `Conflict`, never as lost data.

---

## 6. Phases

Each phase ends with `cargo fmt`, clippy, and tests clean, and a runnable app.

| # | Phase | Deliverables | Done when |
|---|---|---|---|
| 0 | **Restructure** | Create the crates above and move the current scaffold into them. Add the dependency-rule CI check. Update AGENTS.md. | Workspace builds. The existing demo canvas still runs. |
| 1 | **Core model + file format** | Port `geometry`, `ids`, `audit`, `Element`/`Connection`, `KindInfo`, `file.rs` (zip). Port the prototype's `diagram-file.spec.ts` and `zip.spec.ts` cases. | A prototype-exported `.zip` round-trips through Rust unchanged. |
| 2 | **Store + versions** | `hiero-store` with migration 1 (including `diagram_versions` and the type trigger), the repository, `save_version` (with optional description)/`list_versions`/`revert_to_version` (records a new version)/`delete_version`/`status`, the seed, and the store thread. `hiero-core::history` (snapshot, hash, `UndoStack`). Tests on an in-memory DB. | CRUD, import (including refusing a type mismatch), versioning (including that revert adds a version, and that version numbers aren't reused after a delete), and the Saved/Unsaved status are all tested. Cascade delete works. Changing the type is rejected. |
| 3 | **App shell + history view** | List page with Saved/Unsaved badges. Add/Edit details form with validation, where the type is chosen only when adding. Delete confirmation, import/export zip (`rfd` file dialogs), theme and accent colour pickers, outcome messages. History page with revert, delete, and .zip export. Undo stacks per diagram for the app's lifetime. | Every non-editor flow works against SQLite. |
| 4 | **Layout + routing ports** | `layout.rs`, `routing.rs`, `color.rs`, and `edit.rs` with the prototype's `diagram-layout.spec.ts` and `diagram-connections.spec.ts` cases ported as Rust tests. | Rust and TypeScript give the same results on the ported cases. |
| 5 | **Generic editor engine** | Canvas (fixed world, grid, pan, zoom 10–100%), Scene painter, palette (click and drag), selection and multi-select, move, snap, containers, resize handles, delete, properties panel frame, help panel, autosave, page chrome, **Save button + status, undo/redo**. Uses a throwaway "test type" crate. | Works with a minimal `DiagramType`. Keyboard-only use works. |
| 6 | **Conceptual** | `hiero-conceptual` (spec: [conceptual.md](diagrams/conceptual.md)). Shared arrow tool lands here. | Its parity checklist is ticked. |
| 7 | **Physical** | `hiero-physical` (spec: [physical.md](diagrams/physical.md)). Port `physical-diagram.spec.ts`. | Its parity checklist is ticked. |
| 8 | **Deployed component** | `hiero-deployed` (spec: [deployed-component.md](diagrams/deployed-component.md)), tech-stack extraction script, `deployed-component-layout.spec.ts` ported. | Its parity checklist is ticked. |
| 9 | **Exports** | Scene→SVG and the shared draw.io writer, plus the deployed component writers. SVG and draw.io export of history versions. Snapshot tests (`insta`) against SVG and draw.io output from the prototype for the seed diagrams. | Exported files open in a browser and in draw.io and match the prototype. |
| 10 | **MCP server** | `hiero-registry`, then `hiero-mcp` with the tools, resources, and instructions in section 5. The GUI's external-change watcher and conflict handling. Integration tests. The read-only tools, versions, and import/export can be built as soon as phase 2 is done. `edit_diagram` support for each type follows that type's phase. | An agent creates and edits each type of diagram with the GUI open, and the changes appear live and can be undone. `cargo tree -p hiero-mcp -i egui` finds nothing. |
| 11 | **Accessibility + packaging** | AccessKit labels and live announcements from `describe()`. macOS `.app` bundle (`cargo-packager`) containing both binaries, an icon, and code signing notes. | VoiceOver can operate the editor. The `.app` installs and runs. The MCP setup command in the README works against the installed bundle. |

Phases 6–8 depend only on phase 5, so they can run in parallel, one agent or developer per
diagram crate. This is the payoff of keeping the crates separate.

### Parity testing approach

- **Port the spec files.** The prototype has Vitest specs for layout, connections, zip, file,
  physical, deployed layout, SVG, and draw.io. Port their cases as Rust unit tests in the crate
  that owns the code.
- **Golden fixtures.** Run the prototype once to export SVG, draw.io, and zip for every seed
  diagram, then commit them under `tests/fixtures/prototype/`. Rust snapshot tests compare
  against them, normalising number formatting.
- **Text measurement.** Bundle one font (such as Inter). `hiero_core::text::FontMetrics` reads
  it with ttf-parser, and egui renders it, so the screen, exports, and the MCP server all get
  the same widths. Tests use the estimating measurer.

### Suggested dependencies

| Crate | Version | Used in | Why |
|---|---|---|---|
| `rusqlite` (`bundled`) | 0.40 | store | SQLite |
| `rusqlite_migration` | 2.6 | store | Schema migrations via `user_version` |
| `serde_json` | 1 | core, store | props columns, file format |
| `uuid` (`v4`, `serde`) | 1.28 | core | Ids compatible with the prototype |
| `jiff` | 0.2 | core | RFC 3339 timestamps |
| `sha2` | 0.11 | core | Snapshot content hash |
| `zip` (deflate only) | 9 | core | Diagram files |
| `directories` | 6 | app | Platform data dir |
| `egui_extras` (`image`) | 0.37 | editor, deployed | Tech-stack logos |
| `rfd` | latest | app | Native open/save dialogs |
| `insta` | 1.49 | dev | Snapshot tests for exports |
| `rmcp` (`server`, `transport-io`, `macros`) | 3.6 | mcp | MCP server SDK |
| `tokio` (`rt`, `macros`, `io-std`) | 1.54 | mcp | Async runtime for rmcp |
| `schemars` | 1.2 | core, mcp | Tool input schemas from the serde types |
| `ttf-parser` | 0.25 | core | Text measurement from the bundled font, without egui |

---

## 7. Decisions

| # | Question | Decision |
|---|---|---|
| 1 | Can a diagram's type be changed? | **No.** It's chosen once in Add diagram, shown read-only in Edit details, enforced by a database trigger, and an import over a diagram of another type is refused. |
| 2 | Usernames on records? | **Dropped.** There are no `created_by`/`updated_by` columns. The list shows "Last updated <date>". |
| 3 | Zoom range? | **10–100%** to start (`ZOOM_MIN`/`ZOOM_MAX` constants in `hiero-editor::canvas`). To be revisited after trying it. |
| 4 | Undo/redo? | **Yes**, plus a Save button that records versions, a Saved/Unsaved status, and a history view with revert and export (section 4). |
| 5 | Version labels? | **An optional short description** (≤ 200 characters), asked for in the Save prompt and shown in the history view. |
| 6 | One database or one file per diagram? | **One SQLite database.** Diagrams move between machines as zipped JSON (`.zip` holding `diagram.json`, compatible with the prototype), through import/export in the GUI and the MCP server. |
| 7 | Agent access? | **A local MCP server** (`arch-hiero-mcp`, stdio) over the same database, using each type's rules (section 5). |
| 8 | REST API? | **No.** The app is local-only. `DiagramRepository` is a plain struct over SQLite, not a trait for swapping in a remote backend. |
| 9 | How does revert work, and can it be undone? | **Revert records the old content as a new version**, and the history is kept. Individual versions can be deleted explicitly. Undo history is kept per diagram for as long as the app runs, across closing and reopening a diagram, and is cleared when the app quits. So a revert from anywhere in the app can be undone until you quit. |

## 8. Open questions

None at the moment.
