// Zero-dependency embed: at build time, scan the blocks/ directory sitting
// next to this Cargo.toml -- a plain sibling directory, the single source
// of truth for block content -- and generate $OUT_DIR/blocks_generated.rs,
// a `pub static BLOCKS: &[(&str, &str)]` of (filename, include_str!(...))
// pairs. src/blocks.rs pulls it in via `include!`. No include_dir or
// similar crate is used or needed.
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let blocks_dir = Path::new(&manifest_dir).join("blocks");

    println!("cargo:rerun-if-changed={}", blocks_dir.display());

    let mut entries: Vec<_> = fs::read_dir(&blocks_dir)
        .unwrap_or_else(|e| panic!("cannot read blocks dir {}: {e}", blocks_dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|ext| ext == "md").unwrap_or(false))
        .collect();
    entries.sort();

    let mut code = String::new();
    code.push_str("pub static BLOCKS: &[(&str, &str)] = &[\n");
    for path in &entries {
        println!("cargo:rerun-if-changed={}", path.display());

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| panic!("non-UTF8 block filename: {}", path.display()))
            .to_string();

        let abs_path = fs::canonicalize(path)
            .unwrap_or_else(|e| panic!("cannot canonicalize {}: {e}", path.display()));
        let abs_path_str = abs_path
            .to_str()
            .unwrap_or_else(|| panic!("non-UTF8 block path: {}", abs_path.display()));

        // {:?} on a &str yields a properly quoted/escaped Rust string
        // literal, safe to splice directly into generated source.
        code.push_str(&format!("    ({name:?}, include_str!({abs_path_str:?})),\n"));
    }
    code.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("blocks_generated.rs");
    fs::write(&dest, code).unwrap_or_else(|e| panic!("failed to write {}: {e}", dest.display()));
}
