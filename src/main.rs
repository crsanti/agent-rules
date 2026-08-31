//! agent-rules deterministically and idempotently applies a fixed set
//! of embedded config blocks to their target files. See ../README.md for
//! the block file format and semantics.

mod blocks;
mod frontmatter;
mod io_util;
mod jsonhandler;
mod markdown;
mod pathresolve;

use std::collections::HashMap;
use std::process::ExitCode;

const VERSION: &str = "0.1.0";

/// Outcome of applying one block: which target was touched, which
/// marker/json_path it corresponds to, and what happened.
pub struct Res {
    pub target: String,
    pub marker: String,
    pub action: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = env_args_skip_first();
    let code = run(&args);
    ExitCode::from(code as u8)
}

fn env_args_skip_first() -> Vec<String> {
    std::env::args().skip(1).collect()
}

fn run(args: &[String]) -> i32 {
    let mut dry_run = false;
    let mut mode = "apply";

    for a in args {
        match a.as_str() {
            "--dry-run" => dry_run = true,
            "--list" => mode = "list",
            "--version" => mode = "version",
            "-h" | "--help" => mode = "help",
            "apply" => {} // explicit spelling of the default action; a no-op
            other => {
                eprintln!("agent-rules: unknown argument {other:?}");
                print_usage(true);
                return 2;
            }
        }
    }

    match mode {
        "version" => {
            println!("agent-rules {VERSION}");
            0
        }
        "help" => {
            print_usage(false);
            0
        }
        "list" => run_list(),
        _ => run_apply(dry_run),
    }
}

fn print_usage(to_stderr: bool) {
    let text = "\
agent-rules -- apply embedded ~/.agent-rules blocks

Usage:
  agent-rules [apply] [--dry-run]
  agent-rules --list
  agent-rules --version
  agent-rules --help

Flags:
  --dry-run   show what would change; write nothing
  --list      list embedded blocks (name, target, format)
  --version   print version string
  -h, --help  show this help
";
    if to_stderr {
        eprint!("{text}");
    } else {
        print!("{text}");
    }
}

/// Routes a parsed block to its format-specific handler.
fn apply_block(
    name: &str,
    fm: &HashMap<String, String>,
    body: &str,
    dry_run: bool,
) -> Result<Res, String> {
    let raw_target = fm
        .get("target")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{name}: missing 'target' in frontmatter"))?;
    let target = pathresolve::resolve_target(raw_target).map_err(|e| format!("{name}: {e}"))?;
    let marker = fm.get("marker").cloned().unwrap_or_default();
    let format = fm.get("format").cloned().unwrap_or_default();

    match format.as_str() {
        "markdown" => {
            if marker.is_empty() {
                return Err(format!("{name}: markdown block needs 'marker'"));
            }
            let placement = fm.get("placement").cloned().unwrap_or_default();
            markdown::apply_markdown(&target, body, &marker, &placement, dry_run)
        }
        "json" => {
            let json_path = fm.get("json_path").cloned().unwrap_or_default();
            if json_path.is_empty() {
                return Err(format!("{name}: json block needs 'json_path'"));
            }
            jsonhandler::apply_json(&target, body, &json_path, dry_run)
        }
        _ => Ok(Res {
            target,
            marker,
            action: format!("skipped (unknown format {format:?})"),
        }),
    }
}

/// Parses and applies a single embedded block. Every expected failure mode
/// (bad frontmatter, invalid JSON, wrong types, the safety-abort check) is
/// surfaced as `Err`, never a panic, so the caller's loop can isolate a
/// failure to just this one block and keep going.
/// Note: this crate builds with panic = "abort" (see Cargo.toml), so a
/// genuinely unexpected panic here aborts the whole run rather than
/// degrading to a per-block error; see README.
fn process_one_block(name: &str, content: &str, dry_run: bool) -> Result<Res, String> {
    let (fm, body) = frontmatter::parse_block_file(name, content)?;
    apply_block(name, &fm, &body, dry_run)
}

fn run_list() -> i32 {
    println!("agent-rules embedded blocks ({}):", blocks::BLOCKS.len());
    let mut had_err = false;
    for &(name, content) in blocks::BLOCKS {
        match frontmatter::parse_block_file(name, content) {
            Ok((fm, _)) => {
                let format = fm.get("format").cloned().unwrap_or_default();
                let detail = if format == "json" {
                    fm.get("json_path").cloned().unwrap_or_default()
                } else {
                    fm.get("marker").cloned().unwrap_or_default()
                };
                let target = fm.get("target").cloned().unwrap_or_default();
                println!("  {name:<32} -> {target:<28} [{format}] {detail}");
            }
            Err(e) => {
                eprintln!("  [ERROR] {name}: {e}");
                had_err = true;
            }
        }
    }
    if had_err {
        1
    } else {
        0
    }
}

fn run_apply(dry_run: bool) -> i32 {
    let mut results: Vec<(&str, Res)> = Vec::new();
    let mut errors: Vec<(&str, String)> = Vec::new();

    for &(name, content) in blocks::BLOCKS {
        match process_one_block(name, content, dry_run) {
            Ok(res) => results.push((name, res)),
            Err(e) => errors.push((name, e)),
        }
    }

    let mut header = String::from("agent-rules apply");
    if dry_run {
        header.push_str(" (dry-run)");
    }
    println!("{header}:");
    for (name, res) in &results {
        println!("  [{}] {name} -> {} ({})", res.action, res.target, res.marker);
    }
    for (name, err) in &errors {
        eprintln!("  [ERROR] {name}: {err}");
    }
    if errors.is_empty() {
        0
    } else {
        1
    }
}
