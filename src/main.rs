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

/// Read from Cargo.toml at compile time, so bumping the crate version is
/// the only thing needed to change what `agent-rules version` prints.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Outcome of applying one block: which target was touched, which
/// marker/json_path it corresponds to, and what happened.
pub struct Res {
    pub target: String,
    pub marker: String,
    pub action: String,
}

/// A fully parsed, ready-to-run command line.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Apply { dry_run: bool },
    List,
    Version,
    Help,
}

/// Why `parse` rejected a command line. Each variant carries what it
/// needs to name the offending token in the error message; `NoCommand`
/// carries nothing because a bare invocation has no offending token to
/// name -- see `run`'s handling of it.
#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    NoCommand,
    UnknownCommand(String),
    DryRunNotUnderApply,
    UnexpectedArgument { command: &'static str, arg: String },
}

impl ParseError {
    /// One-line message naming the offender, or `None` for `NoCommand`:
    /// a bare invocation gets usage only, since there's no offending
    /// token to call out (no action requested -> no action taken is the
    /// point, not a mistake to report).
    fn message(&self) -> Option<String> {
        match self {
            ParseError::NoCommand => None,
            ParseError::UnknownCommand(cmd) => Some(format!("unknown command {cmd:?}")),
            ParseError::DryRunNotUnderApply => {
                Some("--dry-run is only valid under 'apply'".to_string())
            }
            ParseError::UnexpectedArgument { command, arg } => {
                Some(format!("unexpected argument {arg:?} for '{command}'"))
            }
        }
    }
}

/// Parses a full argument list (excluding argv[0]) into a `Command`.
///
/// Pure and side-effect-free: no I/O, no process exit, just data in and
/// a `Result` out, so the whole CLI surface is testable without spawning
/// a process. See the `tests` module below for the covered cases.
fn parse(args: &[String]) -> Result<Command, ParseError> {
    let Some(first) = args.first() else {
        return Err(ParseError::NoCommand);
    };

    // `--dry-run` is a real, recognized token even when it isn't paired
    // with the literal `apply` subcommand -- give it a specific "wrong
    // place" error instead of lumping it in with unrecognized commands.
    if first.as_str() == "--dry-run" {
        return Err(ParseError::DryRunNotUnderApply);
    }

    let rest = &args[1..];
    match first.as_str() {
        "apply" => parse_apply(rest),
        "list" => parse_no_args("list", rest).map(|()| Command::List),
        "version" | "-v" | "--version" => parse_no_args("version", rest).map(|()| Command::Version),
        "help" | "-h" | "--help" => parse_no_args("help", rest).map(|()| Command::Help),
        other => Err(ParseError::UnknownCommand(other.to_string())),
    }
}

/// `apply` is the only subcommand that accepts a flag: `--dry-run`, zero
/// or more times. Anything else after `apply` is a stray argument.
fn parse_apply(rest: &[String]) -> Result<Command, ParseError> {
    let mut dry_run = false;
    for a in rest {
        if a.as_str() == "--dry-run" {
            dry_run = true;
        } else {
            return Err(ParseError::UnexpectedArgument {
                command: "apply",
                arg: a.clone(),
            });
        }
    }
    Ok(Command::Apply { dry_run })
}

/// `list`, `version`, and `help` take no arguments at all. `--dry-run`
/// gets its own error (it's a real flag, just misplaced); anything else
/// is reported as a plain stray argument.
fn parse_no_args(command: &'static str, rest: &[String]) -> Result<(), ParseError> {
    match rest.first() {
        None => Ok(()),
        Some(extra) if extra.as_str() == "--dry-run" => Err(ParseError::DryRunNotUnderApply),
        Some(extra) => Err(ParseError::UnexpectedArgument {
            command,
            arg: extra.clone(),
        }),
    }
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
    match parse(args) {
        Ok(Command::Apply { dry_run }) => run_apply(dry_run),
        Ok(Command::List) => run_list(),
        Ok(Command::Version) => {
            println!("agent-rules {VERSION}");
            0
        }
        Ok(Command::Help) => {
            print_usage(false);
            0
        }
        Err(e) => {
            if let Some(msg) = e.message() {
                eprintln!("agent-rules: {msg}");
            }
            print_usage(true);
            2
        }
    }
}

