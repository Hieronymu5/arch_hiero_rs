# AGENTS.md

Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, etc.) and humans working on this repo.

## Project

**Arch Hiero** is a native desktop app for creating diagrams. It targets **macOS first**; Windows and Linux will come later, so keep code cross-platform.

- **Language:** Rust, edition 2024, toolchain pinned in `rust-toolchain.toml` (currently 1.99.1).
- **UI:** [egui](https://github.com/emilk/egui) / `eframe` 0.37 with the **wgpu** renderer (Metal on macOS, DX12/Vulkan on Windows, Vulkan/GL on Linux). It is GPU-accelerated, immediate-mode, and pure Rust.
- **No JavaScript/web UI frameworks.** Don't add Tauri, Electron, webviews, or JS tooling.

## Layout

```
Cargo.toml                 workspace: shared deps, lints, profiles
crates/
  hiero-core/              UI-free: geometry, model, the editing engine (`engine.rs`, the
                           `DiagramType` trait), layout, arrow routing, scenes, SVG and
                           draw.io export, the .zip file format, history and hashing,
                           text measurement with the bundled Inter font
  hiero-store/             SQLite: diagrams, elements, connections, versions; migrations
  hiero-editor/            egui: scene painter, canvas, palette, interaction, `DiagramView`
  diagrams/
    hiero-conceptual/      model.rs, rules.rs, scene.rs, arrows.rs (UI-free) + ui.rs
    hiero-physical/        model.rs, rules.rs (networks and zones), scene.rs + ui.rs
    hiero-deployed/        model.rs, layout.rs, rules.rs, scene.rs, drawio.rs,
                           tech_stack(_data).rs + assets/tech/*.png (generated) + ui.rs
  hiero-registry/          UI-free table of types (`TypeOps`) and agent edits (`EditOp`)
  hiero-app/               the GUI binary `arch-hiero`: pages, store thread, dev hooks
  hiero-mcp/               the MCP server binary `arch-hiero-mcp` (stdio)
scripts/
  bundle-macos.sh          builds target/bundle/ArchHiero.app with both binaries
  extract-tech-stack.py    regenerates the tech stack from the prototype
docs/                      each diagram type's spec
```

## Roadmap and specs

This app is a rewrite of the Angular prototype in `../arch_hiero`, with diagrams stored in
SQLite. Read this before working on diagrams:

- `docs/diagrams/{conceptual,physical,deployed-component}.md`: the feature spec and parity
  checklist for each diagram type. Tick a checklist item when it's done.

Crate boundaries:

- `hiero-core` (UI-free shared model, layout, routing, scene, exports, file format) ←
  `hiero-store` (SQLite) and `hiero-editor` (generic egui engine) ← `diagrams/hiero-*` (one
  crate per diagram type) ← `hiero-app`.
- **Diagram crates never depend on each other.** When a second type needs some code, move it
  down into `hiero-core` (UI-free) or `hiero-editor` (egui).
- In a diagram crate, only the `ui` module may use `egui`. The model, rules, scene, and draw.io
  code stay UI-free and unit-tested.
- `hiero-store` knows nothing about specific diagram types. Type-specific fields go in the JSON
  `props` columns, defined by each type's `Props` struct (flattened into the element's JSON
  next to its fieldless `Kind` enum). Keep that JSON the same as the
  prototype's (camelCase, kebab-case kind tags), so prototype `.zip` files import unchanged.
- Diagram crates put `ui` (and their `hiero-editor` dependency) behind a default `ui` feature.
  `hiero-registry` and the MCP server use them with `default-features = false`.
  `cargo tree -p hiero-mcp -i egui` must find nothing.
- **One SQLite database** is shared by the GUI (`arch-hiero`) and the local MCP server
  (`arch-hiero-mcp`, stdio). Every write checks `revision` for conflicts.
  Diagrams move between machines only as zipped JSON (`.zip` holding `diagram.json`).
- MCP edits must go through `hiero-registry`, so each type's rules apply. Never write element
  rows directly from `hiero-mcp`. When you add or change a feature, update the matching MCP tool
  and its schema too.
- A diagram's **type is fixed at creation**. Never add a code path that changes it. The
  database enforces this with a trigger.
- No usernames anywhere: records carry dates only.
- **Two kinds of save:** every change autosaves to the current record. The Save button records
  an immutable version in `diagram_versions`, with an optional description (≤ 200 characters) asked for in the Save prompt. The status is *Saved* when the current content
  hash equals the latest version's hash. Snapshot types must serialise deterministically, so
  use `Vec`/`BTreeMap` and never `HashMap`, or the hash breaks.
- **History is append-only, except for an explicit Delete.** Reverting records the old content
  as a *new* version. Version numbers come from `diagrams.next_version` and are never reused.
  Undo/redo changes only the current record, never versions. Undo stacks are per diagram and
  in memory: they survive closing and reopening a diagram, and are cleared when the app quits.

