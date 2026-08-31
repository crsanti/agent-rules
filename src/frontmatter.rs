use std::collections::HashMap;

/// Splits text into lines, keeping each line's trailing '\n'. Assumes plain
/// LF text -- all a block file can ever contain, since blocks are authored
/// as UTF-8 with '\n' line endings.
fn split_lines_keep_ends(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    let bytes = s.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            lines.push(&s[start..=i]);
            start = i + 1;
        }
    }
    if start < s.len() {
        lines.push(&s[start..]);
    }
    lines
}

/// Parses a block file's minimal "key: value" frontmatter, delimited by two
/// '---' lines (blank lines and '#' comments inside it are skipped),
/// followed by a body kept byte-for-byte verbatim except that
/// leading/trailing '\n' characters are stripped -- only newlines are
/// trimmed, no other whitespace.
pub fn parse_block_file(
    name: &str,
    content: &str,
) -> Result<(HashMap<String, String>, String), String> {
    let lines = split_lines_keep_ends(content);
    if lines.is_empty() || lines[0].trim() != "---" {
        return Err(format!("{name}: must start with '---' frontmatter"));
    }

    let mut fm = HashMap::new();
    let mut body_start: Option<usize> = None;
    for i in 1..lines.len() {
        let trimmed = lines[i].trim();
        if trimmed == "---" {
            body_start = Some(i + 1);
            break;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match trimmed.find(':') {
            None => return Err(format!("{name}: bad frontmatter line: {trimmed:?}")),
            Some(idx) => {
                let key = trimmed[..idx].trim().to_string();
                let value = trimmed[idx + 1..].trim().to_string();
                fm.insert(key, value);
            }
        }
    }
    let body_start =
        body_start.ok_or_else(|| format!("{name}: frontmatter not closed with '---'"))?;

    let body: String = lines[body_start..].concat();
    let body = body.trim_matches('\n').to_string();
    Ok((fm, body))
}
