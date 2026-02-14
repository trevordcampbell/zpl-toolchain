//! Cross-target confidence tests for bindings parity.
//!
//! These tests ensure the bindings-common parse/validate paths produce the same
//! results as direct native core calls for shared fixtures.

use std::path::Path;

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

#[test]
fn parse_output_matches_core_for_shared_fixtures() {
    let tables_json = load_tables_json();
    let tables = load_tables();
    let fixtures = [
        "^XA^FO50,50^A0N,30,30^FDHello^FS^XZ",
        "^XA^MUI,203,203^LH1,0^FO598,0^FDx^FS^XZ",
        "^XA^BY3,2,100^FO50,50^BCN,100,Y,N,N^FD12345^FS^XZ",
    ];

    for fixture in fixtures {
        let via_core = parse_with_tables(fixture, Some(&tables));
        let via_bindings = common::parse_zpl_with_tables_json(fixture, &tables_json)
            .expect("bindings-common parse should succeed");
        assert_eq!(
            serde_json::to_value(&via_core).expect("serialize core parse result"),
            serde_json::to_value(&via_bindings).expect("serialize bindings parse result"),
            "parse parity mismatch for fixture: {fixture}"
        );
    }
}

#[test]
fn validate_output_matches_core_for_shared_fixtures() {
    let tables_json = load_tables_json();
    let tables = load_tables();
    let profile_json = r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800,"height_dots":1200}}"#;
    let profile: Profile = serde_json::from_str(profile_json).expect("parse profile");
    let fixtures = [
        "^XA^FO9999,100^FDtest^FS^XZ",
        "^XA^MUM^FO100,0^GFA,10,10,10,FFFFFFFFFFFFFFFFFFFF^FS^XZ",
        "^XA^BY3,2,100^FO50,50^BCN,100,Y,N,N^FD12345^FS^XZ",
    ];

    for fixture in fixtures {
        let parsed = parse_with_tables(fixture, Some(&tables));
        let mut via_core = validate_with_profile(&parsed.ast, &tables, Some(&profile));
        let mut all_issues = parsed.diagnostics;
        all_issues.extend(via_core.issues);
        via_core.issues = all_issues;

        let via_bindings =
            common::validate_zpl_with_tables_json(fixture, Some(profile_json), &tables_json)
                .expect("bindings-common validate should succeed");

        assert_eq!(
            serde_json::to_value(&via_core).expect("serialize core validation result"),
            serde_json::to_value(&via_bindings).expect("serialize bindings validation result"),
            "validate parity mismatch for fixture: {fixture}"
        );
    }
}
