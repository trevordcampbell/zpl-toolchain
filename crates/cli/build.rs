//! Build script for the ZPL CLI binary.
//!
//! When `generated/parser_tables.json` is present at build time, the file is
//! copied into `OUT_DIR` and the `has_embedded_tables` cfg flag is set so that
//! `main.rs` can embed it via `include_str!`. This eliminates the need for the
//! `--tables` flag in typical usage (ADR 0005).

use std::path::Path;

fn main() {
    // Declare the custom cfg so cargo check-cfg doesn't warn.
    println!("cargo::rustc-check-cfg=cfg(has_embedded_tables)");

    let tables_path = Path::new("../../generated/parser_tables.json");
    println!("cargo:rerun-if-changed={}", tables_path.display());

    if tables_path.exists() {
        println!("cargo:rustc-cfg=has_embedded_tables");

        // Copy into OUT_DIR so include_str! has a stable, absolute path.
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        let dest = Path::new(&out_dir).join("parser_tables.json");
        std::fs::copy(tables_path, &dest).expect("failed to copy parser_tables.json to OUT_DIR");
    }
}
