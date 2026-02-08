//! Shared build script for bindings crates.
//!
//! Copies `generated/parser_tables.json` into `OUT_DIR` and sets
//! the `has_embedded_tables` cfg flag.

use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_embedded_tables)");

    let tables_path = Path::new("../../generated/parser_tables.json");
    println!("cargo:rerun-if-changed={}", tables_path.display());

    if tables_path.exists() {
        println!("cargo:rustc-cfg=has_embedded_tables");

        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        let dest = Path::new(&out_dir).join("parser_tables.json");
        std::fs::copy(tables_path, &dest).expect("failed to copy parser_tables.json to OUT_DIR");
    }
}
