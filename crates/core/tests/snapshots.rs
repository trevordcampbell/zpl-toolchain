//! Golden snapshot tests for AST and diagnostic output.
//!
//! These tests capture expected parser/validator output for representative ZPL
//! labels. If the output changes, the test fails and the developer must review
//! the diff and explicitly accept it.
//!
//! To regenerate golden files after intentional changes:
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p zpl_toolchain_core golden
//! ```

mod common;

use std::path::PathBuf;
use zpl_toolchain_core::grammar::parser::parse_with_tables;
use zpl_toolchain_core::grammar::tables::ParserTables;
use zpl_toolchain_core::validate;

fn golden_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("golden");
    p
}

/// Result of parse + validate: JSON value and extracted diagnostic/issue IDs.
struct ParseValidateResult {
    json: serde_json::Value,
    parser_diagnostic_ids: Vec<String>,
    validator_issue_ids: Vec<String>,
}

/// Compute parse + validate once, return JSON value and extracted IDs.
fn parse_and_validate(input: &str, tables: &ParserTables) -> ParseValidateResult {
    let res = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&res.ast, tables);

    let parser_diagnostic_ids: Vec<String> =
        res.diagnostics.iter().map(|d| d.id.to_string()).collect();

    let validator_issue_ids: Vec<String> = vr.issues.iter().map(|d| d.id.to_string()).collect();

    let json = serde_json::json!({
        "label_count": res.ast.labels.len(),
        "ast": serde_json::to_value(&res.ast).unwrap(),
        "parser_diagnostics": serde_json::to_value(&res.diagnostics).unwrap(),
        "validator_issues": serde_json::to_value(&vr.issues).unwrap(),
        "ok": vr.ok,
    });

    ParseValidateResult {
        json,
        parser_diagnostic_ids,
        validator_issue_ids,
    }
}

/// Produce a deterministic JSON snapshot from parse + validate results.
///
/// Serializes the full AST and diagnostics (whatever fields the structs have)
/// so changes to field names or shapes cause a visible diff.
fn snapshot_json(input: &str, tables: &ParserTables) -> String {
    let pv = parse_and_validate(input, tables);
    serde_json::to_string_pretty(&pv.json).unwrap()
}

/// Snapshot only the renderer-facing resolved label state.
fn snapshot_resolved_labels_json(input: &str, tables: &ParserTables) -> String {
    let res = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&res.ast, tables);
    let json = serde_json::json!({
        "label_count": res.ast.labels.len(),
        "resolved_labels": vr.resolved_labels,
    });
    serde_json::to_string_pretty(&json).unwrap()
}

/// Compare `actual` against a golden file.
///
/// * If `UPDATE_GOLDEN` env var is set, writes (or overwrites) the golden file.
/// * Otherwise, reads the golden file and asserts equality.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_dir().join(format!("{}.json", name));

    if std::env::var("UPDATE_GOLDEN")
        .ok()
        .filter(|v| !v.is_empty() && v != "0" && v != "false")
        .is_some()
    {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        eprintln!("Updated golden file: {}", path.display());
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "Golden file not found: {}\nRun with UPDATE_GOLDEN=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        actual.trim(),
        expected.trim(),
        "Snapshot mismatch for '{}'. Run with UPDATE_GOLDEN=1 to update.",
        name
    );
}

// ─── Snapshot Tests ─────────────────────────────────────────────────────────

#[test]
fn golden_simple_text_label() {
    let tables = &*common::TABLES;
    let input = "^XA\n^FO50,50^A0N,30,30^FDHello World^FS\n^XZ";
    assert_golden("simple_text_label", &snapshot_json(input, tables));
}

#[test]
fn golden_barcode_label() {
    let tables = &*common::TABLES;
    let input = "^XA\n^BY2,3,100\n^FO50,150^BCN,100,Y,N,N^FD>:ABC123^FS\n^XZ";
    assert_golden("barcode_label", &snapshot_json(input, tables));
}

#[test]
fn golden_multi_field_label() {
    let tables = &*common::TABLES;
    let input = "\
^XA\n\
^FO50,50^A0N,30,30^FDLine 1^FS\n\
^FO50,100^A0N,30,30^FDLine 2^FS\n\
^FO50,150^A0N,30,30^FDLine 3^FS\n\
^XZ";
    assert_golden("multi_field_label", &snapshot_json(input, tables));
}

#[test]
fn golden_empty_label() {
    let tables = &*common::TABLES;
    let input = "^XA\n^XZ";
    assert_golden("empty_label", &snapshot_json(input, tables));
}

