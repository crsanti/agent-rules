---
target: ~/.claude/CLAUDE.md
format: markdown
marker: user:odd-feature-doc
placement: after-last-gentle-ai-marker
---
<!-- user:odd-feature-doc — custom rule. OVERRIDES the gentle-ai ODD feature-document rules. Keep AFTER the last gentle-ai marker; do not move it above the first one. -->
## ODD Feature Document — OVERRIDE (this rule has the HIGHEST precedence)

**This rule OVERRIDES the ODD rules under `## Implementation Routing`, and the
shared contracts in `~/.claude/skills/_shared/`, that say:**
> "a two- or three-line header"
>
> "a header of two or three lines with the objective, branch, delivery
> strategy, and test runner"
>
> "`## Log` last, where `L1` is the user's original request verbatim and later
> user corrections, evidence, and decisions are appended."
>
> "Keep one feature document, not a separate plan file or topic."
>
> "Mirror the full current document and repository-relative file locator, not
> just a summary or completion notice."
>
> "The mirror contains the full document and repository-relative locator, not
> a summary."
>
> "Significant verified project discoveries can be saved with `mem_save`
> under the resolved project."
>
> "tell workers to read until `## Log`"

Everything else in ODD stays in force. This rule changes where the log lives,
what reaches Engram, and how review findings are tracked. If any instruction
conflicts with this for a feature document, **THIS rule wins.**

### Files

- `odd/tasks/<feature-name>.md` — the feature document, in this order: header,
  `## Specs`, `## Tasks`, `## Open Items`. No `## Log`. Keep the header short:
  ODD's fields, plus a `Log:` line with the log file's path and a `Next:` line
  with the next step, updated after each task.
- `odd/tasks/<feature-name>-log.md` — the log: `L1` (the user's original
  request, verbatim) and every later entry, under ODD's `## Log` content rules.
  It is a log, not a feature document. It stays in the repository with the
  feature and is NEVER mirrored to Engram.
- The Engram topic `odd/<feature-name>/tasks` mirrors the feature document,
  with its locator, never the log. If `mem_save` reports it truncated, the
  mirror is incomplete: tell the user and mark it pending, as ODD does when
  Engram is unavailable.
- Workers read the whole feature document, and the log only when they need its
  history. On resume, `Next:` says where to continue, and the end of the log
  has the details.
- A document that still has an inline `## Log`: the next time you update it,
  move the log to the log file and give its open items an `F#` (below).

### Decisions and discoveries

The log goes stale; decisions and discoveries do not. Each one gets a log entry
AND its own Engram memory, so none lives only in the log or only in Engram.

- **Decision** (`type: decision`, `topic_key: decision/<slug>`): a technical
  choice later work must respect, with its reason, that no `S#` or open item
  already holds. Requirement changes still rewrite the affected `S#`.
- **Discovery** (`type: discovery`, `topic_key: discovery/<slug>`): a verified
  fact about a tool, library, or environment that would save time in later
  work. Name the version: "Bun 1.4.2: `Bun.serve` ignores `HOST`". A test
  result is not a discovery.
- One memory per decision or discovery; never share a `topic_key`. Use these
  keys for every decision or discovery you save during feature work,
  proactive saves included. Workers do not save them: they report them to
  the parent, which saves them. Name the feature and `L#` each one came from,
  and the discovery a decision relies on.
- Search first. If one already exists under these keys, update it, also when
  a decision changes, and leave any other copy as it is. If it only exists
  under another key, update that one instead of adding a second:
  `mem_update` with its ID, setting the `topic_key` and `type` above, unless
  it holds more than this decision or discovery; then save a new one. Key
  Learnings are captured automatically, without a key: when the search finds
  one, treat it as a memory under another key. Earlier versions stay readable
  with `mem_get_observation` and `include_history: true`.
- NOT decisions: a verdict that only says to fix it or add the test (its task
  and commit are the record), and delivery choices (they stay in the header).

### Review findings

- Every finding a review or verification reports gets an `F#` when it is
  logged: numbered in order from the highest `F#` already used and never
  reused, like `S#`, `T#`, and `L#`. Keep the reviewer's ID next to it:
  `F4 (R1-001)`. Reviewer IDs restart on every review run, so they collide.
- A finding that repeats a recorded one keeps its `F#`: add the new reviewer
  ID, and do not ask the user again about one already decided. When it repeats
  another feature's item, reference that item (`app-skeleton#F4`) instead.
- After each review, every finding ends in exactly one place: fixed (cite the
  task or commit), in an `S#` (cite the `F#`), in `## Open Items`, or as a
  reference to another feature's item.
- Native review prompts are still relayed unchanged; `F#` is only for the
  feature documents.

### `## Open Items`

What is still open, so nobody has to dig it out of the log: review suggestions
and follow-ups, verifier advisories, the open blockers of a
**Needs your decision** result, and open points you raise yourself. Each item
has its `F#`:

- **Pending decisions** — the user has not decided yet.
- **Deferred** — the user accepted it for later (technical debt).
- **Kept as is** — the user chose not to change it. Always give the reason, and
  a "revisit when" condition if there is one.

Each item must make sense without the log, since the mirror has no log; a
trailing `(L#)` only points to more detail. Move an item when its state
changes, and remove it once it is done: the log keeps the history. When
closing (ODD step 7), list the pending decisions still open.

### Other features

Never edit another feature's documents while its branch is open. Record the
effect in your own ("resolves app-skeleton#F4"), and update the other
feature's documents once its branch is merged.
<!-- /user:odd-feature-doc -->
