# Arch Hiero

A fast, native diagramming app for enterprise architecture, written in Rust with
[egui](https://github.com/emilk/egui) and wgpu. It targets macOS today, with Windows and Linux
planned.

It draws three kinds of diagram:

- **Conceptual**: actors, domains, the applications in them, components and data, and the
  arrows between them, with connectors and transformations.
- **Physical**: networks divided into zones, users, servers, databases, appliances, and
  storage, and the traffic between them with its protocol and port.
- **Deployed component**: layers of components, each with its protocols, bulleted
  description, tech stack, and instance count.

Diagrams are kept in a local SQLite database. Every change is saved as you make it; **Save**
(⌘S) records a numbered version you can go back to, export, or delete from the diagram's
history. Undo and redo (⌘Z, ⇧⌘Z) last until you quit. Diagrams can be exported as SVG,
draw.io, or a `.zip` diagram file that imports again on any machine (prototype files too).

## Run

```sh
cargo run -p hiero-app --release
```

The database lives in `~/Library/Application Support/ArchHiero/arch-hiero.sqlite3` on macOS
(set `ARCH_HIERO_DB` to use another). It starts empty; while there are no diagrams, the list
offers to load samples, one of each type.

## Agents (MCP)

`arch-hiero-mcp` is a local [MCP](https://modelcontextprotocol.io) server over stdio. Agents
can list, read, create, and edit diagrams (with each type's own rules), save and revert
versions, and import and export files. Changes appear in the app as they're made, and can be
undone there.

```sh
# From a built bundle
claude mcp add arch-hiero -- /Applications/ArchHiero.app/Contents/MacOS/arch-hiero-mcp
# Or during development
claude mcp add arch-hiero -- cargo run -q --manifest-path "$PWD/Cargo.toml" -p hiero-mcp
```

Other clients use the same command in their MCP configuration, for example `.mcp.json`:

```json
{ "mcpServers": { "arch-hiero": { "command": "/Applications/ArchHiero.app/Contents/MacOS/arch-hiero-mcp" } } }
```

## Build the app bundle

```sh
./scripts/bundle-macos.sh                 # target/bundle/ArchHiero.app
./scripts/bundle-macos.sh --sign "Developer ID Application: …"
```

Contributor and agent guidelines: [AGENTS.md](AGENTS.md). Design and specs: [docs/](docs/).

## License

[MIT](LICENSE). The bundled Inter font is under the [SIL Open Font License](crates/hiero-core/assets/fonts/OFL.txt).