#[test]
fn golden_diagnostic_issues() {
    let tables = &*common::TABLES;
    // Missing field origin, overlapping fields
    let input = "^XA\n^FDNo origin^FS\n^FO50,50^FO100,100^FDOverlap^FS\n^XZ";
    let pv = parse_and_validate(input, tables);
    assert!(
        pv.validator_issue_ids.iter().any(|id| id == "ZPL2201"),
        "expected ZPL2201 in validator issues: {:?}",
        pv.validator_issue_ids
    );
    assert!(
        pv.validator_issue_ids.iter().any(|id| id == "ZPL2204"),
        "expected ZPL2204 in validator issues: {:?}",
        pv.validator_issue_ids
    );
    assert_golden(
        "diagnostic_issues",
        &serde_json::to_string_pretty(&pv.json).unwrap(),
    );
}

#[test]
fn golden_graphics_and_formatting() {
    let tables = &*common::TABLES;
    let input = "\
^XA\n\
^PW800\n\
^LL600\n\
^FO0,0^GB800,600,3^FS\n\
^FO350,60^A0N,40,40^FDINVOICE^FS\n\
^XZ";
    assert_golden("graphics_formatting", &snapshot_json(input, tables));
}

#[test]
fn golden_unknown_commands() {
    let tables = &*common::TABLES;
    let input = "^XA\n^ZZ999\n^NOTACMD\n^FO50,100\n^FDHello^FS\n^XZ";
    let pv = parse_and_validate(input, tables);
    assert!(
        pv.parser_diagnostic_ids
            .iter()
            .any(|id| id == "ZPL.PARSER.1002"),
        "expected ZPL.PARSER.1002 in parser diagnostics: {:?}",
        pv.parser_diagnostic_ids
    );
    assert!(
        pv.validator_issue_ids.iter().any(|id| id == "ZPL1103"),
        "expected ZPL1103 in validator issues: {:?}",
        pv.validator_issue_ids
    );
    assert_golden(
        "unknown_commands",
        &serde_json::to_string_pretty(&pv.json).unwrap(),
    );
}

#[test]
fn golden_multi_label() {
    let tables = &*common::TABLES;
    let input = "^XA^FO10,10^FDLabel 1^FS^XZ\n^XA^FO20,20^FDLabel 2^FS^XZ\n^XA^XZ";
    assert_golden("multi_label", &snapshot_json(input, tables));
}

#[test]
fn golden_raw_payload() {
    let tables = &*common::TABLES;
    let input = "^XA\n^FO0,0\n^GFA,4,4,1,\nFF00FF00\n^FS\n^XZ";
    assert_golden("raw_payload", &snapshot_json(input, tables));
}

#[test]
fn golden_tilde_commands() {
    let tables = &*common::TABLES;
    let input = "~TA000\n^XA^FO10,10^FDAfter tilde^FS^XZ\n~JA";
    let pv = parse_and_validate(input, tables);
    assert!(
        !pv.validator_issue_ids.iter().any(|id| id == "ZPL2205"),
        "expected no ZPL2205 in validator issues: {:?}",
        pv.validator_issue_ids
    );
    assert_golden(
        "tilde_commands",
        &serde_json::to_string_pretty(&pv.json).unwrap(),
    );
}

#[test]
fn golden_cross_command_state() {
    let tables = &*common::TABLES;
    let input = "^XA\n^BY3,2,100\n^FO50,50\n^BCN,100,Y,N,N\n^FD12345^FS\n^XZ";
    assert_golden("cross_command_state", &snapshot_json(input, tables));
}

#[test]
fn golden_resolved_state_barcode_defaults() {
    let tables = &*common::TABLES;
    let input = "^XA\n^BY3,2,100\n^FO50,50\n^BCN,100,Y,N,N\n^FD12345^FS\n^XZ";
    assert_golden(
        "resolved_state_barcode_defaults",
        &snapshot_resolved_labels_json(input, tables),
    );
}

#[test]
fn golden_resolved_state_font_and_orientation_defaults() {
    let tables = &*common::TABLES;
    let input = "^XA\n^CF0,30,20\n^FWB\n^FO40,60^A0,,^FDHello^FS\n^XZ";
    assert_golden(
        "resolved_state_font_orientation_defaults",
        &snapshot_resolved_labels_json(input, tables),
    );
}

#[test]
fn golden_resolved_state_label_layout_defaults() {
    let tables = &*common::TABLES;
    let input = "^XA\n^PW800\n^LL600\n^LH20,30\n^LT5\n^LS10\n^POI\n^PMY\n^LRN\n^XZ";
    assert_golden(
        "resolved_state_layout_defaults",
        &snapshot_resolved_labels_json(input, tables),
    );
}
