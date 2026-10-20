# Implementation plan: Angular prototype → Rust

Goal: rebuild the Angular prototype's feature set (`../arch_hiero`) as a native Rust/egui app
that stores diagrams in SQLite. The code for each diagram type lives in its own crate, and the
code they share lives in common crates.

Diagram type specs:

- [Conceptual](diagrams/conceptual.md)
- [Physical](diagrams/physical.md)
- [Deployed component](diagrams/deployed-component.md)

---

## 1. Feature inventory (what "parity" means)

| Area | Prototype behaviour | Rust home |
|---|---|---|
| Diagram list | Sorted by name, with type, "last updated <date>", and a **Saved/Unsaved** badge with the latest version number (new in Rust). Each row has Download and Delete. Delete asks for confirmation, focused on Cancel. Status/alert messages show the outcome. | `hiero-app::pages::list` |
| Add diagram | Form with Name (required, ≤100), Type (required), and Description (≤1000). A physical diagram starts with its standard zones. Opens the editor afterwards. | `hiero-app::pages::details` |
| Edit details | The same form plus per-type colour pickers (conceptual 8, physical 12) and "Use the default colors". Shows created/updated dates. **The type is read-only after creation** (a change from the prototype). | `hiero-app::pages::details` + each type's `Colors` |
| Import / export file | A `.zip` holding `diagram.json` = `{format:"arch-hiero-diagram", version:1, diagram}`. Importing replaces the diagram with the same id, or adds it. Errors are specific and readable. | `hiero-core::file` + `hiero-store` |
| Editor page chrome | Name, type, **Saved/Unsaved status, Save, Undo, Redo, History**, Help toggle, Export SVG, Export draw.io, Edit details, Back. Shows a "couldn't save" banner when an autosave fails. | `hiero-editor::page` |
| Canvas | Fixed 2400×1600 world, 20 px dotted grid. Pan by dragging empty canvas or with arrow keys. Zoom **10–100%** from a slider, kept centred (the prototype allows 50–100%). The limits are constants, to adjust after trying it. | `hiero-editor::canvas` |
| Palette | Click adds at a free spot (inside the selected container if it fits). Drag drops where released. | `hiero-editor::palette` |
| Selection | Click to select. Shift/Ctrl/Cmd+click toggles. Shift+Space adds the focused element. Clicking empty canvas clears the selection. | `hiero-core::edit` + `hiero-editor` |
| Move | Drag, or arrow keys (Shift for bigger steps). Snaps to the grid and stays on the canvas. Groups move together. Containment is re-evaluated on drop. | same |
| Containers | Kinds declare `can_contain`. Containers grow to fit, with header, padding, and footer. A removed container's children stay. | `hiero-core::layout` |
| Resize | Top-left and bottom-right handles (or width-only), by drag or arrow keys. Minimum sizes. Stays around its children. | same |
| Delete | Delete/Backspace removes the selection, unless focus is in a text field. | same |
| Properties panel | Collapsible. Per-type fields. Applied while valid. Escape returns focus to the canvas. | per-type `ui` + `hiero-editor::properties` |
| Arrows | Drag from the connect handle, or "Add arrow to". Facing-side routing with even spread. Select, edit, delete. | `hiero-core::routing` + `hiero-editor::arrows` |
| Help | Per-type help text above the canvas | per-type `HELP` constant |
| Exports | SVG cropped to the content. draw.io with containers and attached connectors, and a name/description heading. | `hiero-core::export` + per-type overrides |
| Theme | System / Light / Dark picker, remembered | `hiero-app::theme` |
| Accessibility | Every change announced in an ARIA live region, full keyboard operation, WCAG AA contrast | AccessKit (egui) + `describe()` hooks |
| Autosave | Each finished change saves straight away to the current record | `hiero-store`, called by `hiero-editor` |
| **Undo / redo** (new) | ⌘Z / ⇧⌘Z (Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y elsewhere) and toolbar buttons. Covers every content change in the editor. | `hiero-core::history` + `hiero-editor` |
| **Versions** (new) | A Save button (⌘S) records the current diagram as a new numbered version. The status shows *Saved* when the current record matches the latest version, otherwise *Unsaved*. | `hiero-core::history` + `hiero-store` + `hiero-editor` |
| **History view** (new) | Lists versions with number and date. Each version can be reverted to, or exported as .zip, SVG, or draw.io. | `hiero-app::pages::history` |

