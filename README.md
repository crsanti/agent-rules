# Agent Rules — portable custom blocks

Declarative source of truth for custom rules I want injected into AI-agent
config files (Claude Code today; more agents later). Kept **outside** any
agent/harness config so it survives a full wipe-and-recreate of those tools.

**Policy lives here. The mechanism (gentle-ai, each agent) is replaceable underneath.**

## Usage

After recreating agent config from scratch, open any agent and tell it:

> Apply the rules in `~/.agent-rules`: run `python3 ~/.agent-rules/apply.py`.

That command is the whole job. It is deterministic and idempotent — running it
again never duplicates anything. Preview first with `--dry-run`:

```sh
python3 ~/.agent-rules/apply.py --dry-run   # show what would change
python3 ~/.agent-rules/apply.py             # apply
```

No third-party packages required (Python 3 standard library only).

## How it works

`apply.py` is a dispatcher: for each file in `blocks/` it reads the frontmatter
and routes by `format`.

- **markdown** — fully deterministic. The block is delimited by
  `<!-- {marker} -->` … `<!-- /{marker} -->`. The script replaces the existing
  block in place (self-healing duplicates) or, if absent, inserts it **after the
  last** `<!-- /gentle-ai:… -->` line (or at end of file if there are none). It
  **never** writes before the first gentle-ai marker and **never** edits a
  gentle-ai block; the run aborts if a change would drop any gentle-ai marker.
- **json** — pluggable, not implemented yet. Reported as `skipped`, never
  guessed. (Will deep-merge keys with a real JSON parser when added.)

Backups are written to `.backups/` before any change.

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

The body is everything **after** the closing `---`. The script never copies the
frontmatter into the target — only the body between the delimiters.

## Manual fallback (only if Python 3 is unavailable)

For each file in `blocks/`, read its frontmatter, then for a **markdown** target:

1. Search the target for a line matching the marker name, e.g. any line like
   `<!-- user:my-rule … -->` (it may carry trailing description text), and its
   closing `<!-- /user:my-rule -->`.
2. **If found** → replace everything from the opening line to the closing line
   (inclusive) with the block body. If more than one such block exists, keep one
   and delete the rest.
3. **If not found** → insert the block body after the **last** line matching
   `<!-- /gentle-ai:… -->`. If there is no such line, append at end of file.
4. **Never** place the block before the first `<!-- gentle-ai:… -->` marker, and
   **never** add, remove, or edit any `<!-- gentle-ai:… -->` block.
5. Verify: the marker now appears exactly once and the count of `gentle-ai:`
   markers is unchanged.

## Inventory

| Block file | Target | Format | What it does |
|---|---|---|---|
| `blocks/claude.search-tools.md` | `~/.claude/CLAUDE.md` | markdown | Prefer fff MCP tools for file/content search inside projects |
| `blocks/claude.language-es-es.md` | `~/.claude/CLAUDE.md` | markdown | Always answer in peninsular Castilian Spanish (overrides persona language rule) |
