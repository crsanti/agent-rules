#!/usr/bin/env python3
"""Deterministically apply ~/.agent-rules custom blocks to agent config files.

Usage:
    python3 apply.py [--dry-run] [--rules-dir DIR]

It reads every block file under <rules-dir>/blocks/ and injects each one into
its target config file according to the block's frontmatter. The whole point is
to need ZERO judgment from the caller: a model only has to run this command.

Determinism, per target format:
  - markdown: idempotent text op bounded by unique
    <!-- {marker} --> ... <!-- /{marker} --> delimiters. Repeatable; self-heals
    duplicate blocks; never touches gentle-ai marker blocks.
  - json: not implemented yet (pluggable) -> reported as skipped, never guessed.

Safety: a backup is written under <rules-dir>/.backups/ before any change, and
the run aborts if applying a block would reduce the number of gentle-ai markers.
"""

from __future__ import annotations

import argparse
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path

RULES_DIR = Path(__file__).resolve().parent


# --- block file parsing (no external deps; simple key: value frontmatter) ---

def parse_block_file(path: Path) -> tuple[dict, str]:
    """Return (frontmatter, body). Body is everything after the closing '---',
    verbatim, including the block's own <!-- marker --> delimiters."""
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=True)
    if not lines or lines[0].strip() != "---":
        raise ValueError(f"{path.name}: must start with '---' frontmatter")
    fm: dict[str, str] = {}
    body_start = None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            body_start = i + 1
            break
        line = lines[i].strip()
        if not line or line.startswith("#"):
            continue
        if ":" not in line:
            raise ValueError(f"{path.name}: bad frontmatter line: {line!r}")
        key, _, value = line.partition(":")
        fm[key.strip()] = value.strip()
    if body_start is None:
        raise ValueError(f"{path.name}: frontmatter not closed with '---'")
    body = "".join(lines[body_start:]).strip("\n")
    return fm, body


# --- markdown handler -------------------------------------------------------

def _line_res(marker: str):
    m = re.escape(marker)
    # Opening line tolerates extra text after the marker name (e.g. a comment),
    # so a descriptive marker still matches. Closing line is exact.
    open_re = re.compile(r"^\s*<!--\s*" + m + r"\b.*-->\s*$")
    close_re = re.compile(r"^\s*<!--\s*/" + m + r"\s*-->\s*$")
    return open_re, close_re


def _block_line_ranges(lines, open_re, close_re):
    """Inclusive (start, end) line ranges for each existing block."""
    ranges, i, n = [], 0, len(lines)
    while i < n:
        if open_re.match(lines[i]):
            j = i + 1
            while j < n and not close_re.match(lines[j]):
                j += 1
            ranges.append((i, j if j < n else i))
            i = (j + 1) if j < n else (i + 1)
        else:
            i += 1
    return ranges


def _last_anchor_index(lines):
    anchor = re.compile(r"^\s*<!--\s*/gentle-ai:.*-->\s*$")
    idx = None
    for i, ln in enumerate(lines):
        if anchor.match(ln):
            idx = i
    return idx


def _count_gentle(text: str) -> int:
    return len(re.findall(r"(?m)^\s*<!--\s*/?gentle-ai:", text))


def apply_markdown(target: Path, body: str, marker: str, placement: str,
                   dry_run: bool) -> dict:
    existed = target.exists()
    original = target.read_text(encoding="utf-8") if existed else ""
    gentle_before = _count_gentle(original)

    open_re, close_re = _line_res(marker)
    lines = original.splitlines()
    block_lines = body.split("\n")
    ranges = _block_line_ranges(lines, open_re, close_re)

    if ranges:
        first_start = ranges[0][0]
        for (s, e) in sorted(ranges, key=lambda r: r[0], reverse=True):
            del lines[s:e + 1]
        lines[first_start:first_start] = block_lines
        action = "updated" if len(ranges) == 1 else "updated+deduped"
    else:
        if placement == "after-last-gentle-ai-marker" and \
                (anchor := _last_anchor_index(lines)) is not None:
            lines[anchor + 1:anchor + 1] = [""] + block_lines
        else:
            if lines and lines[-1].strip() != "":
                lines.append("")
            lines += block_lines
        action = "inserted"

    new_text = "\n".join(lines)
    if not existed or original.endswith("\n"):
        new_text += "\n"

    if _count_gentle(new_text) < gentle_before:
        raise RuntimeError(
            f"SAFETY ABORT: would drop gentle-ai markers "
            f"({gentle_before}->{_count_gentle(new_text)}) in {target}")

    if new_text == original:
        return {"target": str(target), "marker": marker, "action": "unchanged"}
    if not dry_run:
        if existed:
            _backup(target)
        _atomic_write(target, new_text)
    return {"target": str(target), "marker": marker,
            "action": action + (" (dry-run)" if dry_run else "")}


# --- io helpers -------------------------------------------------------------

def _backup(target: Path) -> None:
    bdir = RULES_DIR / ".backups"
    bdir.mkdir(exist_ok=True)
    ts = datetime.now().strftime("%Y%m%d-%H%M%S")
    shutil.copy2(target, bdir / f"{target.name}.{ts}.bak")


def _atomic_write(path: Path, text: str) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


# --- dispatcher -------------------------------------------------------------

def apply_block(block_file: Path, dry_run: bool) -> dict:
    fm, body = parse_block_file(block_file)
    if "target" not in fm:
        raise ValueError(f"{block_file.name}: missing 'target' in frontmatter")
    target = Path(fm["target"]).expanduser()
    marker = fm.get("marker", "")
    fmt = fm.get("format", "")
    if fmt == "markdown":
        if not marker:
            raise ValueError(f"{block_file.name}: markdown block needs 'marker'")
        return apply_markdown(target, body, marker, fm.get("placement", ""), dry_run)
    if fmt == "json":
        return {"target": str(target), "marker": marker,
                "action": "skipped (json handler not implemented yet)"}
    return {"target": str(target), "marker": marker,
            "action": f"skipped (unknown format {fmt!r})"}


def main() -> int:
    global RULES_DIR
    ap = argparse.ArgumentParser(description="Apply ~/.agent-rules custom blocks.")
    ap.add_argument("--dry-run", action="store_true",
                    help="show what would change without writing")
    ap.add_argument("--rules-dir", default=str(RULES_DIR),
                    help="directory holding blocks/ (default: this script's dir)")
    args = ap.parse_args()
    RULES_DIR = Path(args.rules_dir).expanduser().resolve()
    blocks_dir = RULES_DIR / "blocks"
    if not blocks_dir.is_dir():
        print(f"error: no blocks/ directory in {RULES_DIR}", file=sys.stderr)
        return 1

    results, errors = [], []
    for bf in sorted(blocks_dir.glob("*.md")):
        try:
            results.append((bf.name, apply_block(bf, args.dry_run)))
        except Exception as exc:  # noqa: BLE001 - report, never half-apply
            errors.append((bf.name, str(exc)))

    header = "agent-rules apply" + (" (dry-run)" if args.dry_run else "")
    print(f"{header}:")
    for name, res in results:
        print(f"  [{res['action']}] {name} -> {res['target']} ({res['marker']})")
    for name, err in errors:
        print(f"  [ERROR] {name}: {err}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