Out of scope for now, but the design should make these easy later: systems catalogue entities
and REST sync. Usernames are dropped: the Rust version records dates only, with no "by <user>".

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
    text.rs              trait MeasureText + an estimating implementation for tests
    export/svg.rs        Scene → SVG string, cropped to the content
    export/drawio.rs     mxfile writer: cells, containers, edges, heading
    file.rs              .zip diagram file read/write. Same format as the prototype.
  hiero-store/           SQLite (rusqlite, bundled). Depends on hiero-core only.
    schema/              migrations (rusqlite_migration, user_version)
    repo.rs              DiagramRepository: list, get, create, update_meta, save_content,
                         delete, import (upsert by id), save_version, list_versions,
                         get_version, revert_to_version, status
    seed.rs              First-run sample diagrams (ported from public/data/diagrams.json)
  hiero-editor/          egui. Depends on hiero-core.
    canvas.rs            World↔screen transform, grid, pan/zoom, hit-testing, focus/keys
    paint.rs             Scene → egui::Painter
    engine.rs            Editor<T: DiagramType + DiagramView>: input → hiero_core::edit ops
    arrows.rs            Connect handle, pending arrow, arrow selection
    palette.rs, properties.rs, page.rs (chrome), widgets.rs (text field w/ validation, etc.)
    text.rs              MeasureText built on egui fonts (the same numbers on screen and in exports)
  diagrams/
    hiero-conceptual/    model.rs (ConceptualKind, ConceptualArrow, Colors), rules.rs (impl
                         DiagramType), scene.rs, drawio.rs, ui.rs (impl DiagramView), help.rs
    hiero-physical/      same layout. rules.rs holds the zone/network stacking logic.
    hiero-deployed/      same layout, plus tech_stack.rs and assets/tech/*.png, and its own
                         svg.rs and drawio.rs
  hiero-app/             The binary: navigation, list/details pages, theme, the registry of
                         diagram types, and the store thread
```

**Allowed dependencies** (CI checks these with `cargo metadata`):

```
hiero-app ──► hiero-store ──► hiero-core
    │                            ▲
    ├──► hiero-editor ───────────┤
    └──► hiero-{conceptual,physical,deployed} ──► hiero-editor, hiero-core
```

Inside a diagram crate, only `ui.rs` (and its submodules) may `use egui`. A test in each crate
checks this by searching the source files.

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

`hiero-editor` provides an object-safe `trait DiagramEditor { fn ui(&mut self, ui) -> Outcome;
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
DiagramView`, plus one line here. Nothing else changes.

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
  `INSERT` them all again with prepared statements, then update `updated_at` and
  `content_hash`.
  Diagrams have hundreds of rows at most, so a full rewrite is simpler and safer than diffing.
  If profiling says otherwise, switch to diffing.
- **Import** (`.zip`) upserts by id: an existing diagram is replaced completely, and fields
  missing from the file are cleared. The timestamps are set fresh, as in the prototype. Any
  `createdBy`/`updatedBy` in prototype files is ignored. If the existing diagram has a
  different type, the import is refused: "“CRM” is a conceptual diagram, but the file holds a
  physical diagram."
- **Seed:** on the first run (an empty `diagrams` table), insert the prototype's sample
  diagrams from an embedded copy of `diagrams.json`, each with version 1.

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
- **Save** runs in one transaction: read the current record, build its snapshot and hash, and
  insert version `max(version) + 1` with the description. It does nothing if the hash already
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
- The stack belongs to the open editor session and lives in memory only. It's cleared when the
  diagram is closed. Saving a version doesn't clear it.

### History view

Opened from **History** in the editor, or from a diagram's row in the list. It's a page that
lists the versions, newest first:

| Version | Saved | Description | |
|---|---|---|---|
| v4 · *matches current* | 20 Oct 2026, 17:42 | Before the network redesign | Revert · Export ▾ (.zip / SVG / draw.io) |
| v3 | 18 Oct 2026, 09:10 | – | Revert · Export ▾ |

- **Revert to vN** replaces the current record with version N's snapshot. Later versions are
  kept, so history is never rewritten. Afterwards the status is *Unsaved* (unless N is the
  latest version), and the next Save records it as a new version.
- Before reverting, a dialog asks for confirmation, with Cancel focused:
  - If the status is Unsaved: "Revert to v3? Your current changes haven't been saved as a
    version and will be lost." The choices are **Save as v5, then revert**, **Revert**, and
    **Cancel**. *Save as v5, then revert* has the same optional Description field, pre-filled
    with "Before reverting to v3" and editable.
  - If Saved: "Revert to v3?" The choices are **Revert** and **Cancel**.
- When the history page was opened from the editor, the revert also goes onto that session's
  undo stack, so ⌘Z brings the previous state back.
- **Export vN** writes the snapshot as:
  - `.zip`: the standard diagram file. It keeps the diagram's id, so importing it later
    restores over that diagram.
  - `.svg` / `.drawio`: rendered from the snapshot by the type's `scene` / `to_drawio`. Those are
    UI-free, so no editor needs to be open.

  Files are named "<name> v3.<ext>".
- Versions are deleted only with their diagram, through `ON DELETE CASCADE`.

---

## 5. Phases

Each phase ends with `cargo fmt`, clippy, and tests clean, and a runnable app.

| # | Phase | Deliverables | Done when |
|---|---|---|---|
| 0 | **Restructure** | Create the crates above and move the current scaffold into them. Add the dependency-rule CI check. Update AGENTS.md. | Workspace builds. The existing demo canvas still runs. |
| 1 | **Core model + file format** | Port `geometry`, `ids`, `audit`, `Element`/`Connection`, `KindInfo`, `file.rs` (zip). Port the prototype's `diagram-file.spec.ts` and `zip.spec.ts` cases. | A prototype-exported `.zip` round-trips through Rust unchanged. |
| 2 | **Store + versions** | `hiero-store` with migration 1 (including `diagram_versions` and the type trigger), the repository, `save_version` (with optional description)/`list_versions`/`revert_to_version`/`status`, the seed, and the store thread. `hiero-core::history` (snapshot, hash, `UndoStack`). Tests on an in-memory DB. | CRUD, import (including refusing a type mismatch), versioning, and the Saved/Unsaved status are all tested. Cascade delete works. Changing the type is rejected. |
| 3 | **App shell + history view** | List page with Saved/Unsaved badges. Add/Edit details form with validation, where the type is chosen only when adding. Delete confirmation, import/export zip (`rfd` file dialogs), theme picker, outcome messages. History page with revert and .zip export. | Every non-editor flow works against SQLite. |
| 4 | **Layout + routing ports** | `layout.rs`, `routing.rs`, `color.rs`, and `edit.rs` with the prototype's `diagram-layout.spec.ts` and `diagram-connections.spec.ts` cases ported as Rust tests. | Rust and TypeScript give the same results on the ported cases. |
| 5 | **Generic editor engine** | Canvas (fixed world, grid, pan, zoom 10–100%), Scene painter, palette (click and drag), selection and multi-select, move, snap, containers, resize handles, delete, properties panel frame, help panel, autosave, page chrome, **Save button + status, undo/redo**. Uses a throwaway "test type" crate. | Works with a minimal `DiagramType`. Keyboard-only use works. |
| 6 | **Conceptual** | `hiero-conceptual` (spec: [conceptual.md](diagrams/conceptual.md)). Shared arrow tool lands here. | Its parity checklist is ticked. |
| 7 | **Physical** | `hiero-physical` (spec: [physical.md](diagrams/physical.md)). Port `physical-diagram.spec.ts`. | Its parity checklist is ticked. |
| 8 | **Deployed component** | `hiero-deployed` (spec: [deployed-component.md](diagrams/deployed-component.md)), tech-stack extraction script, `deployed-component-layout.spec.ts` ported. | Its parity checklist is ticked. |
| 9 | **Exports** | Scene→SVG and the shared draw.io writer, plus the deployed component writers. SVG and draw.io export of history versions. Snapshot tests (`insta`) against SVG and draw.io output from the prototype for the seed diagrams. | Exported files open in a browser and in draw.io and match the prototype. |
| 10 | **Accessibility + packaging** | AccessKit labels and live announcements from `describe()`. macOS `.app` bundle (`cargo-packager`), icon, and code signing notes. | VoiceOver can operate the editor. The `.app` installs and runs. |

Phases 6–8 depend only on phase 5, so they can run in parallel, one agent or developer per
diagram crate. This is the payoff of keeping the crates separate.

### Parity testing approach

- **Port the spec files.** The prototype has Vitest specs for layout, connections, zip, file,
  physical, deployed layout, SVG, and draw.io. Port their cases as Rust unit tests in the crate
  that owns the code.
- **Golden fixtures.** Run the prototype once to export SVG, draw.io, and zip for every seed
  diagram, then commit them under `tests/fixtures/prototype/`. Rust snapshot tests compare
  against them, normalising number formatting.
- **Text measurement.** Bundle one font (such as Inter) and use it on screen and in exports. The
  same `MeasureText` then gives the same widths everywhere. Tests use the estimating measurer.

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

---

## 6. Decisions

| # | Question | Decision |
|---|---|---|
| 1 | Can a diagram's type be changed? | **No.** It's chosen once in Add diagram, shown read-only in Edit details, enforced by a database trigger, and an import over a diagram of another type is refused. |
| 2 | Usernames on records? | **Dropped.** There are no `created_by`/`updated_by` columns. The list shows "Last updated <date>". |
| 3 | Zoom range? | **10–100%** to start (`ZOOM_MIN`/`ZOOM_MAX` constants in `hiero-editor::canvas`). To be revisited after trying it. |
| 4 | Undo/redo? | **Yes**, plus a Save button that records versions, a Saved/Unsaved status, and a history view with revert and export (section 4). |
| 5 | Version labels? | **An optional short description** (≤ 200 characters), asked for in the Save prompt and shown in the history view. |

## 7. Open questions

1. **One database or one file per diagram?** This plan uses one database, with `.zip` files for
   sharing. If teams later need to share via git, a per-diagram file format could be added
   beside it.
2. **REST later.** The prototype's repository was a stand-in for a future API. Keep
   `DiagramRepository` a trait, so a remote implementation can replace SQLite or sync with it.
3. **Redo after revert.** A revert done outside an editor session, from the list's history
   link, can't be undone with ⌘Z. It can only be reverted again. This seems fine, because
   every saved version stays available.