fn print_usage(to_stderr: bool) {
    let text = "\
agent-rules -- apply embedded ~/.agent-rules blocks

Usage:
  agent-rules apply [--dry-run]
  agent-rules list
  agent-rules version
  agent-rules help

Commands:
  apply     apply embedded blocks to their targets
  list      list embedded blocks (name, target, format)
  version   print the version string (also: -v, --version)
  help      show this help (also: -h, --help)

Flags:
  --dry-run   apply only -- show what would change; write nothing
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn apply_bare_defaults_to_no_dry_run() {
        assert_eq!(
            parse(&args(&["apply"])),
            Ok(Command::Apply { dry_run: false })
        );
    }

    #[test]
    fn apply_with_dry_run() {
        assert_eq!(
            parse(&args(&["apply", "--dry-run"])),
            Ok(Command::Apply { dry_run: true })
        );
    }

    #[test]
    fn list_subcommand() {
        assert_eq!(parse(&args(&["list"])), Ok(Command::List));
    }

    #[test]
    fn version_subcommand() {
        assert_eq!(parse(&args(&["version"])), Ok(Command::Version));
    }

    #[test]
    fn help_subcommand() {
        assert_eq!(parse(&args(&["help"])), Ok(Command::Help));
    }

    #[test]
    fn bare_invocation_is_rejected_with_no_offender() {
        // No args at all: rejected, but with no offending token to name
        // (message() returns None for this variant) -- see `run`.
        assert_eq!(parse(&args(&[])), Err(ParseError::NoCommand));
        assert_eq!(ParseError::NoCommand.message(), None);
    }

    #[test]
    fn unknown_subcommand_is_rejected() {
        assert_eq!(
            parse(&args(&["frobnicate"])),
            Err(ParseError::UnknownCommand("frobnicate".to_string()))
        );
    }

    #[test]
    fn help_and_version_flag_aliases_are_accepted() {
        assert_eq!(parse(&args(&["-h"])), Ok(Command::Help));
        assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(parse(&args(&["-v"])), Ok(Command::Version));
        assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
    }

    #[test]
    fn removed_list_flag_stays_rejected() {
        // --list has no alias: subcommand only.
        assert_eq!(
            parse(&args(&["--list"])),
            Err(ParseError::UnknownCommand("--list".to_string()))
        );
    }

    #[test]
    fn stray_argument_after_list_is_rejected() {
        assert_eq!(
            parse(&args(&["list", "--foo"])),
            Err(ParseError::UnexpectedArgument {
                command: "list",
                arg: "--foo".to_string(),
            })
        );
    }

    #[test]
    fn stray_argument_after_apply_is_rejected() {
        assert_eq!(
            parse(&args(&["apply", "extra"])),
            Err(ParseError::UnexpectedArgument {
                command: "apply",
                arg: "extra".to_string(),
            })
        );
    }

    #[test]
    fn dry_run_under_list_is_rejected_specifically() {
        // Distinct from the generic stray-argument case above: --dry-run
        // is a real flag, just not valid here, so it gets its own error.
        assert_eq!(
            parse(&args(&["list", "--dry-run"])),
            Err(ParseError::DryRunNotUnderApply)
        );
    }

    #[test]
    fn dry_run_under_version_and_help_is_rejected() {
        assert_eq!(
            parse(&args(&["version", "--dry-run"])),
            Err(ParseError::DryRunNotUnderApply)
        );
        assert_eq!(
            parse(&args(&["help", "--dry-run"])),
            Err(ParseError::DryRunNotUnderApply)
        );
    }

    #[test]
    fn dry_run_alone_is_rejected() {
        assert_eq!(
            parse(&args(&["--dry-run"])),
            Err(ParseError::DryRunNotUnderApply)
        );
    }
}
