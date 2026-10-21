---
name: commit
description: Commit the current changes to git with a Conventional Commits message (type(scope): subject), after running this repo's fmt, clippy, and test checks. Use when asked to commit, make a commit, or save work to git.
compatibility: Needs git and the Rust toolchain pinned in rust-toolchain.toml, with shell access.
---

# Commit with Conventional Commits

Commit the work in progress as one or more commits whose messages follow
[Conventional Commits 1.0](https://www.conventionalcommits.org/en/v1.0.0/).

## 1. Look before you stage

- Run `git status --short`, `git diff`, and `git diff --staged` to see what changed.
- Stage only what belongs to the change. Leave untracked files you didn't create in this session
  alone, and mention them, unless the user says to include them.
- Never commit secrets, scratch files, `target/`, or local databases (`*.sqlite3`).
- If the changes are several unrelated things, propose splitting them into one commit each (stage
  with `git add <paths>`). If the user wants a single commit, use it, and give the subject the
  type of the most significant change.

## 2. Check

Run the same checks as CI (see AGENTS.md) and only commit if they pass:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --examples -- -D warnings
cargo test --workspace
```

If `cargo fmt` changes files, include them. If clippy or a test fails, stop and report it with the
output. Don't commit with a known failure, and never use `--no-verify`.

Skip the checks only when the commit changes no Rust code or Cargo files (such as docs alone),
and say you skipped them.

## 3. Branch

If you're on `main` and the user hasn't said to commit there, create a branch first, named after
the change in kebab-case, such as `feat/edge-selection` or `fix/zoom-clamp`.

## 4. Write the message

```
<type>(<scope>): <subject>

<body>

<footers>
```

**Type**, one of:

| Type | For |
|---|---|
| `feat` | A new feature or a change in behaviour users see |
| `fix` | A bug fix |
| `refactor` | A code change that neither fixes a bug nor adds a feature |
| `perf` | A performance improvement |
| `test` | Adding or correcting tests only |
| `docs` | Documentation only (`docs/`, AGENTS.md, comments) |
| `style` | Formatting only, with no change in meaning |
| `build` | Cargo, dependencies, the toolchain, or `scripts/bundle-macos.sh` |
| `ci` | `.github/workflows` |
| `chore` | Anything else, such as regenerated files |

**Scope** (optional) is the area changed, from the crate it touches most:

| Scope | Crate or area |
|---|---|
| `core` | `hiero-core` |
| `store` | `hiero-store` |
| `editor` | `hiero-editor` |
| `conceptual`, `physical`, `deployed` | the diagram crates |
| `registry` | `hiero-registry` |
| `app` | `hiero-app` |
| `mcp` | `hiero-mcp` |
| `scripts` | `scripts/` |

Leave the scope out when a change spans several areas.

**Subject:**

- Imperative mood and lower case, as in `feat(physical): draw names with no background`.
- No full stop at the end. Keep the whole first line within 72 characters.
- Say what changes for the user or the code, not how. Avoid vague words such as "update" or
  "misc".

**Body** (leave it out only for trivial changes): explain what was wrong or missing and why this
change, in plain sentences wrapped at 72 characters. A short bullet list is fine for a
multi-part commit.

**Footers:**

- A breaking change, such as to the stored JSON, the `.zip` file format, the SQLite schema, or an
  MCP tool's schema: put `!` after the type or scope (`feat(store)!: ...`) and add a
  `BREAKING CHANGE: <what breaks and how to migrate>` footer.
- `Refs: #123` or `Fixes: #123` when there's an issue.
- End with any attribution trailer the session's instructions ask for.

## 5. Commit

Pass the message through a heredoc, so the formatting survives:

```sh
git commit -F - <<'EOF'
fix(registry): describe added arrows with their given protocol

The message for an arrow added by an agent was built before its props
were applied, so it named the target's default protocol and port
instead of the ones given.
EOF
```

Then show the result with `git log --oneline -1` and `git status --short`, and report the hash,
the subject, the branch, and anything you left uncommitted.

Don't push, amend, rebase, or force anything unless the user asks.
