//! Self-update: replace the running binary with the asset from the latest
//! tagged GitHub release. See ../README.md for the install scripts this
//! mirrors and the release asset names they both depend on.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const RELEASES_LATEST_URL: &str = "https://github.com/crsanti/agent-rules/releases/latest";

/// Maps this host's OS/arch (`std::env::consts::OS` / `ARCH`) to the
/// release asset that runs on it. Only the 4 platforms the release
/// workflow actually builds are supported -- see the Dockerfile's 4
/// `build_one` calls -- so anything else is a clear error rather than a
/// guess at a filename that doesn't exist.
fn asset_name(os: &str, arch: &str) -> Result<&'static str, String> {
    match (os, arch) {
        ("macos", "x86_64") => Ok("agent-rules-darwin-amd64"),
        ("macos", "aarch64") => Ok("agent-rules-darwin-arm64"),
        ("linux", "x86_64") => Ok("agent-rules-linux-amd64"),
        ("windows", "x86_64") => Ok("agent-rules-windows-amd64.exe"),
        _ => Err(format!(
            "no prebuilt binary for {os}-{arch} (supported: macos-x86_64, macos-aarch64, \
             linux-x86_64, windows-x86_64)"
        )),
    }
}

/// Extracts the release tag from a raw HTTP response header block -- the
/// output of `curl -fsSI` against `releases/latest`, which GitHub answers
/// with a redirect whose `location:` header points at
/// `releases/tag/<tag>`. Matches `location:` case-insensitively (header
/// names are case-insensitive) and splits on `\n` rather than relying on
/// `str::lines`, trimming a trailing `\r` off each line by hand, since
/// HTTP headers are CRLF-terminated.
fn extract_tag_from_headers(headers: &str) -> Result<String, String> {
    for raw_line in headers.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("location") {
            continue;
        }
        let tag = value.trim().rsplit('/').next().unwrap_or("").trim();
        if tag.is_empty() {
            return Err("empty 'location' header".to_string());
        }
        return Ok(tag.to_string());
    }
    Err(
        "no 'location' header in response (releases/latest did not redirect as expected)"
            .to_string(),
    )
}

/// Parses `vX.Y.Z[-suffix]` or `X.Y.Z[-suffix]` into `(major, minor,
/// patch)`. A leading `v` is optional; anything from the first `-` or `+`
/// onward (a SemVer prerelease or build suffix) is ignored, since only
/// the three numeric components matter for deciding whether to upgrade.
/// Returned as a plain tuple so ordering (`v0.2.10 > v0.2.9`) is exact
/// integer comparison of each component, never a string compare.
fn parse_version(raw: &str) -> Result<(u64, u64, u64), String> {
    let s = raw.strip_prefix('v').unwrap_or(raw);
    let core = s.split(['-', '+']).next().unwrap_or(s);
    let mut parts = core.split('.');
    let (Some(major), Some(minor), Some(patch), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(format!("not a valid version: {raw:?}"));
    };
    let component = |p: &str| {
        p.parse::<u64>()
            .map_err(|_| format!("not a valid version: {raw:?}"))
    };
    Ok((component(major)?, component(minor)?, component(patch)?))
}

/// Appends the literal ".old" suffix to a file name by string
/// concatenation. `Path::with_extension` would replace an existing
/// extension instead of appending to it -- "agent-rules.exe" must become
/// "agent-rules.exe.old", not "agent-rules.old".
fn old_sibling_name(file_name: &str) -> String {
    format!("{file_name}.old")
}

/// The path to the binary currently running, resolved through any
/// symlink so a symlinked install replaces the real file instead of
/// shadowing it with a plain file at the symlink's location.
fn resolve_exe_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(fs::canonicalize(&exe).unwrap_or(exe))
}

/// Removes a `<binary>.old` sibling left behind by a previous Windows
/// swap (see `swap_in`) if one is still there. Failure is ignored: on
/// unix this file never exists in the first place, so the call is a
/// harmless no-op there, and on windows the file is usually just gone
/// already.
fn cleanup_old_sibling(exe: &Path) {
    if let Some(name) = exe.file_name().and_then(|n| n.to_str()) {
        let _ = fs::remove_file(exe.with_file_name(old_sibling_name(name)));
    }
}

