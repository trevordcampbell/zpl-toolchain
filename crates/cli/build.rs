//! Build script for the ZPL CLI binary.
//!
//! Embeds `parser_tables.json` into the binary so that `zpl lint` and other
//! commands work out of the box — no `--tables` flag needed (ADR 0005).
//!
//! Table resolution order:
//!   1. `../../generated/parser_tables.json` — workspace-level generated copy (freshest during dev)
//!   2. `data/parser_tables.json` — committed in-crate copy (works from crates.io tarball)
//!
//! If neither exists the binary is built without tables; `lint` and `print`
//! will require `--tables <PATH>` at runtime.

use std::path::Path;

fn main() {
    // Declare the custom cfg so cargo check-cfg doesn't warn.
    println!("cargo::rustc-check-cfg=cfg(has_embedded_tables)");

    // 1. Workspace-level generated copy — preferred during local development
    //    because it reflects the latest spec-compiler output.
    let workspace = Path::new("../../generated/parser_tables.json");
    // 2. In-crate copy — fallback for crates.io installs where the workspace
    //    path doesn't exist.
    let in_crate = Path::new("data/parser_tables.json");

    // Watch both locations for changes.
    println!("cargo:rerun-if-changed=../../generated/parser_tables.json");
    println!("cargo:rerun-if-changed=data/parser_tables.json");

    let tables_path = if workspace.exists() {
        workspace
    } else if in_crate.exists() {
        in_crate
    } else {
        // Neither source available — build without embedded tables.
        return;
    };

    println!("cargo:rustc-cfg=has_embedded_tables");

    // Copy into OUT_DIR so include_str! has a stable, absolute path.
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("parser_tables.json");
    std::fs::copy(tables_path, &dest).expect("failed to copy parser_tables.json to OUT_DIR");
}
