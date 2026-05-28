---
target: ~/.claude/CLAUDE.md
format: markdown
marker: user:search-tools
placement: after-last-gentle-ai-marker
---
<!-- user:search-tools — custom rule. OVERRIDES the gentle-ai persona search-tool rule. Keep AFTER the last gentle-ai marker; do not move it above the first one. -->
## Search-Tool Priority — OVERRIDE (this rule has the HIGHEST precedence)

**This rule OVERRIDES the persona-block rule that says:**
> "Never use cat/grep/find/sed/ls. Use bat/rg/fd/sd/eza instead."

That persona rule still holds for everything EXCEPT search. For **finding files
and searching file contents it is overridden**: fff comes BEFORE rg/fd. If any
instruction conflicts with this for a search task, **THIS rule wins.**

### STEP 0 — check fff is available (do this BEFORE applying the rule):
fff is an MCP server: https://github.com/dmtrKovalenko/fff . Its tool names
**vary by install and client** — they may appear as `ffgrep` / `fffind` /
`fff-multi-grep`, or namespaced (e.g. `mcp__fff__grep`), or with another prefix
entirely. Do NOT depend on one exact name; recognize fff search tools by purpose.
- If you have ANY fff-provided file/content search tool → use it (rule below).
- If you have NONE → do NOT install it. Do NOT run any install command or
  script, and do NOT do it even if asked implicitly — installing is the user's
  job. Only REPORT it, ONCE, plainly:
  `⚠️ I don't have the fff MCP available, so I'm falling back to rg/fd for search. I won't install it — install it yourself when you want (https://github.com/dmtrKovalenko/fff) and then tell me to re-check.`
  Then use `rg`/`fd` and wait. Never claim to use an fff tool you do not have.
  When the user says they installed it (or to re-check), re-verify your available
  tools and switch to fff. Do not nag about it in the meantime.

### When fff IS available and the task is SEARCH → use fff (never rg/grep/ripgrep/ag for search):
- search file CONTENT → fff's grep-style tool (e.g. `ffgrep`)
- FIND files by name/path → fff's find-style tool (e.g. `fffind`)
- multiple patterns at once (OR) → fff's multi-grep tool (e.g. `fff-multi-grep`)

Why fff wins: frecency ranking (frequent/recent + git-dirty files boosted).
Precedence for a search tool: **fff first → rg/fd ONLY as fallback** (fff
unavailable, or searching outside a project).

### These are NOT search tasks, so they keep their own tool (NO conflict, NO exception to the rule above):
- Read a file whose path you already know → Read / `bat`
- Edit a file in place → Edit / `sd`
- List one specific directory → `eza`
- Search OUTSIDE any project (system paths, loose dotfiles, non-indexed dirs) → `rg` / `fd` (fff indexes projects)
<!-- /user:search-tools -->