fn curl_available() -> bool {
    Command::new("curl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Runs `curl -fsSI` against `releases/latest` and returns the raw
/// response headers. `-I` sends a HEAD request and, without `-L`, curl
/// does not follow the redirect -- it just prints the first response's
/// headers, which is exactly the `location:` line `extract_tag_from_headers`
/// needs. This avoids the GitHub API entirely, so there's no JSON parsing
/// and no separate, tighter unauthenticated rate limit to run into.
fn fetch_latest_release_headers() -> Result<String, String> {
    let output = Command::new("curl")
        .args(["-fsSI", RELEASES_LATEST_URL])
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("checking the latest release failed ({})", output.status)
        } else {
            format!("checking the latest release failed: {detail}")
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Downloads `url` into `dest` with `curl -fsSL`. `dest` is expected to be
/// a temp file sitting next to the real executable (see `run_upgrade`),
/// so the caller's later rename onto the executable is same-filesystem
/// and atomic.
fn download_asset(url: &str, dest: &Path) -> Result<(), String> {
    let output = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(dest)
        .arg(url)
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("downloading {url} failed ({})", output.status)
        } else {
            format!("downloading {url} failed: {detail}")
        });
    }
    Ok(())
}

/// Swaps `tmp` into place at `exe`. A plain rename over the running
/// binary is valid on unix as long as source and destination are on the
/// same filesystem (guaranteed here: `tmp` is a sibling of `exe`) --
/// existing open file handles to the old inode, including this process's
/// own, keep working exactly as before.
#[cfg(unix)]
fn swap_in(tmp: &Path, exe: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(tmp, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("chmod {}: {e}", tmp.display()))?;
    fs::rename(tmp, exe).map_err(|e| format!("replacing {}: {e}", exe.display()))
}

/// Swaps `tmp` into place at `exe`. Windows refuses to overwrite a
/// running executable directly, but it does allow renaming one, so the
/// running binary is moved aside to `<name>.old` first and the new build
/// is renamed into the now-free path. If that second rename fails, a
/// restore of the original is attempted; when the restore succeeds the
/// returned error names only the install failure, and when the restore
/// also fails the error names both failures and points at the `.old`
/// path so the previous binary can be recovered by hand. On the
/// ordinary success path the `.old` file is best-effort removed --
/// this normally fails while the moved-aside file is still the one
/// executing, which is fine: `cleanup_old_sibling` clears it on the next
/// run.
#[cfg(windows)]
fn swap_in(tmp: &Path, exe: &Path) -> Result<(), String> {
    let name = exe
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("cannot read file name of {}", exe.display()))?;
    let old = exe.with_file_name(old_sibling_name(name));
    fs::rename(exe, &old).map_err(|e| format!("moving the running binary aside: {e}"))?;
    if let Err(e) = fs::rename(tmp, exe) {
        return Err(match fs::rename(&old, exe) {
            Ok(()) => format!("installing the new binary: {e}"),
            Err(restore_err) => format!(
                "installing the new binary: {e} (restoring the previous binary also failed: \
                 {restore_err} -- it is still at {})",
                old.display()
            ),
        });
    }
    let _ = fs::remove_file(&old);
    Ok(())
}

fn fail(msg: &str) -> i32 {
    eprintln!("agent-rules: upgrade: {msg}");
    1
}

