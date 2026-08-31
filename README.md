# Agent Rules — portable custom blocks

Declarative source of truth for custom rules injected into AI-agent config
files (Claude Code today; more agents later). Kept **outside** any
agent/harness config so it survives a full wipe-and-recreate of those tools.

**Policy lives here, in `blocks/`. The mechanism underneath — gentle-ai,
each agent's own config — is replaceable.**

## Quick start

1. Get the `agent-rules` binary for your platform — a
   [release download](#releases), or build it yourself with `mise run build`
   (see [Local development](#local-development)).
2. Preview: `agent-rules --dry-run` — shows what would change, writes
   nothing.
3. Apply: `agent-rules` — writes the blocks, backing up anything it
   overwrites first (see [Backups](#backups)).

Running it again is always safe: `agent-rules` is idempotent. A second run
reports every block unchanged.

## How it works

`agent-rules` is a dispatcher: for every block embedded from `blocks/` (see
[Block file format](#block-file-format)) it reads the frontmatter and routes
by `format`.

| Format | Behavior |
|---|---|
| `markdown` | Deterministic block replace. The block is delimited by `<!-- {marker} -->` … `<!-- /{marker} -->`. An existing block is replaced in place (self-healing duplicates collapse to one); a missing block is inserted after the **last** `<!-- /gentle-ai:… -->` line, or at end of file if there are none. It **never** writes before the first `gentle-ai:` marker and **never** edits a `gentle-ai:` block — the run aborts if a change would drop any `gentle-ai:` marker. |
| `json` | Idempotent remove/ensure on the array at a dotted `json_path` (e.g. `permissions.deny`). The block body is a spec: `{"remove": [...], "ensure": [...]}`. Entries in `remove` are dropped; entries in `ensure` are appended if absent. Every other key in the file is left untouched, so re-running converges to a fixed point. |

Backups are written before any change — see [Backups](#backups).

## Block file format

```
---
target: ~/path/to/config        # required; ~ is expanded
format: markdown                 # markdown | json
marker: user:my-rule             # markdown: the block's marker name
placement: after-last-gentle-ai-marker
---
<!-- user:my-rule -->
…content to inject, verbatim…
<!-- /user:my-rule -->
```

The body is everything **after** the closing `---`. `agent-rules` never
copies the frontmatter into the target — only the body between the
delimiters.

## Inventory

| Block file | Target | Format | What it does |
|---|---|---|---|
| `blocks/claude.search-tools.md` | `~/.claude/CLAUDE.md` | markdown | Prefer fff MCP tools for file/content search inside projects |
| `blocks/claude.language-es-es.md` | `~/.claude/CLAUDE.md` | markdown | Always answer in peninsular Castilian Spanish (overrides persona language rule) |
| `blocks/claude.settings-env-deny.md` | `~/.claude/settings.json` | json | Narrow gentle-ai's broad `.env.*` deny to specific variants |

## The `agent-rules` binary

A single self-contained executable. The blocks in `blocks/` are embedded
into it at build time (see `build.rs`), so the compiled binary is the only
file that needs to exist on a machine to run it — no `blocks/` directory
alongside it, nothing to install at runtime.

| Command | Does |
|---|---|
| `agent-rules` / `agent-rules apply` | Apply all blocks (default action) |
| `agent-rules --dry-run` | Show what would change; write nothing |
| `agent-rules --list` | List embedded blocks (name, target, format) |
| `agent-rules --version` | Print the version string |
| `agent-rules --help` | Show usage |

### Backups

Before any change, a target file's previous contents are written to
`<executable-dir>/.backups/<filename>.<epoch-seconds>.bak` — there's no
on-disk `blocks/` directory next to a compiled binary to anchor `.backups/`
to, since blocks are embedded, so it lives next to the executable itself
instead. The timestamp is Unix epoch seconds: plain std has no
dependency-free way to read the local UTC offset, and this project takes on
no dependency beyond `serde_json`.

## Local development

```sh
mise install                 # installs the Rust toolchain pinned in mise.toml
mise exec -- cargo check     # or `cargo check` directly, if your shell has mise activated
mise exec -- cargo test
mise run build                # -> dist/agent-rules-{os}-{arch}[.exe], all 4 targets, via Docker
```

`mise run build` runs `docker compose run --rm build` (see `compose.yaml`) —
nothing needs to be installed on the host beyond Docker; the cross-compile
toolchain lives entirely in the build container. It builds all 4 targets in
one run — `linux/amd64`, `darwin/amd64`, `darwin/arm64`, `windows/amd64` —
into a flat `dist/`. Named Docker volumes cache the Cargo registry and
target dir across runs, so a second `mise run build` is a warm build.

**Embedding `blocks/`:** `build.rs` scans the `blocks/` directory sitting
next to `Cargo.toml` and generates a `pub static BLOCKS: &[(&str, &str)]` of
`(filename, include_str!(...))` pairs for every `blocks/*.md` file, which
`src/blocks.rs` pulls in via `include!`. `Cargo.toml`, `src/`, `build.rs`,
and `blocks/` are all plain siblings at the repo root, so this works
identically for a native `cargo build` and for the Docker build — which
compiles straight from a read-only bind mount of that same tree, no
assembly step either way.

**Size profile:** `opt-level = "z"`, `lto = true`, `codegen-units = 1`,
`panic = "abort"`, `strip = true` in `Cargo.toml`, plus the `musl` (not
`gnu`) target for Linux, for a genuinely static binary with no runtime libc
dependency.

## Releases

Tag a semver version and push the tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

`.github/workflows/release.yml` then: checks that the tag's version matches
`Cargo.toml`, runs the same `docker compose run --rm build`, and publishes
`dist/agent-rules-*` on the GitHub Release for that tag with auto-generated
notes.

> **Gotcha:** some machines have a global `~/.gitignore` that excludes
> `mise.toml` (a common pattern, since it can hold machine-specific
> settings). If edits to `mise.toml` aren't showing up in `git status`,
> check with `git check-ignore -v mise.toml`, and force-add it with
> `git add -f mise.toml` if so.
