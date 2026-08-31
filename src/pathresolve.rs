/// Per-OS home directory using only std::env. No `dirs` crate is available
/// here (serde_json is the only dependency this project allows).
pub fn home_dir() -> Option<String> {
    #[cfg(windows)]
    {
        if let Ok(p) = std::env::var("USERPROFILE") {
            if !p.is_empty() {
                return Some(p);
            }
        }
        if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
            return Some(format!("{drive}{path}"));
        }
        None
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().filter(|s| !s.is_empty())
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Hand-rolled $VAR / ${VAR} / %VAR% expansion. No regex or shellexpand
/// crate is available (serde_json is the only dependency this project
/// allows), so this is a small manual scanner instead of a pattern match.
/// An unset variable expands to the empty string.
pub fn expand_env_vars(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        if c == '$' {
            if i + 1 < n && chars[i + 1] == '{' {
                if let Some(end) = (i + 2..n).find(|&idx| chars[idx] == '}') {
                    let name: String = chars[i + 2..end].iter().collect();
                    out.push_str(&std::env::var(&name).unwrap_or_default());
                    i = end + 1;
                    continue;
                }
            } else if i + 1 < n && is_ident_start(chars[i + 1]) {
                let start = i + 1;
                let mut end = start;
                while end < n && is_ident_char(chars[end]) {
                    end += 1;
                }
                let name: String = chars[start..end].iter().collect();
                out.push_str(&std::env::var(&name).unwrap_or_default());
                i = end;
                continue;
            }
            out.push(c);
            i += 1;
        } else if c == '%' {
            if let Some(end) = (i + 1..n).find(|&idx| chars[idx] == '%') {
                if end > i + 1 {
                    let name: String = chars[i + 1..end].iter().collect();
                    if name.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                        out.push_str(&std::env::var(&name).unwrap_or_default());
                        i = end + 1;
                        continue;
                    }
                }
            }
            out.push(c);
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Mechanical path resolution only: expand environment variables and a
/// leading '~' to the per-OS home directory. The actual target path is
/// policy that lives in each block's frontmatter (see blocks/*.md) -- this
/// never hardcodes an absolute config path.
pub fn resolve_target(raw: &str) -> Result<String, String> {
    let expanded = expand_env_vars(raw);

    let no_home_err = || "cannot resolve home directory (HOME/USERPROFILE not set)".to_string();

    let with_home = if expanded == "~" {
        home_dir().ok_or_else(no_home_err)?
    } else if let Some(rest) = expanded.strip_prefix("~/") {
        let home = home_dir().ok_or_else(no_home_err)?;
        format!("{}/{}", home.trim_end_matches(|c: char| c == '/' || c == '\\'), rest)
    } else if let Some(rest) = expanded.strip_prefix("~\\") {
        let home = home_dir().ok_or_else(no_home_err)?;
        format!("{}\\{}", home.trim_end_matches(|c: char| c == '/' || c == '\\'), rest)
    } else {
        expanded
    };

    #[cfg(windows)]
    {
        Ok(with_home.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        Ok(with_home)
    }
}
