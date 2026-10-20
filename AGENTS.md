# AGENTS.md

Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, etc.) and humans working on this repo.

## Project

**Arch Hiero** is a native desktop app for creating diagrams. It targets **macOS first**; Windows and Linux will come later, so keep code cross-platform.

- **Language:** Rust, edition 2024, toolchain pinned in `rust-toolchain.toml` (currently 1.99.1).
- **UI:** [egui](https://github.com/emilk/egui) / `eframe` 0.37 with the **wgpu** renderer (Metal on macOS, DX12/Vulkan on Windows, Vulkan/GL on Linux). It is GPU-accelerated, immediate-mode, and pure Rust.
- **No JavaScript/web UI frameworks.** Don't add Tauri, Electron, webviews, or JS tooling.

## Layout

```
Cargo.toml              # workspace: shared deps, lints, profiles
crates/
  hiero-core/           # UI-independent diagram model (nodes, edges, hit-testing). No UI deps.
  hiero-app/            # eframe desktop app; binary is `arch-hiero`
    src/main.rs         # window setup / entry point
    src/app.rs          # top-level App state, panels, persistence
    src/canvas.rs       # pan/zoom diagram canvas, drawn with egui::Painter
```

## Commands

```sh
cargo run -p hiero-app              # run the app (debug)
cargo run -p hiero-app --release    # run optimized
cargo test --workspace              # tests
cargo fmt --all                     # format
cargo clippy --workspace --all-targets -- -D warnings   # lint (must be clean)
```

Before you call a change done, run `cargo fmt --all`, the clippy command above, and `cargo test --workspace`. CI runs the same checks.

## Architecture rules

1. **Model/view split.** Diagram data and logic (geometry, hit-testing, editing operations, serialization) go in `hiero-core`. It must **not** depend on `egui`, `eframe`, or any windowing/rendering crate. `hiero-app` handles input and drawing only.
2. **Unit-test `hiero-core`.** Every new model operation gets a test. UI code is thin enough that it doesn't need tests.
3. **Performance.**
   - Draw diagrams with `egui::Painter` shapes, not one widget per node.
   - Cull anything outside the visible rect before painting.
   - Don't allocate per frame in hot paths when you can avoid it.
   - Large diagrams will eventually need a spatial index in `hiero-core`; don't do linear scans in new hot paths.
   - Dependencies are built with `opt-level = 2` even in dev builds, so debug runs stay responsive.
4. **Coordinates.** The model stores *world* coordinates (`hiero_core::Point`). Only `Canvas::to_screen` / `Canvas::to_world` convert between world and screen space.
5. **Persistence.** App state is saved through eframe's `persistence` feature (serde/RON). New persisted fields need `#[serde(default)]` so older saved state still loads. Mark transient UI state `#[serde(skip)]`.
6. **Cross-platform.** Wrap platform-specific code in `#[cfg(target_os = "...")]`. Use `⌘/Ctrl` via `egui::Modifiers::command`, never a hard-coded Cmd key.
7. **Safety.** `unsafe_code` is forbidden workspace-wide.

## Conventions

- Lints: `clippy::pedantic` is on (warn) for the whole workspace and CI treats warnings as errors. Fix the lint rather than `#[allow]`ing it. If you must allow one, add a short comment saying why.
- Add dependencies to `[workspace.dependencies]` in the root `Cargo.toml`, then reference them with `dep.workspace = true`.
- Keep egui crate versions in lockstep (`egui`, `eframe`, and any `egui_*` add-ons must share a minor version).
- Check the egui API against the **0.37** version. It changed a lot between releases: e.g. `App::ui(&mut self, ui, frame)` replaced `App::update`, and `egui::Panel::top/left/right` replaced `TopBottomPanel`/`SidePanel`. Read the sources in `~/.cargo/registry/src/*/egui-0.37*/` when unsure.
- Commits: small, imperative subject lines (`Add edge selection`, `Fix zoom clamp`).

## Upgrading Rust / egui

- Rust: bump `channel` in `rust-toolchain.toml` and `rust-version` in the root `Cargo.toml` together.
- egui: bump `egui` and `eframe` together, then build and fix any API changes.