/// Replaces the running binary with the latest GitHub release build for
/// this OS/arch. Every decision (which tag is latest, whether to
/// upgrade, which asset to fetch) is a pure function above; this is the
/// thin shell that sequences them with the actual network and filesystem
/// calls. A failed download or swap leaves the current binary untouched:
/// all writes land on a temp file first, and it's only ever renamed onto
/// the real path once it's known good.
pub fn run_upgrade() -> i32 {
    let exe = match resolve_exe_path() {
        Ok(p) => p,
        Err(e) => return fail(&format!("could not locate the running executable: {e}")),
    };
    cleanup_old_sibling(&exe);

    if !curl_available() {
        return fail("requires 'curl' on PATH");
    }

    let headers = match fetch_latest_release_headers() {
        Ok(h) => h,
        Err(e) => return fail(&e),
    };
    let tag = match extract_tag_from_headers(&headers) {
        Ok(t) => t,
        Err(e) => return fail(&e),
    };
    let latest = match parse_version(&tag) {
        Ok(v) => v,
        Err(e) => return fail(&format!("latest release tag {tag:?}: {e}")),
    };
    let current = match parse_version(crate::VERSION) {
        Ok(v) => v,
        Err(e) => return fail(&format!("current version {:?}: {e}", crate::VERSION)),
    };

    if latest == current {
        println!(
            "agent-rules upgrade: {} is already up to date (latest release: {tag})",
            crate::VERSION
        );
        return 0;
    }
    if latest < current {
        println!(
            "agent-rules upgrade: {} is ahead of the latest release ({tag}); nothing to do",
            crate::VERSION
        );
        return 0;
    }

    let asset = match asset_name(std::env::consts::OS, std::env::consts::ARCH) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };

    println!("agent-rules upgrade: {} -> {tag}", crate::VERSION);
    println!("  downloading {asset}...");

    let tmp = exe.with_file_name(".agent-rules-upgrade.tmp");
    let url = format!("https://github.com/crsanti/agent-rules/releases/download/{tag}/{asset}");
    if let Err(e) = download_asset(&url, &tmp) {
        let _ = fs::remove_file(&tmp);
        return fail(&e);
    }

    match fs::metadata(&tmp) {
        Ok(m) if m.len() > 0 => {}
        _ => {
            let _ = fs::remove_file(&tmp);
            return fail("downloaded asset is empty or missing");
        }
    }

    println!("  installing...");
    if let Err(e) = swap_in(&tmp, &exe) {
        let _ = fs::remove_file(&tmp);
        return fail(&e);
    }

    println!("upgraded: {} -> {tag}", crate::VERSION);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_covers_all_supported_platforms() {
        assert_eq!(
            asset_name("macos", "x86_64"),
            Ok("agent-rules-darwin-amd64")
        );
        assert_eq!(
            asset_name("macos", "aarch64"),
            Ok("agent-rules-darwin-arm64")
        );
        assert_eq!(asset_name("linux", "x86_64"), Ok("agent-rules-linux-amd64"));
        assert_eq!(
            asset_name("windows", "x86_64"),
            Ok("agent-rules-windows-amd64.exe")
        );
    }

    #[test]
    fn asset_name_rejects_unsupported_platform() {
        let err = asset_name("linux", "aarch64").unwrap_err();
        assert!(err.contains("linux-aarch64"), "error message: {err}");
    }

    #[test]
    fn tag_extraction_from_normal_crlf_headers() {
        let headers = "HTTP/2 302\r\ncontent-type: text/html\r\nlocation: https://github.com/crsanti/agent-rules/releases/tag/v0.3.0\r\n\r\n";
        assert_eq!(extract_tag_from_headers(headers), Ok("v0.3.0".to_string()));
    }

    #[test]
    fn tag_extraction_is_case_insensitive_on_header_name() {
        let headers = "HTTP/2 302\r\nLocation: https://github.com/crsanti/agent-rules/releases/tag/v0.3.0\r\n";
        assert_eq!(extract_tag_from_headers(headers), Ok("v0.3.0".to_string()));

        let headers = "HTTP/2 302\r\nLOCATION: https://github.com/crsanti/agent-rules/releases/tag/v0.3.0\r\n";
        assert_eq!(extract_tag_from_headers(headers), Ok("v0.3.0".to_string()));
    }

    #[test]
    fn tag_extraction_trims_trailing_whitespace_and_junk() {
        let headers =
            "HTTP/2 302\r\nlocation: https://github.com/crsanti/agent-rules/releases/tag/v0.3.0   \r\n";
        assert_eq!(extract_tag_from_headers(headers), Ok("v0.3.0".to_string()));
    }

    #[test]
    fn tag_extraction_errors_without_location_header() {
        let headers = "HTTP/2 200\r\ncontent-type: text/html\r\n";
        assert!(extract_tag_from_headers(headers).is_err());
    }

    #[test]
    fn version_parsing_accepts_v_prefix_and_bare_form() {
        assert_eq!(parse_version("v0.2.0"), Ok((0, 2, 0)));
        assert_eq!(parse_version("0.2.0"), Ok((0, 2, 0)));
    }

    #[test]
    fn version_parsing_strips_prerelease_suffix() {
        assert_eq!(parse_version("v1.2.3-rc1"), Ok((1, 2, 3)));
        assert_eq!(parse_version("1.2.3+build5"), Ok((1, 2, 3)));
    }

    #[test]
    fn version_parsing_rejects_garbage() {
        assert!(parse_version("not-a-version").is_err());
        assert!(parse_version("1.2").is_err());
        assert!(parse_version("1.2.3.4").is_err());
        assert!(parse_version("1.2.x").is_err());
    }

    #[test]
    fn version_ordering_compares_numerically_not_lexically() {
        // A string compare would put "v0.2.9" after "v0.2.10" ('9' > '1').
        assert!(parse_version("v0.2.10").unwrap() > parse_version("v0.2.9").unwrap());
    }

    #[test]
    fn old_sibling_name_appends_rather_than_replaces_extension() {
        assert_eq!(old_sibling_name("agent-rules.exe"), "agent-rules.exe.old");
        assert_eq!(old_sibling_name("agent-rules"), "agent-rules.old");
    }
}
