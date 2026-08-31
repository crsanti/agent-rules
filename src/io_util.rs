use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn read_if_exists(path: &str) -> Result<(bool, String), String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok((true, s)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok((false, String::new())),
        Err(e) => Err(format!("{path}: {e}")),
    }
}

/// The directory containing the running executable. There is no on-disk
/// blocks/ directory next to a compiled binary to anchor to, since blocks
/// are embedded -- this is where .backups/ is written instead.
fn rules_dir() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    Ok(exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".")))
}

/// Seconds since the Unix epoch, used as the backup filename's timestamp
/// component. Plain std has no dependency-free, safe way to read the local
/// UTC offset (a crate like chrono or time would be needed, and this
/// project takes on no dependency beyond serde_json), so the filename uses
/// UTC epoch seconds rather than a local-time-formatted timestamp. This
/// only affects what a backup file is *named*, never what gets applied.
fn backup_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn backup_file(target: &str) -> Result<(), String> {
    let dir = rules_dir()?;
    let bdir = dir.join(".backups");
    fs::create_dir_all(&bdir).map_err(|e| e.to_string())?;
    let data = fs::read(target).map_err(|e| format!("{target}: {e}"))?;
    let base = Path::new(target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.to_string());
    let dest = bdir.join(format!("{base}.{}.bak", backup_timestamp()));
    fs::write(&dest, data).map_err(|e| e.to_string())
}

/// Writes to a sibling `<target>.tmp` file, then renames it over `target`.
/// The temp file must live in the same directory as `target` for the
/// rename to be atomic (same filesystem); appending ".tmp" to the full
/// path guarantees that trivially.
pub fn atomic_write(target: &str, text: &str) -> Result<(), String> {
    let tmp = format!("{target}.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, target).map_err(|e| e.to_string())
}
