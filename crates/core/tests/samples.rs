//! Sample-based integration tests — parse real-world ZPL files and verify results.

mod common;

use std::fs;
use std::path::PathBuf;
use zpl_toolchain_core::grammar::parser::parse_with_tables;
use zpl_toolchain_core::validate;

#[test]
fn lint_samples_directory() {
    let tables = &*common::TABLES;
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/core -> repo root
    root.pop();
    root.pop();
    // Assert other generated bundles exist
    assert!(
        root.join("generated/constraints_bundle.json").exists(),
        "missing constraints_bundle.json"
    );
    assert!(
        root.join("generated/docs_bundle.json").exists(),
        "missing docs_bundle.json"
    );
    assert!(
        root.join("generated/coverage.json").exists(),
        "missing coverage.json"
    );
    let samples_dir = root.join("samples");
    let profile_path = root.join("profiles/zebra-generic-203.json");
    let profile = if profile_path.exists() {
        serde_json::from_str::<zpl_toolchain_profile::Profile>(
            &fs::read_to_string(&profile_path).unwrap(),
        )
        .ok()
    } else {
        None
    };
    for entry in fs::read_dir(&samples_dir).expect("samples") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("zpl") {
            continue;
        }
        let input = fs::read_to_string(&path).expect("read zpl");
        let res = parse_with_tables(&input, Some(tables));
        let vr = validate::validate_with_profile(&res.ast, tables, profile.as_ref());
        assert!(vr.ok, "lint failed for {:?}: {:?}", path, vr.issues);
    }
}
