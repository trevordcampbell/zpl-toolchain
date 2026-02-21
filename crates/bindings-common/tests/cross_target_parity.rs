//! Cross-target confidence tests for bindings parity.
//!
//! These tests ensure the bindings-common parse/validate paths produce the same
//! results as direct native core calls for shared fixtures.

use std::path::Path;

use serde::Deserialize;
use zpl_toolchain_bindings_common as common;
use zpl_toolchain_core::{parse_with_tables, validate_with_profile};
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::ParserTables;

fn load_tables_json() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../generated/parser_tables.json");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}

fn load_tables() -> ParserTables {
    serde_json::from_str(&load_tables_json()).expect("failed to parse parser tables")
}

#[derive(Debug, Deserialize)]
struct FixtureEntry {
    id: String,
    zpl: String,
}

#[derive(Debug, Deserialize)]
struct ParityFixtureSet {
    version: u32,
    profile: serde_json::Value,
    parse: Vec<FixtureEntry>,
    validate: Vec<FixtureEntry>,
}

fn load_fixtures() -> ParityFixtureSet {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/fixtures/bindings-parity.v1.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture file {}: {}", path.display(), e));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse fixture file {}: {}", path.display(), e))
}

#[test]
fn parse_output_matches_core_for_shared_fixtures() {
    let tables_json = load_tables_json();
    let tables = load_tables();
    let fixtures = load_fixtures();
    assert_eq!(fixtures.version, 1, "unexpected fixture schema version");

    for fixture in fixtures.parse {
        let via_core = parse_with_tables(&fixture.zpl, Some(&tables));
        let via_bindings = common::parse_zpl_with_tables_json(&fixture.zpl, &tables_json)
            .expect("bindings-common parse should succeed");
        assert_eq!(
            serde_json::to_value(&via_core).expect("serialize core parse result"),
            serde_json::to_value(&via_bindings).expect("serialize bindings parse result"),
            "parse parity mismatch for fixture id={}",
            fixture.id
        );
    }
}

#[test]
fn validate_output_matches_core_for_shared_fixtures() {
    let tables_json = load_tables_json();
    let tables = load_tables();
    let fixtures = load_fixtures();
    assert_eq!(fixtures.version, 1, "unexpected fixture schema version");
    let profile_json =
        serde_json::to_string(&fixtures.profile).expect("serialize fixture profile json");
    let profile: Profile = serde_json::from_value(fixtures.profile).expect("parse profile");

    for fixture in fixtures.validate {
        let parsed = parse_with_tables(&fixture.zpl, Some(&tables));
        let mut via_core = validate_with_profile(&parsed.ast, &tables, Some(&profile));
        let mut all_issues = parsed.diagnostics;
        all_issues.extend(via_core.issues);
        via_core.issues = all_issues;

        let via_bindings =
            common::validate_zpl_with_tables_json(&fixture.zpl, Some(&profile_json), &tables_json)
                .expect("bindings-common validate should succeed");

        assert_eq!(
            serde_json::to_value(&via_core).expect("serialize core validation result"),
            serde_json::to_value(&via_bindings).expect("serialize bindings validation result"),
            "validate parity mismatch for fixture id={}",
            fixture.id
        );
    }
}