## Commands

```sh
cargo run -p hiero-app                        # run the app (debug)
cargo run -p hiero-app --release              # run optimized
cargo run -p hiero-mcp -- --db /tmp/x.sqlite3 # run the MCP server on a scratch database
cargo test --workspace                        # tests
cargo fmt --all                               # format
cargo clippy --workspace --all-targets --examples -- -D warnings   # lint (must be clean)
./scripts/bundle-macos.sh                     # build ArchHiero.app
```

Before you call a change done, run `cargo fmt --all`, the clippy command above, and `cargo test --workspace`. CI runs the same checks, plus the dependency-rule checks in `.github/workflows/ci.yml`.

### Checking the app without clicking

The app reads these environment variables (see `crates/hiero-app/src/devtools.rs`):

- `ARCH_HIERO_DB=<path>`: use a scratch database instead of the real one. Always do this when testing.
- `ARCH_HIERO_OPEN=<diagram name>` or `ARCH_HIERO_NEW=<type>`: open or create a diagram on start.
- `ARCH_HIERO_SELECT=<element name>`: select an element.
- `ARCH_HIERO_SCREENSHOT=<file.png>` (with optional `ARCH_HIERO_SCREENSHOT_AFTER=<seconds>`): save a screenshot of the window and quit. Look at it.

`cargo run -p hiero-mcp --example agent_edit -- <db> <diagram name>` edits a diagram from another process, to check the app's live reload.

## Architecture rules

1. **Model/view split.** Everything that decides where things go (rules, layout, routing, scenes, exports) is UI-free and unit-tested: `hiero-core` and the non-`ui` modules of each diagram crate. Only `hiero-editor`, the diagram crates' `ui` modules, and `hiero-app` touch egui.
2. **Every change goes through the engine.** Mutations call `Engine` methods (or a type's hooks), which keep the layout, arrange the elements by the type's rules, and count changes. In the editor, wrap them in `Editor::act` so they're saved and undoable.
3. **Render once, draw twice.** A type builds a `Scene`; the editor paints it and the SVG export serialises it. Don't draw an element's look in egui directly.
4. **Performance.** Paint shapes, not widgets; paths are tessellated once and cached (`hiero-editor::paint`). Rebuild the scene only when the content changes or while dragging.
5. **Coordinates.** The model stores canvas (world) pixels. Only `paint::Transform` converts to and from the screen.
6. **Text.** Measure with `hiero_core::text` (harfrust and the bundled Inter font, the same as egui draws), never with egui's layout, so exports and the MCP server agree with the screen.
7. **Cross-platform.** Wrap platform-specific code in `#[cfg(target_os = "...")]`. Use `egui::Modifiers::COMMAND` for shortcuts, never a hard-coded Cmd key.
8. **Safety.** `unsafe_code` is forbidden workspace-wide.

## Conventions

- Lints: `clippy::pedantic` is on (warn) for the whole workspace and CI treats warnings as errors. A few noisy pedantic lints are allowed in the root `Cargo.toml`. Otherwise fix the lint rather than `#[allow]`ing it; if you must allow one, add a short reason.
- Add dependencies to `[workspace.dependencies]` in the root `Cargo.toml`, then reference them with `dep.workspace = true`.
- Keep egui crate versions in lockstep (`egui`, `eframe`, and any `egui_*` add-ons must share a minor version).
- Check the egui API against the **0.37** version. It changed a lot between releases: e.g. `App::ui(&mut self, ui, frame)` replaced `App::update`, `egui::Panel::top/left/right` replaced `TopBottomPanel`/`SidePanel`, `Id::unique`/`IdSalt` replaced `Id::new`, and `egui::Role` replaced `WidgetType`. Read the sources in `~/.cargo/registry/src/*/egui-0.37*/` when unsure. The same goes for `rmcp` 3.6.
- Generated files: `crates/diagrams/hiero-deployed/src/tech_stack_data.rs` and its `assets/tech/` come from `scripts/extract-tech-stack.py`; `hiero-core/src/export/stencils.rs` holds draw.io stencils copied from the prototype.
- Commits: small, following [Conventional Commits](https://www.conventionalcommits.org/), with
  lower-case imperative subjects (`feat(editor): add edge selection`, `fix(app): clamp zoom`).
  The `commit` skill in `.agents/skills/commit/` has the types, scopes, and steps.
- Skills: put agent skills in `.agents/skills/<name>/SKILL.md`, following the open
  [Agent Skills](https://agentskills.io/specification) format, so any agent can use them.
  Claude Code finds them through a symlink in `.claude/skills/`, so add one for each new skill.

## Upgrading Rust / egui

- Rust: bump `channel` in `rust-toolchain.toml` and `rust-version` in the root `Cargo.toml` together.
- egui: bump `egui` and `eframe` together, then build and fix any API changes.
