// Hand-rolled line matching for the marker open/close pair and the
// gentle-ai anchor/count patterns. No regex crate is available here
// (serde_json is the only dependency this project allows), so these are
// direct string scans instead -- and because they compare plain substrings
// rather than a compiled pattern, there is no escaping step needed at all:
// arbitrary marker text just works safely.

/// Matches an opening marker line -- `^\s*<!--\s*MARKER\b.*-->\s*$` in
/// regex terms: tolerates trailing text after the marker (e.g. a
/// description comment), as long as the marker is followed by a non-word
/// character and the line still ends (ignoring trailing whitespace) with
/// "-->".
fn matches_open(line: &str, marker: &str) -> bool {
    let t = line.trim_start();
    let Some(after_open) = t.strip_prefix("<!--") else {
        return false;
    };
    let rest = after_open.trim_start();
    let Some(after_marker) = rest.strip_prefix(marker) else {
        return false;
    };
    if let Some(c) = after_marker.chars().next() {
        if c.is_alphanumeric() || c == '_' {
            return false; // fails the \b word-boundary requirement
        }
    }
    after_marker.trim_end().ends_with("-->")
}

/// Matches a closing marker line -- `^\s*<!--\s*/MARKER\s*-->\s*$` in
/// regex terms: exact, no extra text tolerated between the marker and
/// "-->".
fn matches_close(line: &str, marker: &str) -> bool {
    let t = line.trim_start();
    let Some(after_open) = t.strip_prefix("<!--") else {
        return false;
    };
    let rest = after_open.trim_start();
    let expect = format!("/{marker}");
    let Some(after_marker) = rest.strip_prefix(expect.as_str()) else {
        return false;
    };
    let after_ws = after_marker.trim_start();
    let Some(tail) = after_ws.strip_prefix("-->") else {
        return false;
    };
    tail.trim().is_empty()
}

/// Matches a gentle-ai closing anchor line, in regex terms:
/// `^\s*<!--\s*/gentle-ai:.*-->\s*$`.
fn matches_gentle_anchor(line: &str) -> bool {
    let t = line.trim_start();
    let Some(after_open) = t.strip_prefix("<!--") else {
        return false;
    };
    let rest = after_open.trim_start();
    let Some(after) = rest.strip_prefix("/gentle-ai:") else {
        return false;
    };
    after.trim_end().ends_with("-->")
}

/// Counts lines starting with an opening or closing gentle-ai marker
/// prefix -- `(?m)^\s*<!--\s*/?gentle-ai:` in regex terms. Deliberately
/// looser than matches_gentle_anchor: it only checks the line *starts
/// with* that prefix, with no requirement that "-->" ever appears. Used
/// solely to compare marker counts before/after a change as a safety net.
pub fn count_gentle(text: &str) -> usize {
    let mut count = 0;
    for raw_line in text.split('\n') {
        let t = raw_line.trim_start();
        let Some(after_open) = t.strip_prefix("<!--") else {
            continue;
        };
        let rest = after_open.trim_start();
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        if rest.starts_with("gentle-ai:") {
            count += 1;
        }
    }
    count
}

/// Splits text into lines without keeping line endings: "" -> [], and a
/// final trailing newline never produces an extra empty element.
fn split_lines_no_keep_ends(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<&str> = s.split('\n').collect();
    if parts.last() == Some(&"") {
        parts.pop();
    }
    parts
        .iter()
        .map(|p| p.strip_suffix('\r').unwrap_or(p).to_string())
        .collect()
}

struct LineRange {
    start: usize,
    end: usize, // inclusive
}

/// Computes the inclusive line ranges of every existing marker-delimited
/// block matching `marker`.
fn block_line_ranges(lines: &[String], marker: &str) -> Vec<LineRange> {
    let mut ranges = Vec::new();
    let n = lines.len();
    let mut i = 0;
    while i < n {
        if matches_open(&lines[i], marker) {
            let mut j = i + 1;
            while j < n && !matches_close(&lines[j], marker) {
                j += 1;
            }
            if j < n {
                ranges.push(LineRange { start: i, end: j });
                i = j + 1;
            } else {
                ranges.push(LineRange { start: i, end: i });
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    ranges
}

fn last_anchor_index(lines: &[String]) -> Option<usize> {
    let mut idx = None;
    for (i, ln) in lines.iter().enumerate() {
        if matches_gentle_anchor(ln) {
            idx = Some(i);
        }
    }
    idx
}

fn delete_range(lines: &mut Vec<String>, s: usize, e: usize) {
    lines.drain(s..=e);
}

fn insert_at(lines: &mut Vec<String>, idx: usize, items: &[String]) {
    lines.splice(idx..idx, items.iter().cloned());
}

fn append_block_at_end(lines: &mut Vec<String>, block_lines: &[String]) {
    if let Some(last) = lines.last() {
        if !last.trim().is_empty() {
            lines.push(String::new());
        }
    }
    lines.extend(block_lines.iter().cloned());
}

/// Replaces an existing marker-delimited block in place (de-duplicating if
/// more than one exists), or inserts after the last gentle-ai marker
/// (falling back to EOF) when the block doesn't exist yet. Never touches
/// content before the first gentle-ai marker or edits a gentle-ai-owned
/// block itself.
pub fn apply_markdown(
    target: &str,
    body: &str,
    marker: &str,
    placement: &str,
    dry_run: bool,
) -> Result<crate::Res, String> {
    let (existed, original) = crate::io_util::read_if_exists(target)?;
    let gentle_before = count_gentle(&original);

    let mut lines = split_lines_no_keep_ends(&original);
    let block_lines: Vec<String> = body.split('\n').map(|s| s.to_string()).collect();
    let ranges = block_line_ranges(&lines, marker);

    let action: &str;
    if !ranges.is_empty() {
        let first_start = ranges[0].start;
        for r in ranges.iter().rev() {
            delete_range(&mut lines, r.start, r.end);
        }
        insert_at(&mut lines, first_start, &block_lines);
        action = if ranges.len() == 1 {
            "updated"
        } else {
            "updated+deduped"
        };
    } else {
        let mut inserted = false;
        if placement == "after-last-gentle-ai-marker" {
            if let Some(anchor) = last_anchor_index(&lines) {
                let mut to_insert = vec![String::new()];
                to_insert.extend(block_lines.iter().cloned());
                insert_at(&mut lines, anchor + 1, &to_insert);
                inserted = true;
            }
        }
        if !inserted {
            append_block_at_end(&mut lines, &block_lines);
        }
        action = "inserted";
    }

    let mut new_text = lines.join("\n");
    if !existed || original.ends_with('\n') {
        new_text.push('\n');
    }

    let gentle_after = count_gentle(&new_text);
    if gentle_after < gentle_before {
        return Err(format!(
            "SAFETY ABORT: would drop gentle-ai markers ({gentle_before}->{gentle_after}) in {target}"
        ));
    }

    if new_text == original {
        return Ok(crate::Res {
            target: target.to_string(),
            marker: marker.to_string(),
            action: "unchanged".to_string(),
        });
    }
    if !dry_run {
        if existed {
            crate::io_util::backup_file(target)?;
        }
        crate::io_util::atomic_write(target, &new_text)?;
    }
    let mut action_str = action.to_string();
    if dry_run {
        action_str.push_str(" (dry-run)");
    }
    Ok(crate::Res {
        target: target.to_string(),
        marker: marker.to_string(),
        action: action_str,
    })
}
