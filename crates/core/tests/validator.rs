//! Validator tests for the ZPL toolchain.
//!
//! Tests validation diagnostics (ZPL1xxx, ZPL2xxx, ZPL3xxx), profile
//! constraints, printer gates, media modes, structural validation,
//! semantic validation, and cross-command constraints.
//!
//! Parser tests (tokenization, command recognition, AST structure) live in
//! `parser.rs`.

mod common;

use common::{extract_codes, find_args, find_diag};
use zpl_toolchain_core::grammar::parser::parse_with_tables;
use zpl_toolchain_core::validate::{self, validate_with_profile};
use zpl_toolchain_diagnostics::{Severity, codes};
use zpl_toolchain_spec_tables::{ArgUnion, Constraint, ConstraintKind};

// ─── Validator Basics ────────────────────────────────────────────────────────

#[test]
fn validator_ignores_trivia_and_field_data_nodes() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA;comment\n^FO10,10^FDdata^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    // Should not produce errors from Trivia or FieldData nodes themselves
    let unexpected: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| d.message.contains("Trivia") || d.message.contains("FieldData"))
        .collect();
    assert!(
        unexpected.is_empty(),
        "validator should ignore Trivia/FieldData nodes: {:?}",
        unexpected
    );
}

#[test]
fn validator_diagnostics_have_spans() {
    let tables = &*common::TABLES;
    // Create a label with an out-of-range value to trigger a validator diagnostic
    let result = parse_with_tables("^XA^BY999^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    // Find any diagnostic with a span
    let with_span: Vec<_> = vr.issues.iter().filter(|d| d.span.is_some()).collect();
    if !vr.issues.is_empty() {
        assert!(
            !with_span.is_empty(),
            "validator diagnostics should include spans: {:?}",
            vr.issues
        );
    }
}

#[test]
fn diag_no_false_positives_valid_label() {
    let tables = &*common::TABLES;
    // A well-formed label should produce NO error-level diagnostics
    let result = parse_with_tables("^XA^CF0,30^FO50,50^FDHello World^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let errors: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "well-formed label should have no errors: {:?}",
        errors,
    );
}

#[test]
fn validation_result_includes_resolved_labels_per_input_label() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^BY2,3,40^XZ^XA^BY3,2.5,60^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert_eq!(
        vr.resolved_labels.len(),
        2,
        "expected one resolved state per input label"
    );
    assert_eq!(vr.resolved_labels[0].values.barcode.height, Some(40));
    assert_eq!(vr.resolved_labels[1].values.barcode.height, Some(60));
}

#[test]
fn resolved_label_state_tracks_effective_dimensions() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^PW800^LL1200^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert_eq!(vr.resolved_labels.len(), 1);
    assert_eq!(vr.resolved_labels[0].effective_width, Some(800.0));
    assert_eq!(vr.resolved_labels[0].effective_height, Some(1200.0));
}

// ─── ZPL1101: Arity ─────────────────────────────────────────────────────────

#[test]
fn diag_zpl1101_too_many_args() {
    let tables = &*common::TABLES;
    // ^BY has arity 3; give it 5 args
    let result = parse_with_tables("^XA^BY1,2,10,extra,more^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::ARITY),
        "should flag too many args: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1101_correct_arity_passes() {
    let tables = &*common::TABLES;
    // ^BY with exactly 3 args (correct arity) should NOT trigger ZPL1101
    let result = parse_with_tables("^XA^BY2,3.0,10^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::ARITY),
        "correct arity should not emit ZPL1101: {:?}",
        vr.issues,
    );
}

// ─── ZPL1103: Invalid Enum ───────────────────────────────────────────────────

#[test]
fn diag_zpl1103_invalid_enum() {
    let tables = &*common::TABLES;
    // ^BC orientation (arg 0) must be N/R/I/B; give it X
    let result = parse_with_tables("^XA^BY2^BCX^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::INVALID_ENUM),
        "should flag invalid enum: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1103_valid_enum_passes() {
    let tables = &*common::TABLES;
    // ^BC orientation (arg 0) with valid value "N" should NOT trigger ZPL1103
    let result = parse_with_tables("^XA^BY2^BCN,100,Y,N,N^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::INVALID_ENUM),
        "valid enum should not emit ZPL1103: {:?}",
        vr.issues,
    );
}

// ─── ZPL1104: Empty Field Data ───────────────────────────────────────────────

#[test]
fn diag_zpl1104_empty_field_data() {
    let tables = &*common::TABLES;
    // ^FD with no data and no following FieldData node should trigger ZPL1104.
    // The ^FD/^FV spec has an emptyData constraint: { "kind": "emptyData", "severity": "warn" }.
    let result = parse_with_tables("^XA^FO50,50^FD^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::EMPTY_FIELD_DATA),
        "empty field data should emit ZPL1104: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1104_non_empty_field_data_passes() {
    let tables = &*common::TABLES;
    // ^FD with content should NOT trigger ZPL1104
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::EMPTY_FIELD_DATA),
        "non-empty field data should not emit ZPL1104: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1104_empty_field_variable() {
    let tables = &*common::TABLES;
    // ^FV shares emptyData semantics with ^FD.
    let result = parse_with_tables("^XA^FO50,50^FV^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::EMPTY_FIELD_DATA),
        "empty field variable should emit ZPL1104: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1104_field_data_in_following_node_satisfies_non_empty() {
    let tables = &*common::TABLES;
    // Inline ^FD arg is empty, but following FieldData content exists.
    let result = parse_with_tables("^XA^FO50,50^FD\nhello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::EMPTY_FIELD_DATA),
        "non-empty trailing FieldData should satisfy emptyData constraint: {:?}",
        vr.issues,
    );
}

// ─── ZPL1107/1108/1109: Type Validation ──────────────────────────────────────

#[test]
fn diag_zpl1107_int_type_mismatch() {
    let tables = &*common::TABLES;
    // ^FO x (arg 0) should be int; give it "abc"
    let result = parse_with_tables("^XA^FOabc,200^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::EXPECTED_INTEGER),
        "should flag non-integer: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1107_valid_integer_passes() {
    let tables = &*common::TABLES;
    // ^FO with valid integer args should NOT trigger ZPL1107
    let result = parse_with_tables("^XA^FO100,200^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::EXPECTED_INTEGER),
        "valid integer should not emit ZPL1107: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1108_float_type_mismatch() {
    let tables = &*common::TABLES;
    // ^BY ratio (arg 1) is float with range [2.0, 3.0]; give it "xyz"
    let result = parse_with_tables("^XA^BY2,xyz,10^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::EXPECTED_NUMERIC),
        "should flag non-float: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1108_valid_float_passes() {
    let tables = &*common::TABLES;
    // ^BY ratio (arg 1) with valid float 3.0 should NOT trigger ZPL1108
    let result = parse_with_tables("^XA^BY2,3.0,10^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::EXPECTED_NUMERIC),
        "valid float should not emit ZPL1108: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1109_char_type_mismatch() {
    let tables = &*common::TABLES;
    // ^CW takes font letter (char) + font name; give "ABC" (3 chars) as font letter
    // ^CC is not suitable for this test because it consumes exactly one char as
    // the new prefix and re-tokenizes the rest.
    // Instead, test with a command that has a char-typed arg but normal parsing.
    // Use ^CD via the validator directly with a multi-char delimiter (which gets
    // consumed as one char by the early handler, so also not suitable).
    // ^FW orientation is enum, not char type.
    // Look for any command with type=char that uses normal arg parsing.
    // Since ^CC/^CD/^CT now use early single-char handling, we need to test
    // the char type validation path with a different command.
    // For now, verify ^CC correctly takes only one char (the first) from "ABC".
    let result = parse_with_tables("^XA^CCABC^XZ", Some(tables));
    let codes = extract_codes(&result);
    // ^CC takes 'A' as the new prefix; 'B' and 'C' become unknown-command candidates
    assert!(codes.contains(&"^CC".to_string()), "should parse ^CC");
    // With prefix changed to 'A', subsequent tokens are re-tokenized
    // The command should have arg value 'A' (single char)
    let cc_args = find_args(&result, "^CC");
    assert_eq!(
        cc_args[0].value.as_deref(),
        Some("A"),
        "^CC should take only first char"
    );
}

#[test]
fn diag_zpl1109_single_char_passes() {
    let tables = &*common::TABLES;
    // ^CC with a single valid char should NOT trigger ZPL1109
    let result = parse_with_tables("^XA^CC+^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::EXPECTED_CHAR),
        "single char should pass: {:?}",
        vr.issues,
    );
}

// ─── ZPL1201: Out of Range ───────────────────────────────────────────────────

#[test]
fn diag_zpl1201_out_of_range() {
    let tables = &*common::TABLES;
    // ^BY module width (arg 0) range is [1, 10]; give it 99
    let result = parse_with_tables("^XA^BY99^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::OUT_OF_RANGE),
        "should flag out of range: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1201_in_range_passes() {
    let tables = &*common::TABLES;
    // ^BY module width (arg 0) range is [1, 10]; give it 5 (within range)
    let result = parse_with_tables("^XA^BY5^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::OUT_OF_RANGE),
        "in-range value should not emit ZPL1201: {:?}",
        vr.issues,
    );
}

// ─── ZPL1401: Profile Constraint ─────────────────────────────────────────────

#[test]
fn diag_zpl1401_profile_constraint() {
    let tables = &*common::TABLES;
    // ^PW (print width) has profileConstraint { field: "page.width_dots", op: "lte" }
    // Create a profile with width_dots = 100, then set ^PW9999 → violates lte
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":100,"height_dots":100}}"#,
    );
    let result = parse_with_tables("^XA^PW9999^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "should flag profile violation: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_profile_constraint_ll_exceeds_height() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^LL1500 exceeds profile height 1200 — now caught by generic profileConstraint (ZPL1401)
    let result = parse_with_tables("^XA^LL1500^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "should flag ^LL exceeding profile height via generic constraint: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_profile_constraint_ll_within_height() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^LL800 is within profile height 1200 — generic profileConstraint should not fire
    let result = parse_with_tables("^XA^LL800^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "should not flag ^LL within profile height: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_profile_constraint_pw_respects_mu_unit_conversion() {
    let tables = &*common::TABLES;
    // With ^MU inches, ^PW is given in inches and must be converted to dots
    // before profileConstraint comparison against page.width_dots.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":812,"height_dots":1200}}"#,
    );
    // 5 inches at 203 dpi = 1015 dots > 812, so this must violate profileConstraint.
    let result = parse_with_tables("^XA^MUI^PW5^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "^PW under ^MU inches should be converted to dots for profileConstraint: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_profile_constraint_pw_respects_mu_desired_dpi_conversion() {
    let tables = &*common::TABLES;
    // Profile width corresponds to 4 inches at 203 dpi.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":812,"height_dots":1200}}"#,
    );
    // ^MU specifies conversion to desired dpi=300. ^PW4 should be interpreted as
    // 1200 dots, which exceeds 812.
    let result = parse_with_tables("^XA^MUI,203,300^PW4^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "^MU desired dpi should affect profileConstraint conversion: {:?}",
        vr.issues,
    );
}

// ─── ZPL1501/1502: Required Missing/Empty ────────────────────────────────────

#[test]
fn diag_zpl1501_required_missing() {
    let tables = &*common::TABLES;
    // ^GF has 5 args: enum(optional), int(required), int(required),
    // int(required), string(required). Giving no args should flag the
    // required ones. With allowEmptyTrailing=true, absent trailing args
    // are treated as "empty" (ZPL1502) rather than "missing" (ZPL1501).
    let result = parse_with_tables("^XA^GF^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::REQUIRED_MISSING || d.id == codes::REQUIRED_EMPTY),
        "should flag required missing/empty: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1502_required_empty() {
    let tables = &*common::TABLES;
    // ^GFA,,100,10,data — arg 1 (binary_byte_count) is required (optional=false)
    // but given as empty (the ",," leaves it blank).
    let result = parse_with_tables("^XA^GFA,,100,10,data^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let has_presence = vr
        .issues
        .iter()
        .any(|d| d.id == codes::REQUIRED_MISSING || d.id == codes::REQUIRED_EMPTY);
    assert!(
        has_presence,
        "should flag empty required arg: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_mn_mode_optional_with_default() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^MN^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let has_presence = vr
        .issues
        .iter()
        .any(|d| d.id == codes::REQUIRED_MISSING || d.id == codes::REQUIRED_EMPTY);
    assert!(
        !has_presence,
        "^MN mode should be optional with default N: {:?}",
        vr.issues,
    );
}

// ─── ZPL2101: Required Command ───────────────────────────────────────────────

#[test]
fn diag_zpl2101_requires_missing() {
    let tables = &*common::TABLES;
    // ^BC requires ^BY in the label; omit ^BY entirely
    let result = parse_with_tables("^XA^BCN,100,Y,N,N^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::REQUIRED_COMMAND),
        "should flag missing required command: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2101_br_requires_by_missing() {
    let tables = &*common::TABLES;
    // ^BR requires ^BY — ZPL2101 documents all barcode requires constraints
    let result = parse_with_tables("^XA^FO10,10^BRN^FD1234567890123456789^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::REQUIRED_COMMAND),
        "^BR without ^BY should emit ZPL2101: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2101_required_command_present_passes() {
    let tables = &*common::TABLES;
    // ^BC requires ^BY; include ^BY before ^BC — should NOT trigger ZPL2101
    let result = parse_with_tables("^XA^BY2^BCN,100,Y,N,N^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::REQUIRED_COMMAND),
        "required command present should not emit ZPL2101: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2101_field_scoped_requires_missing_in_current_field() {
    let tables = &*common::TABLES;
    // ^FB now has a field-scoped requires(^FD|^FV). ^FD in a previous field
    // must not satisfy ^FB in a later field.
    let result = parse_with_tables(
        "^XA^FO10,10^FDfirst^FS^FO20,20^FB100,2,0,L,0^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::REQUIRED_COMMAND),
        "field-scoped requires should fail when current field has no ^FD/^FV: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2101_field_scoped_requires_satisfied_by_following_fd() {
    let tables = &*common::TABLES;
    // ^FB requires ^FD/^FV in the same field, regardless of command order.
    let result = parse_with_tables("^XA^FO20,20^FB100,2,0,L,0^FDsecond^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::REQUIRED_COMMAND),
        "field-scoped requires should pass when same field contains ^FD: {:?}",
        vr.issues,
    );
}

// ─── ZPL2103: Order Violation ────────────────────────────────────────────────

#[test]
fn diag_zpl2103_order_violation() {
    let tables = &*common::TABLES;
    // ^FH has constraint "before:^FD|^FV". Place ^FH after ^FD in the same
    // field to trigger an order violation.
    let result = parse_with_tables("^XA^FO10,10^FDtest^FH^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::ORDER_BEFORE),
        "should flag order violation: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2103_correct_order_passes() {
    let tables = &*common::TABLES;
    // ^BC placed before ^FD (correct order) — should NOT trigger ZPL2103
    let result = parse_with_tables("^XA^BY2^FO10,10^BCN,100,Y,N,N^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::ORDER_BEFORE),
        "correct order should not emit ZPL2103: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2103_field_order_does_not_cross_fields() {
    let tables = &*common::TABLES;
    // ^A has order constraints relative to ^FD/^FV and ^FS. It should be
    // evaluated within its current field, not against prior fields.
    let result = parse_with_tables(
        "^XA^FO10,10^FDfirst^FS^FO20,20^A0,30,30^FDsecond^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::ORDER_BEFORE),
        "field-scoped ordering should not be violated by prior field commands: {:?}",
        vr.issues,
    );
}

// ─── ZPL2201: Field Data Without Origin ──────────────────────────────────────

#[test]
fn diag_zpl2201_missing_field_origin() {
    let tables = &*common::TABLES;
    // ^FD without ^FO or ^FT preceding it
    let result = parse_with_tables("^XA^FDno origin^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN),
        "should flag missing field origin: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2201_field_origin_present_passes() {
    let tables = &*common::TABLES;
    // ^FO before ^FD provides field origin — should NOT trigger ZPL2201
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN),
        "field origin present should not emit ZPL2201: {:?}",
        vr.issues,
    );
}

// ─── ZPL2202: Empty Label ────────────────────────────────────────────────────

#[test]
fn diag_zpl2202_empty_label() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::EMPTY_LABEL),
        "empty label should emit ZPL2202: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2202_non_empty_label_passes() {
    let tables = &*common::TABLES;
    // A label with content should NOT trigger ZPL2202
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::EMPTY_LABEL),
        "non-empty label should not emit ZPL2202: {:?}",
        vr.issues,
    );
}

// ─── ZPL2203: Overlapping Fields ─────────────────────────────────────────────

#[test]
fn diag_zpl2203_overlapping_fields() {
    let tables = &*common::TABLES;
    // Two ^FO without ^FS between them
    let result = parse_with_tables("^XA^FO10,10^FO20,20^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::FIELD_NOT_CLOSED),
        "overlapping fields should emit ZPL2203: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2203_non_overlapping_fields_passes() {
    let tables = &*common::TABLES;
    // Properly separated fields: ^FO -> ^FD -> ^FS, then ^FO -> ^FD -> ^FS
    let result = parse_with_tables(
        "^XA^FO10,10^FDFirst^FS^FO10,50^FDSecond^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::FIELD_NOT_CLOSED),
        "non-overlapping fields should not emit ZPL2203: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2203_unclosed_field_at_end_of_label() {
    let tables = &*common::TABLES;
    // Missing ^FS before ^XZ should emit FIELD_NOT_CLOSED.
    let result = parse_with_tables("^XA^FO10,10^FDHello^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::FIELD_NOT_CLOSED),
        "unclosed field at end of label should emit ZPL2203: {:?}",
        vr.issues,
    );
}

// ─── ZPL2204: Orphaned Field Separator ───────────────────────────────────────

#[test]
fn diag_zpl2204_orphaned_fs() {
    let tables = &*common::TABLES;
    // ^FS without a preceding ^FO
    let result = parse_with_tables("^XA^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::ORPHANED_FIELD_SEPARATOR),
        "orphaned ^FS should emit ZPL2204: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2204_paired_fs_passes() {
    let tables = &*common::TABLES;
    // ^FS properly paired with ^FO — should NOT trigger ZPL2204
    let result = parse_with_tables("^XA^FO10,10^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::ORPHANED_FIELD_SEPARATOR),
        "paired ^FS should not emit ZPL2204: {:?}",
        vr.issues,
    );
}

// ─── ZPL2205: Host Command in Label ──────────────────────────────────────────

#[test]
fn diag_zpl2205_scope_violation_host_in_label() {
    let tables = &*common::TABLES;
    // ~HS is a host command (plane: "host") — should not be inside ^XA/^XZ
    let result = parse_with_tables("^XA~HS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "host command inside label should emit ZPL2205: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_label_command_in_label_passes() {
    let tables = &*common::TABLES;
    // ^FO is a label-scoped command — should NOT trigger ZPL2205 inside ^XA/^XZ
    let result = parse_with_tables("^XA^FO10,10^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "label command in label should not emit ZPL2205: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_pre_xa_device_commands_do_not_warn() {
    let tables = &*common::TABLES;
    // ^PW/^LL before ^XA should not be treated as inside-label commands.
    let result = parse_with_tables("^PW900^LL200^XA^FO10,20^FDok^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "pre-^XA device commands should not emit ZPL2205: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_inside_xa_pw_ll_do_not_warn() {
    let tables = &*common::TABLES;
    // ^PW/^LL are label-format setup commands and should be allowed inside ^XA/^XZ.
    let result = parse_with_tables("^XA^PW900^LL200^FO10,20^FDok^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "inside-label ^PW/^LL should not emit ZPL2205: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_inside_xa_format_setup_commands_do_not_warn() {
    let tables = &*common::TABLES;
    // Common format/media setup commands are valid inside ^XA/^XZ.
    let result = parse_with_tables(
        "^XA^MMT^MNN^MTT^PR4,4,4^MD5~SD15~NC001^FO10,20^FDok^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "inside-label format setup commands should not emit ZPL2205: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_inside_xa_cw_allowed_by_placement() {
    let tables = &*common::TABLES;
    // ^CW is session/device semantic state but valid inside ^XA/^XZ by placement rule.
    let result = parse_with_tables("^XA^CWx,E:ARIAL.TTF^FO10,20^FDok^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "^CW should be allowed inside label by explicit placement: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_hh_inside_xa_warns() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^HH^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::HOST_COMMAND_IN_LABEL),
        "^HH should be treated as host-return command and warn inside label: {:?}",
        vr.issues,
    );
}

#[test]
fn hz_uppercase_info_type_is_accepted() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^HZO,E:TEST.GRF,N", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::INVALID_ENUM),
        "uppercase ^HZ info_type should be accepted: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2205_inside_flag_resets_after_xz() {
    let tables = &*common::TABLES;
    // First ~HS is inside label (warn). Second ~HS is outside label (allowed).
    let result = parse_with_tables("^XA~HS^XZ~HS", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let count = vr
        .issues
        .iter()
        .filter(|d| d.id == codes::HOST_COMMAND_IN_LABEL)
        .count();
    assert_eq!(
        count, 1,
        "inside-format bounds should reset at ^XZ: {:?}",
        vr.issues,
    );
}

// ─── ZPL2301: Duplicate Field Number ─────────────────────────────────────────

#[test]
fn diag_zpl2301_duplicate_fn() {
    let tables = &*common::TABLES;
    // Two fields with the same ^FN number
    let result = parse_with_tables(
        "^XA^FO10,10^FN1^FDFirst^FS^FO10,50^FN1^FDSecond^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::DUPLICATE_FIELD_NUMBER),
        "duplicate ^FN should emit ZPL2301: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2301_unique_fn_passes() {
    let tables = &*common::TABLES;
    // Two fields with different ^FN numbers — should NOT trigger
    let result = parse_with_tables(
        "^XA^FO10,10^FN1^FDFirst^FS^FO10,50^FN2^FDSecond^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::DUPLICATE_FIELD_NUMBER),
        "unique ^FN numbers should not emit ZPL2301: {:?}",
        vr.issues,
    );
}

// ─── ZPL2302: Position Out of Bounds ─────────────────────────────────────────

#[test]
fn diag_zpl2302_position_out_of_bounds() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^FO with x=9999 exceeds profile width of 800
    let result = parse_with_tables("^XA^FO9999,100^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "position exceeding bounds should emit ZPL2302: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_position_within_bounds() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^FO with valid positions
    let result = parse_with_tables("^XA^FO100,200^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "valid position should not emit ZPL2302: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_ft_position_out_of_bounds() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    let result = parse_with_tables("^XA^FT9999,100^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "^FT out-of-bounds should emit ZPL2302: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_ft_position_within_bounds() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    let result = parse_with_tables("^XA^FT799,1199^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "^FT position within bounds should not emit ZPL2302: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_exact_boundary_is_currently_allowed() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    let result = parse_with_tables("^XA^PW800^LL1200^FO800,1200^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "exact boundary position currently uses strict '>' (no ZPL2302): {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_pw_overrides_profile() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^PW sets width to 400 (narrower than profile), ^FO x=500 exceeds it
    let result = parse_with_tables("^XA^PW400^FO500,100^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "^PW should override profile width for bounds check: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_lh_offsets_field_position() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^LH offsets ^FO, so effective x = 700 + 200 = 900 > profile width 800
    let result = parse_with_tables("^XA^LH700,0^FO200,50^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "^LH should offset ^FO/^FT bounds checks: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2302_lh_resets_per_label() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // Label 1: effective x=900 -> out of bounds.
    // Label 2: ^LH is reset; x=200 -> in bounds.
    let result = parse_with_tables(
        "^XA^LH700,0^FO200,50^FDL1^FS^XZ^XA^FO200,50^FDL2^FS^XZ",
        Some(tables),
    );
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    let out_of_bounds_count = vr
        .issues
        .iter()
        .filter(|d| d.id == codes::POSITION_OUT_OF_BOUNDS)
        .count();
    assert_eq!(
        out_of_bounds_count, 1,
        "^LH should apply per-label only: {:?}",
        vr.issues
    );
}

#[test]
fn diag_zpl2302_lh_with_mu_inches_conversion() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^MUI,203,203 => inches to dots. ^LH1,0 => x home = 203 dots.
    // ^FO598,0 => effective x = 801 > 800.
    let result = parse_with_tables("^XA^MUI,203,203^LH1,0^FO598,0^FDx^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "^LH should be normalized using active ^MU units: {:?}",
        vr.issues,
    );
}

// ─── ZPL2303: Unknown Font ───────────────────────────────────────────────────

#[test]
fn diag_zpl2303_unknown_font() {
    let tables = &*common::TABLES;
    // ^A with font 'x' (lowercase, not built-in A-Z or 0-9)
    let result = parse_with_tables("^XA^FO10,10^Ax,30,30^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::UNKNOWN_FONT),
        "unknown font should emit ZPL2303: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2303_builtin_font_passes() {
    let tables = &*common::TABLES;
    // ^A with font 'A' (built-in)
    let result = parse_with_tables("^XA^FO10,10^AA,30,30^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::UNKNOWN_FONT),
        "built-in font should not emit ZPL2303: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2303_cw_loaded_font_passes() {
    let tables = &*common::TABLES;
    // ^CW loads font 'x', then ^A uses it — should NOT trigger
    let result = parse_with_tables(
        "^XA^CWx,E:ARIAL.TTF^FO10,10^Ax,30,30^FDtest^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::UNKNOWN_FONT),
        "^CW loaded font should not emit ZPL2303: {:?}",
        vr.issues,
    );
}

// ─── ZPL2304: Invalid Hex Escape ─────────────────────────────────────────────

#[test]
fn diag_zpl2304_invalid_hex_escape() {
    let tables = &*common::TABLES;
    // ^FH enables hex escapes in field data; _GZ has invalid hex chars
    // (G and Z are not in 0-9/A-F). Use newline after ^FD to ensure content
    // flows into a FieldData node (inline content is absorbed into ^FD args
    // and the ZPL2304 check runs on FieldData nodes).
    let result = parse_with_tables("^XA^FO50,50^FH^FD\nHello _GZ World\n^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let diag_ids: Vec<&str> = vr.issues.iter().map(|d| &*d.id).collect();
    assert!(
        diag_ids.contains(&codes::INVALID_HEX_ESCAPE),
        "expected ZPL2304 for invalid hex escape _GZ, got {:?}",
        diag_ids
    );
}

#[test]
fn diag_zpl2304_invalid_hex_escape_inline_fd() {
    let tables = &*common::TABLES;
    // Inline ^FD content should be checked too, not only multiline FieldData nodes.
    let result = parse_with_tables("^XA^FO50,50^FH^FDHello _GZ World^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let diag_ids: Vec<&str> = vr.issues.iter().map(|d| &*d.id).collect();
    assert!(
        diag_ids.contains(&codes::INVALID_HEX_ESCAPE),
        "inline invalid hex escape should emit ZPL2304: {:?}",
        vr.issues
    );
}

#[test]
fn diag_zpl2304_valid_hex_escape_passes() {
    let tables = &*common::TABLES;
    // ^FH with valid hex escape _1A (both chars are valid hex digits) should not trigger
    let result = parse_with_tables("^XA^FO50,50^FH^FD\nHello _1A World\n^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let diag_ids: Vec<&str> = vr.issues.iter().map(|d| &*d.id).collect();
    assert!(
        !diag_ids.contains(&codes::INVALID_HEX_ESCAPE),
        "expected no ZPL2304 for valid hex escape _1A, got {:?}",
        diag_ids
    );
}

#[test]
fn diag_zpl2304_custom_indicator() {
    let tables = &*common::TABLES;
    // ^FH# sets '#' as the indicator — #GZ should trigger ZPL2304, _GZ should be fine
    let result = parse_with_tables("^XA^FO50,50^FH#^FD\n#GZ and _GZ\n^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let hex_diags: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| d.id == codes::INVALID_HEX_ESCAPE)
        .collect();
    assert_eq!(
        hex_diags.len(),
        1,
        "custom indicator '#' should find 1 error (#GZ), got {:?}",
        hex_diags
    );
    // Verify the indicator key is in the context
    let ctx = hex_diags[0]
        .context
        .as_ref()
        .expect("ZPL2304 should have context");
    assert_eq!(
        ctx.get("indicator").unwrap(),
        "#",
        "context should report custom indicator"
    );
}

#[test]
fn diag_zpl2304_indicator_resets_between_fields() {
    let tables = &*common::TABLES;
    // Field 1 uses custom indicator '#', field 2 uses default '_'
    // Field 1: #41 is valid (hex), field 2: _GZ should trigger error
    let result = parse_with_tables(
        "^XA^FO10,10^FH#^FD\n#41\n^FS^FO10,50^FH^FD\n_GZ\n^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    let hex_diags: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| d.id == codes::INVALID_HEX_ESCAPE)
        .collect();
    assert_eq!(
        hex_diags.len(),
        1,
        "should have 1 error for _GZ in field 2: {:?}",
        hex_diags
    );
    let ctx = hex_diags[0]
        .context
        .as_ref()
        .expect("ZPL2304 should have context");
    assert_eq!(
        ctx.get("indicator").unwrap(),
        "_",
        "field 2 should use default indicator"
    );
}

// ─── ZPL2305: Redundant State ────────────────────────────────────────────────

#[test]
fn diag_zpl2305_redundant_state() {
    let tables = &*common::TABLES;
    // Two ^BY commands with no barcode between them
    let result = parse_with_tables("^XA^BY2^BY3^BCN,100^FDtest^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::REDUNDANT_STATE),
        "redundant ^BY should emit ZPL2305: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2305_consumed_state_passes() {
    let tables = &*common::TABLES;
    // ^BY then barcode then ^BY — second ^BY is NOT redundant because first was consumed
    let result = parse_with_tables(
        "^XA^BY2^BCN,100^FDtest1^FS^BY3^BCN,200^FDtest2^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::REDUNDANT_STATE),
        "consumed state should not emit ZPL2305: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2305_alias_forms_share_same_producer_state() {
    let tables = &*common::TABLES;
    // ^CC and ~CC are aliases for the same producer command.
    let result = parse_with_tables("^XA^CC^^~CC^^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::REDUNDANT_STATE),
        "alias producer forms should participate in the same redundant-state tracking: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl1201_bt_row_height_default_from_by_is_validated() {
    let mut tables = (*common::TABLES).clone();
    let bt_cmd = tables
        .commands
        .iter_mut()
        .find(|cmd| cmd.codes.iter().any(|c| c == "^BT"))
        .expect("^BT command entry should exist");
    let bt_args = bt_cmd
        .args
        .as_mut()
        .expect("^BT should have args in generated tables");
    for arg_union in bt_args {
        if let zpl_toolchain_spec_tables::ArgUnion::Single(arg) = arg_union
            && arg.key.as_deref() == Some("h2")
        {
            arg.default_from_state_key = Some("barcode.height".to_string());
        }
    }

    // ^BT h2 defaults from ^BY barcode.height; 9999 is out of h2's [1,255] range.
    let result = parse_with_tables("^XA^BY2,3,9999^BTN^FDtest^FS^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^BT")),
        "resolved ^BT defaults should be validated against arg range: {:?}",
        vr.issues,
    );
}

#[test]
fn bt_row_height_without_state_key_uses_non_state_default() {
    let mut tables = (*common::TABLES).clone();
    let bt_cmd = tables
        .commands
        .iter_mut()
        .find(|cmd| cmd.codes.iter().any(|c| c == "^BT"))
        .expect("^BT command entry should exist");
    let bt_args = bt_cmd
        .args
        .as_mut()
        .expect("^BT should have args in generated tables");
    for arg_union in bt_args {
        if let zpl_toolchain_spec_tables::ArgUnion::Single(arg) = arg_union
            && arg.key.as_deref() == Some("h2")
        {
            // Explicitly clear mapping to simulate ambiguous defaultFrom config.
            arg.default_from_state_key = None;
        }
    }

    // With no explicit state key and ^BY having multiple effects.sets, validator
    // should not infer ambiguous mapping and should fall back to ^BT's static default.
    let result = parse_with_tables("^XA^BY2,3,9999^BTN^FDtest^FS^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^BT")),
        "ambiguous defaultFrom without state key should not use ^BY height implicitly: {:?}",
        vr.issues,
    );
}

#[test]
fn by_defaults_do_not_leak_across_labels() {
    let tables = &*common::TABLES;
    let result = parse_with_tables(
        "^XA^FO10,10^BY2,3,9999^BTN^FDa^FS^XZ^XA^FO10,10^BTN^FDb^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    let bt_requires_warnings = vr
        .issues
        .iter()
        .filter(|d| {
            d.id == codes::REQUIRED_COMMAND
                && d.message.contains("^BT")
                && d.message.contains("^BY")
        })
        .count();
    assert_eq!(
        bt_requires_warnings, 1,
        "label-scoped ^BY defaults should not leak into the next label: {:?}",
        vr.issues
    );
}

#[test]
fn gs_height_width_resolve_from_cf_defaults() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^CFA,24,12^GS^FDx^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::REQUIRED_MISSING),
        "^GS should resolve h/w defaults from ^CF via explicit state keys: {:?}",
        vr.issues
    );
}

#[test]
fn required_arg_with_ambiguous_default_from_without_state_key_is_missing() {
    let mut tables = (*common::TABLES).clone();
    let bo_cmd = tables
        .commands
        .iter_mut()
        .find(|cmd| cmd.codes.iter().any(|c| c == "^BO"))
        .expect("^BO command entry should exist");
    let bo_args = bo_cmd
        .args
        .as_mut()
        .expect("^BO should have args in generated tables");
    for arg_union in bo_args {
        if let zpl_toolchain_spec_tables::ArgUnion::Single(arg) = arg_union
            && arg.key.as_deref() == Some("a")
        {
            arg.optional = false;
            arg.default_from_state_key = None;
            arg.default = None;
            arg.default_by_dpi = None;
        }
    }

    // ^FW has multiple effects.sets keys, so defaultFrom is ambiguous without
    // defaultFromStateKey and should not satisfy required presence.
    let result = parse_with_tables("^XA^FWR^BO,2,N,0,N,1^FDtest^FS^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::REQUIRED_MISSING || d.id == codes::REQUIRED_EMPTY),
        "ambiguous unresolved defaultFrom should not suppress required-arg diagnostics: {:?}",
        vr.issues,
    );
}

// ─── ZPL2306: Serialization Without Field Number ─────────────────────────────

#[test]
fn diag_zpl2306_serial_without_fn() {
    let tables = &*common::TABLES;
    // ^SN in a field without ^FN
    let result = parse_with_tables("^XA^FO10,10^SN001,1,Y^FD001^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::SERIALIZATION_WITHOUT_FIELD_NUMBER),
        "^SN without ^FN should emit ZPL2306: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2306_serial_with_fn_passes() {
    let tables = &*common::TABLES;
    // ^SN in a field WITH ^FN — should NOT trigger
    let result = parse_with_tables("^XA^FO10,10^FN1^SN001,1,Y^FD001^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::SERIALIZATION_WITHOUT_FIELD_NUMBER),
        "^SN with ^FN should not emit ZPL2306: {:?}",
        vr.issues,
    );
}

// ─── ZPL2307: ^GF Data Length Mismatch ───────────────────────────────────────

#[test]
fn diag_zpl2307_gf_ascii_hex_mismatch() {
    let tables = &*common::TABLES;
    // ^GF with ASCII hex (A): 3 bytes = 6 hex chars expected, but only 4 provided
    let result = parse_with_tables("^XA^GFA,3,3,1,AABB^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "^GF with mismatched ASCII hex data length should emit ZPL2307: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2307_gf_ascii_hex_correct() {
    let tables = &*common::TABLES;
    // ^GF with ASCII hex (A): 3 bytes = 6 hex chars, data is exactly 6 chars
    let result = parse_with_tables("^XA^GFA,3,3,1,AABBCC^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "^GF with correct ASCII hex data length should not emit ZPL2307: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2307_gf_binary_mismatch() {
    let tables = &*common::TABLES;
    // ^GF with binary (B): 4 bytes declared, but data is 2 bytes
    let result = parse_with_tables("^XA^GFB,4,4,1,AB^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "^GF with mismatched binary data length should emit ZPL2307: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2307_gf_compressed_skipped() {
    let tables = &*common::TABLES;
    // ^GF with compressed (C): should NOT emit ZPL2307 — compression makes length unpredictable
    let result = parse_with_tables("^XA^GFC,100,100,10,ABC^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "^GF with compressed format should not emit ZPL2307: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2307_gf_multiline_correct() {
    let tables = &*common::TABLES;
    // ^GF with 8 bytes declared = 16 ASCII hex chars.
    // 8 chars inline (FFAA5500) + 8 chars continuation (FFAA5500) = 16 total.
    let input = "^XA^GFA,8,8,1,FFAA5500\nFFAA5500\n^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "^GF with correct total data across continuation should not emit ZPL2307: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2307_gf_multiline_mismatch() {
    let tables = &*common::TABLES;
    // ^GF with 8 bytes declared = 16 ASCII hex chars.
    // Only 4 chars inline (FFAA) + 4 chars continuation (5500) = 8 total, which is wrong.
    let input = "^XA^GFA,8,8,1,FFAA\n5500\n^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "^GF with mismatched total data across continuation should emit ZPL2307: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2307_gf_inline_only_regression() {
    let tables = &*common::TABLES;
    // Single-line ^GF that was correct before — must still pass (regression guard)
    let input = "^XA^GFA,4,4,1,AABBCCDD^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::GF_DATA_LENGTH_MISMATCH),
        "single-line ^GF with correct length should not emit ZPL2307 (regression): {:?}",
        vr.issues,
    );
}

// ─── ZPL2308: Graphics Bounds Check ──────────────────────────────────────────

#[test]
fn diag_zpl2308_gf_overflows_width() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^PW400 sets width to 400. ^FO300,0 sets position.
    // ^GF with bytes_per_row=20 → graphic_width = 20*8 = 160 dots.
    // 300 + 160 = 460 > 400 → should overflow.
    // Using ASCII hex (A): graphic_field_count=20, bytes_per_row=20 → 1 row.
    // data: 40 hex chars (20 bytes * 2 hex/byte)
    let data = "FF".repeat(20);
    let input = format!("^XA^PW400^FO300,0^GFA,20,20,20,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::GF_BOUNDS_OVERFLOW),
        "^GF overflowing label width should emit ZPL2308: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2308_gf_overflows_height() {
    let tables = &*common::TABLES;
    // ^FO0,50 sets position. ^GF with graphic_field_count=200, bytes_per_row=10.
    // graphic_height = 200/10 = 20 rows. graphic_width = 10*8 = 80 dots.
    // 50 + 20 = 70 > 60 → should overflow.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800,"height_dots":60}}"#,
    );
    let data = "FF".repeat(200);
    let input = format!("^XA^FO0,50^GFA,200,200,10,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::GF_BOUNDS_OVERFLOW),
        "^GF overflowing label height should emit ZPL2308: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2308_gf_fits_no_diagnostic() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^FO0,0 + ^GF with bytes_per_row=10: graphic_width=80, graphic_height=10.
    // 0+80 ≤ 800, 0+10 ≤ 1200 → fits fine.
    let data = "FF".repeat(100);
    let input = format!("^XA^FO0,0^GFA,100,100,10,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::GF_BOUNDS_OVERFLOW),
        "^GF that fits should not emit ZPL2308: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2308_gf_bounds_respect_mu_inches_conversion() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^MUI converts ^FO coordinates to dots. ^FO4,0 => x=812 dots at 203 dpi.
    // ^GF width is 10 bytes/row => 80 dots; 812 + 80 > 800, so this must overflow.
    let data = "FF".repeat(10);
    let input = format!("^XA^MUI,203,203^FO4,0^GFA,10,10,10,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::GF_BOUNDS_OVERFLOW),
        "^MU inches conversion should apply to ^GF bounds checks (ZPL2308): {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2308_gf_bounds_respect_mu_mm_conversion() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // ^MUM converts millimeters to dots. ^FO100,0 => ~799 dots at 203 dpi.
    // ^GF width is 80 dots, so effective max x exceeds 800.
    let data = "FF".repeat(10);
    let input = format!("^XA^MUM^FO100,0^GFA,10,10,10,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::GF_BOUNDS_OVERFLOW),
        "^MU mm conversion should apply to ^GF bounds checks (ZPL2308): {:?}",
        vr.issues,
    );
}

// ─── ZPL2309: Graphics Memory Estimation ─────────────────────────────────────

#[test]
fn diag_zpl2309_gf_exceeds_memory() {
    let tables = &*common::TABLES;
    // Profile with 1 KB RAM (1024 bytes). Two ^GF with 600 bytes each = 1200 > 1024.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800,"height_dots":1200},"memory":{"ram_kb":1}}"#,
    );
    let data1 = "FF".repeat(600);
    let data2 = "FF".repeat(600);
    let input = format!(
        "^XA^FO0,0^GFA,600,600,10,{}^FS^FO0,100^GFA,600,600,10,{}^FS^XZ",
        data1, data2
    );
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::GF_MEMORY_EXCEEDED),
        "total ^GF bytes exceeding RAM should emit ZPL2309: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2309_gf_no_warning_without_profile() {
    let tables = &*common::TABLES;
    // No profile → no memory check.
    let data = "FF".repeat(600);
    let input = format!("^XA^FO0,0^GFA,600,600,10,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::GF_MEMORY_EXCEEDED),
        "^GF without profile should not emit ZPL2309: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2309_gf_within_memory_no_warning() {
    let tables = &*common::TABLES;
    // Profile with 512 KB RAM. ^GF with 100 bytes — well within limits.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"memory":{"ram_kb":512}}"#,
    );
    let data = "FF".repeat(100);
    let input = format!("^XA^FO0,0^GFA,100,100,10,{}^FS^XZ", data);
    let result = parse_with_tables(&input, Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::GF_MEMORY_EXCEEDED),
        "^GF within RAM should not emit ZPL2309: {:?}",
        vr.issues,
    );
}

// ─── ZPL2310: Missing Explicit Dimensions ────────────────────────────────────

#[test]
fn diag_zpl2310_missing_pw_ll_with_profile() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // Profile provides dimensions but label has no ^PW or ^LL
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::MISSING_EXPLICIT_DIMENSIONS),
        "label without ^PW/^LL with profile should emit ZPL2310: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2310_has_pw_ll_no_diagnostic() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // Label has both ^PW and ^LL — should NOT trigger
    let result = parse_with_tables("^XA^PW800^LL1200^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::MISSING_EXPLICIT_DIMENSIONS),
        "label with ^PW and ^LL should not emit ZPL2310: {:?}",
        vr.issues,
    );
}

// ─── ZPL2311: Object Bounds (Text/Barcode Overflow) ───────────────────────────

#[test]
fn diag_zpl2311_text_overflow_x() {
    let tables = &*common::TABLES;
    // Label 100 dots wide. Text at x=80 with 30-dot chars (^A0N,30,30) + 20 chars = 600 dots
    // → would overflow right. Use short label: 100 wide, text at 50 with 10-char "0123456789"
    // at 30x30 = 300 dots → 50+300=350 > 100.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":100,"height_dots":200}}"#,
    );
    let result = parse_with_tables(
        "^XA^PW100^LL200^CF0,30,30^FO50,10^FD0123456789^FS^XZ",
        Some(tables),
    );
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::OBJECT_BOUNDS_OVERFLOW),
        "text extending beyond label width should emit ZPL2311: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2311_barcode_overflow_y() {
    let tables = &*common::TABLES;
    // Label 60 dots tall. Barcode at y=50 with ^BY height 30 → 50+30=80 > 60.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":400,"height_dots":60}}"#,
    );
    let result = parse_with_tables(
        "^XA^PW400^LL60^BY2,3,30^FO10,50^BCN,30^FDABC123^FS^XZ",
        Some(tables),
    );
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::OBJECT_BOUNDS_OVERFLOW),
        "barcode extending beyond label height should emit ZPL2311: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2311_text_within_bounds_no_diagnostic() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    let result = parse_with_tables(
        "^XA^PW800^LL1200^CF0,24,24^FO50,50^FDHello^FS^XZ",
        Some(tables),
    );
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::OBJECT_BOUNDS_OVERFLOW),
        "text within bounds should not emit ZPL2311: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2311_no_bounds_skips_check() {
    let tables = &*common::TABLES;
    // No profile, no ^PW/^LL → no bounds → no ZPL2311
    let result = parse_with_tables(
        "^XA^FO50,50^FDVeryLongTextThatCouldOverflow^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::OBJECT_BOUNDS_OVERFLOW),
        "no bounds should skip ZPL2311: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2310_no_profile_no_diagnostic() {
    let tables = &*common::TABLES;
    // No profile → no ZPL2310
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::MISSING_EXPLICIT_DIMENSIONS),
        "label without profile should not emit ZPL2310: {:?}",
        vr.issues,
    );
}

#[test]
fn diag_zpl2310_partial_pw_only() {
    let tables = &*common::TABLES;
    let profile = common::profile_800x1200();
    // Label has ^PW but not ^LL — should emit ZPL2310 for missing ^LL
    let result = parse_with_tables("^XA^PW800^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    let diag = vr
        .issues
        .iter()
        .find(|d| d.id == codes::MISSING_EXPLICIT_DIMENSIONS);
    assert!(
        diag.is_some(),
        "label with only ^PW should emit ZPL2310: {:?}",
        vr.issues,
    );
    let ctx = diag.unwrap().context.as_ref().expect("should have context");
    assert!(
        ctx.get("missing_commands").unwrap().contains("^LL"),
        "missing_commands should include ^LL: {:?}",
        ctx,
    );
    assert!(
        !ctx.get("missing_commands").unwrap().contains("^PW"),
        "missing_commands should not include ^PW: {:?}",
        ctx,
    );
}

#[test]
fn diag_zpl2310_profile_no_page_no_diagnostic() {
    let tables = &*common::TABLES;
    // Profile without page dimensions — should NOT trigger ZPL2310
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::MISSING_EXPLICIT_DIMENSIONS),
        "profile without page dimensions should not emit ZPL2310: {:?}",
        vr.issues,
    );
}

// ─── ZPL2401/2402: Barcode Validation ────────────────────────────────────────

#[test]
fn barcode_fd_ean13_valid() {
    let tables = &*common::TABLES;
    // ^BE (EAN-13) with exactly 12 digits — should pass
    let input = "^XA^FO10,10^BE,50,N,N^FD123456789012^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::BARCODE_INVALID_CHAR || d.id == codes::BARCODE_DATA_LENGTH),
        "valid EAN-13 data should not trigger barcode diagnostics: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_ean13_invalid_chars() {
    let tables = &*common::TABLES;
    // ^BE (EAN-13) with non-numeric characters — should flag ZPL2401
    let input = "^XA^FO10,10^BE,50,N,N^FDABCDEF123456^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::BARCODE_INVALID_CHAR),
        "non-numeric EAN-13 data should trigger ZPL2401: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_ean13_wrong_length() {
    let tables = &*common::TABLES;
    // ^BE (EAN-13) with wrong length — should flag ZPL2402
    let input = "^XA^FO10,10^BE,50,N,N^FD12345^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "wrong-length EAN-13 data should trigger ZPL2402: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_code39_valid() {
    let tables = &*common::TABLES;
    // ^B3 (Code 39) with valid alphanumeric — should pass
    let input = "^XA^FO10,10^B3,N,50,N,N^FDTEST 123^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::BARCODE_INVALID_CHAR),
        "valid Code 39 data should not trigger ZPL2401: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_code39_invalid_chars() {
    let tables = &*common::TABLES;
    // ^B3 (Code 39) with lowercase (not in standard charset) — should flag ZPL2401
    let input = "^XA^FO10,10^B3,N,50,N,N^FDhello^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::BARCODE_INVALID_CHAR),
        "lowercase in Code 39 data should trigger ZPL2401: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_interleaved2of5_even_parity() {
    let tables = &*common::TABLES;
    // ^B2 (Interleaved 2 of 5) with even digit count — should pass
    let input = "^XA^FO10,10^B2,50^FD1234^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "even digit count for I2of5 should pass: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_interleaved2of5_odd_parity() {
    let tables = &*common::TABLES;
    // ^B2 (Interleaved 2 of 5) with odd digit count — should flag ZPL2402
    let input = "^XA^FO10,10^B2,50^FD123^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "odd digit count for I2of5 should trigger ZPL2402: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_no_barcode_no_validation() {
    let tables = &*common::TABLES;
    // No barcode command — ^FD with any content should not trigger barcode diagnostics
    let input = "^XA^FO10,10^FDanything goes here!@#$^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::BARCODE_INVALID_CHAR || d.id == codes::BARCODE_DATA_LENGTH),
        "no barcode command should not trigger barcode diagnostics: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_validation_skipped_when_fh_active() {
    let tables = &*common::TABLES;
    // ^FH active: barcode content checks are intentionally skipped because raw
    // text includes escape sequences.
    let input = "^XA^FO10,10^B2,50^FH^FD123^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "^FH should suppress barcode parity/length validation: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_bs_allowed_lengths_invalid_three() {
    let tables = &*common::TABLES;
    // ^BS allows only 2 or 5 digits.
    let input = "^XA^FO10,10^BSN,50,Y,N^FD123^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "^BS with 3 digits should violate allowed discrete lengths: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_bs_allowed_lengths_valid_two() {
    let tables = &*common::TABLES;
    let input = "^XA^FO10,10^BSN,50,Y,N^FD12^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "^BS with 2 digits should pass allowed discrete lengths: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_combined_inline_and_multiline_validates_as_one_payload() {
    let tables = &*common::TABLES;
    // Combined payload is 12 digits for EAN-13, split across inline and FieldData nodes.
    let input = "^XA^FO10,10^BE,50,N,N^FD123\n456789012^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "split field data should be validated as one combined barcode payload: {:?}",
        vr.issues,
    );
}

#[test]
fn xg_path_form_does_not_emit_invalid_enum() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^XGR:IMAGE.GRF,2,2^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::INVALID_ENUM),
        "^XG d:o.x path form should not be treated as invalid enum: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_multiple_barcodes_same_field_are_validated_separately() {
    let tables = &*common::TABLES;
    // First barcode (^BC) accepts alphanumeric payload; second (^B2) requires
    // numeric even-length payload. They must be validated against their own
    // corresponding ^FD segments.
    let input = "^XA^BY2^FO10,10^BCN,50,Y,N,N^FD123ABC^B2,50^FD4567^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::BARCODE_INVALID_CHAR || d.id == codes::BARCODE_DATA_LENGTH),
        "multiple barcodes in one field should not cross-validate payloads: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_bs_requires_by() {
    let tables = &*common::TABLES;
    let input = "^XA^FO10,10^BSN,50,Y,N^FD12^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::REQUIRED_COMMAND),
        "^BS should require ^BY defaults in label: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_msi_too_long_triggers_length() {
    let tables = &*common::TABLES;
    // ^BM max length is 14.
    let input = "^XA^BY2^FO10,10^BM^FD123456789012345^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "^BM payload over max length should trigger barcode length diagnostic: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_diagnostics_point_to_fd_span_not_fs() {
    let tables = &*common::TABLES;
    let input = "^XA^BY2^FO10,10^B2,50^FD123^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);

    let expected_fd_span = result.ast.labels[0]
        .nodes
        .iter()
        .find_map(|n| match n {
            zpl_toolchain_core::grammar::ast::Node::Command { code, span, .. } if code == "^FD" => {
                Some(*span)
            }
            _ => None,
        })
        .expect("test input should include ^FD command with span");

    let diag = vr
        .issues
        .iter()
        .find(|d| d.id == codes::BARCODE_DATA_LENGTH)
        .expect("odd-length I2of5 should emit barcode length diagnostic");
    assert_eq!(
        diag.span,
        Some(expected_fd_span),
        "barcode diagnostic span should point at field data, not ^FS: {:?}",
        vr.issues
    );
}

#[test]
fn hz_extended_form_does_not_fail_arity() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^HZO,E:TEST.GRF,N", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::ARITY),
        "^HZ extended form should be accepted without arity diagnostic: {:?}",
        vr.issues,
    );
}

#[test]
fn hz_uppercase_info_type_no_invalid_enum() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^HZO,E:TEST.GRF,N", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::INVALID_ENUM),
        "^HZ uppercase info type should not trigger INVALID_ENUM: {:?}",
        vr.issues,
    );
}

#[test]
fn xg_path_form_no_invalid_enum_or_arity() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^XGR:IMAGE.GRF,2,2^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::INVALID_ENUM || d.id == codes::ARITY),
        "^XG path-form should parse without enum/arity diagnostics: {:?}",
        vr.issues,
    );
}

#[test]
fn mn_no_args_uses_default_without_required_missing() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^MN^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::REQUIRED_MISSING),
        "^MN should allow omitted mode using default N: {:?}",
        vr.issues,
    );
}

#[test]
fn mu_units_persist_across_labels() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800,"height_dots":1200}}"#,
    );
    // Label 1 sets inches. Label 2 uses ^PW without resetting units.
    // 4 inches at 203 dpi = 812 dots > profile width 800.
    let result = parse_with_tables("^XA^MUI^XZ^XA^PW4^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "^MU session state should persist across labels: {:?}",
        vr.issues,
    );
}

#[test]
fn ft_opens_field_and_overlaps_without_fs() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^FT20,20^FDx^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::FIELD_NOT_CLOSED),
        "^FT should be treated as field-opening and overlap without prior ^FS: {:?}",
        vr.issues,
    );
}

#[test]
fn barcode_fd_multiline_segments_are_validated_as_combined_payload() {
    let tables = &*common::TABLES;
    // Combined payload is 12 digits and valid for ^BE.
    let input = "^XA^FO10,10^BE,50,N,N^FD123\n456789012^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::BARCODE_DATA_LENGTH),
        "split field data should be validated as one combined payload: {:?}",
        vr.issues,
    );
}

// ─── ZPL3001: Note ───────────────────────────────────────────────────────────

#[test]
fn diag_zpl3001_note_emitted() {
    let tables = &*common::TABLES;
    // ^BY has note constraints about firmware behavior
    let result = parse_with_tables("^XA^BY2,3.0,10^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let notes: Vec<_> = vr.issues.iter().filter(|d| d.id == codes::NOTE).collect();
    assert!(
        !notes.is_empty(),
        "commands with note constraints should emit ZPL3001: {:?}",
        vr.issues,
    );
}

#[test]
fn note_ls_before_first_fs_is_not_emitted() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^LS0^FO10,10^FDX^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::NOTE
            && d.context
                .as_ref()
                .and_then(|c| c.get("command"))
                .is_some_and(|v| v == "^LS")),
        "^LS before first ^FS should not emit compatibility note: {:?}",
        vr.issues
    );
}

#[test]
fn note_ls_after_first_fs_is_emitted_as_warn() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^FDX^FS^LS0^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let ls_note = vr.issues.iter().find(|d| {
        d.id == codes::NOTE
            && d.context
                .as_ref()
                .and_then(|c| c.get("command"))
                .is_some_and(|v| v == "^LS")
    });
    assert!(
        ls_note.is_some(),
        "expected ^LS compatibility note: {:?}",
        vr.issues
    );
    assert_eq!(ls_note.expect("checked above").severity, Severity::Warn);
}

// ─── Structural Validation ───────────────────────────────────────────────────

#[test]
fn structural_valid_field_block_no_warnings() {
    let tables = &*common::TABLES;
    // Properly structured: ^FO -> ^FD -> ^FS
    let result = parse_with_tables("^XA^FO50,50^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    let structural: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| d.id.starts_with("ZPL22"))
        .collect();
    assert!(
        structural.is_empty(),
        "well-formed field block should have no structural warnings: {:?}",
        structural,
    );
}

#[test]
fn structural_multiple_fields_valid() {
    let tables = &*common::TABLES;
    // Multiple properly structured fields
    let result = parse_with_tables(
        "^XA^FO10,10^FDFirst^FS^FO10,50^FDSecond^FS^XZ",
        Some(tables),
    );
    let vr = validate::validate(&result.ast, tables);
    let structural: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| d.id.starts_with("ZPL22"))
        .collect();
    assert!(
        structural.is_empty(),
        "multiple well-formed field blocks should have no structural warnings: {:?}",
        structural,
    );
}

// ─── Profile Edge Cases ─────────────────────────────────────────────────────

#[test]
fn profile_none_page_skips_constraints() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);
    let result = parse_with_tables("^XA^PW9999^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    // No profile constraint should fire since page is None
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "profile with no page should not trigger constraints: {:?}",
        vr.issues,
    );
}

#[test]
fn profile_missing_height_skips_ll_constraint() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800}}"#,
    );
    let result = parse_with_tables("^XA^LL9999^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    // No height constraint should fire since height_dots is not in profile
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::PROFILE_CONSTRAINT && d.message.contains("height")),
        "profile without height_dots should not trigger ^LL constraint: {:?}",
        vr.issues,
    );
}

#[test]
fn profile_height_used_for_position_bounds() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800,"height_dots":600}}"#,
    );
    let result = parse_with_tables("^XA^FO100,700^XZ", Some(tables));
    let vr = validate_with_profile(&result.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::POSITION_OUT_OF_BOUNDS),
        "y position 700 should exceed profile height 600: {:?}",
        vr.issues,
    );
}

#[test]
fn profile_300dpi_width_constraint() {
    let tables = &*common::TABLES;
    // 300 dpi profile has width_dots: 1218
    let profile = common::profile_from_json(
        r#"{"id":"zebra-generic-300","schema_version":"1.0.0","dpi":300,"page":{"width_dots":1218,"height_dots":1800}}"#,
    );

    // ^PW1300 exceeds the 300 dpi profile width (1218 dots)
    let ast = parse_with_tables("^XA^PW1300^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "^PW1300 should exceed 300 dpi profile width 1218: {:?}",
        vr.issues,
    );

    // ^PW1200 is within the 300 dpi profile width (1218 dots)
    let ast2 = parse_with_tables("^XA^PW1200^XZ", Some(tables));
    let vr2 = validate_with_profile(&ast2.ast, tables, Some(&profile));
    assert!(
        !vr2.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "^PW1200 should be within 300 dpi profile width 1218: {:?}",
        vr2.issues,
    );
}

#[test]
fn all_profile_constraint_fields_are_resolvable() {
    let tables = &*common::TABLES;

    // Build a fully-populated test profile so every field resolves to Some.
    let profile = common::profile_from_json(
        r#"{
        "id": "test",
        "schema_version": "1.1.0",
        "dpi": 203,
        "page": { "width_dots": 812, "height_dots": 1218 },
        "speed_range": { "min": 2, "max": 8 },
        "darkness_range": { "min": 0, "max": 30 },
        "features": {
            "cutter": true, "peel": true, "rewinder": true,
            "applicator": true, "rfid": true, "rtc": true,
            "battery": true, "zbi": true, "lcd": true, "kiosk": true
        },
        "media": {
            "print_method": "both",
            "supported_modes": ["T", "P", "C"],
            "supported_tracking": ["N", "Y", "M"]
        },
        "memory": { "ram_kb": 512, "flash_kb": 65536, "firmware_version": "V60.19.15Z" }
    }"#,
    );

    // Collect all unique profileConstraint.field values from all commands.
    let mut profile_fields: std::collections::HashSet<String> = std::collections::HashSet::new();
    for cmd in &tables.commands {
        if let Some(args) = &cmd.args {
            for arg_union in args {
                let args_to_check: Vec<&zpl_toolchain_spec_tables::Arg> = match arg_union {
                    zpl_toolchain_spec_tables::ArgUnion::Single(a) => vec![a.as_ref()],
                    zpl_toolchain_spec_tables::ArgUnion::OneOf { one_of } => {
                        one_of.iter().collect()
                    }
                };
                for arg in args_to_check {
                    if let Some(pc) = &arg.profile_constraint {
                        profile_fields.insert(pc.field.clone());
                    }
                }
            }
        }
    }

    assert!(
        !profile_fields.is_empty(),
        "Expected at least one profileConstraint.field in the spec tables — \
         is the test data missing?"
    );

    // Verify each field is resolvable.
    for field in &profile_fields {
        let resolved = zpl_toolchain_core::validate::resolve_profile_field(&profile, field);
        assert!(
            resolved.is_some(),
            "profileConstraint references field '{}' but resolve_profile_field returns None for it. \
             Add an entry to PROFILE_FIELD_REGISTRY for this field path.",
            field
        );
    }
}

#[test]
fn diag_profile_constraint_sd_exceeds_darkness() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"darkness_range":{"min":0,"max":30}}"#,
    );

    // ~SD31 exceeds darkness_range.max (30)
    let ast = parse_with_tables("^XA~SD31^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::PROFILE_CONSTRAINT && d.message.contains("darkness")),
        "~SD31 should exceed darkness_range.max 30: {:?}",
        vr.issues,
    );

    // ~SD25 is within darkness_range.max (30)
    let ast2 = parse_with_tables("^XA~SD25^XZ", Some(tables));
    let vr2 = validate_with_profile(&ast2.ast, tables, Some(&profile));
    assert!(
        !vr2.issues
            .iter()
            .any(|d| d.id == codes::PROFILE_CONSTRAINT && d.message.contains("darkness")),
        "~SD25 should be within darkness_range.max 30: {:?}",
        vr2.issues,
    );
}

#[test]
fn profile_all_none_skips_all_constraints() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);

    // All optional profile fields are None — no profileConstraint should fire
    let ast = parse_with_tables("^XA^PW9999^LL9999~SD99^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PROFILE_CONSTRAINT),
        "all-None profile should skip all profile constraints: {:?}",
        vr.issues,
    );
}

// ─── Printer Gate Enforcement ────────────────────────────────────────────────

#[test]
fn printer_gate_command_level_violation() {
    let tables = &*common::TABLES;
    // This test verifies that commands WITHOUT printerGates don't trigger ZPL1402
    // even when the profile has features set to false.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{"cutter":false}}"#,
    );
    // ^PW has no printerGates, so it should NOT trigger ZPL1402
    let ast = parse_with_tables("^XA^PW100^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "commands without printerGates should not trigger ZPL1402: {:?}",
        vr.issues,
    );
}

#[test]
fn printer_gate_no_features_skips() {
    let tables = &*common::TABLES;
    // Profile without features should skip all gate checks
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);
    let ast = parse_with_tables("^XA^PW100^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "profile without features should skip gate checks: {:?}",
        vr.issues,
    );
}

#[test]
fn printer_gate_command_fires_when_feature_false() {
    let tables = &*common::TABLES;
    // ^RF has printerGates: ["rfid"] in spec. Profile has rfid=false.
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{"rfid":false}}"#,
    );
    let ast = parse_with_tables("^XA^RFW,H,0,1,E^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "^RF with rfid=false should trigger ZPL1402: {:?}",
        vr.issues,
    );
}

#[test]
fn printer_gate_command_skips_when_feature_true() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{"rfid":true}}"#,
    );
    let ast = parse_with_tables("^XA^RFW,H,0,1,E^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "^RF with rfid=true should NOT trigger ZPL1402: {:?}",
        vr.issues,
    );
}

// ─── Enum Value-Level Gate Tests ─────────────────────────────────────────────

#[test]
fn printer_gate_enum_value_fires_when_feature_false() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{"cutter":false}}"#,
    );
    let ast = parse_with_tables("^XA^MMC^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "^MMC with cutter=false should trigger ZPL1402: {:?}",
        vr.issues,
    );
}

#[test]
fn printer_gate_enum_value_skips_ungated_value() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{"cutter":false}}"#,
    );
    let ast = parse_with_tables("^XA^MMT^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "^MMT should not trigger ZPL1402 even with cutter=false: {:?}",
        vr.issues,
    );
}

#[test]
fn printer_gate_enum_value_skips_unknown_feature() {
    let tables = &*common::TABLES;
    // cutter is None (unknown) — should skip gate check
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{}}"#,
    );
    let ast = parse_with_tables("^XA^MMC^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues.iter().any(|d| d.id == codes::PRINTER_GATE),
        "^MMC with cutter=None should NOT trigger ZPL1402: {:?}",
        vr.issues,
    );
}

// ─── Media Mode Validation (ZPL1403) ─────────────────────────────────────────

#[test]
fn media_mode_unsupported_mm() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"supported_modes":["T"]}}"#,
    );
    // "C" (cutter) not in supported_modes ["T"]
    let ast = parse_with_tables("^XA^MMC^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::MEDIA_MODE_UNSUPPORTED),
        "^MMC should trigger ZPL1403 when C not in supported_modes: {:?}",
        vr.issues,
    );
}

#[test]
fn media_mode_supported_mm() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"supported_modes":["T"]}}"#,
    );
    let ast = parse_with_tables("^XA^MMT^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::MEDIA_MODE_UNSUPPORTED),
        "^MMT should not trigger ZPL1403 when T in supported_modes: {:?}",
        vr.issues,
    );
}

#[test]
fn media_tracking_unsupported_mn() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"supported_tracking":["N","Y"]}}"#,
    );
    let ast = parse_with_tables("^XA^MNM^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::MEDIA_MODE_UNSUPPORTED),
        "^MNM should trigger ZPL1403 when M not in supported_tracking: {:?}",
        vr.issues,
    );
}

#[test]
fn media_print_method_conflict_mt() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"print_method":"direct_thermal"}}"#,
    );
    // T = thermal transfer, but profile is direct_thermal only
    let ast = parse_with_tables("^XA^MTT^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::MEDIA_MODE_UNSUPPORTED),
        "^MTT should trigger ZPL1403 when profile is direct_thermal: {:?}",
        vr.issues,
    );
}

#[test]
fn media_print_method_both_allows_all() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"print_method":"both"}}"#,
    );
    let ast = parse_with_tables("^XA^MTT^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::MEDIA_MODE_UNSUPPORTED),
        "^MTT should not trigger ZPL1403 when print_method is Both: {:?}",
        vr.issues,
    );
}

#[test]
fn media_no_profile_media_skips() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);
    let ast = parse_with_tables("^XA^MMC^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::MEDIA_MODE_UNSUPPORTED),
        "no profile media should skip ZPL1403: {:?}",
        vr.issues,
    );
}

// ─── defaultByDpi Loading ────────────────────────────────────────────────────

#[test]
fn default_by_dpi_loaded_in_spec_tables() {
    let tables = &*common::TABLES;
    // ^BQ should have defaultByDpi on its magnification arg
    let bq = tables
        .cmd_by_code("^BQ")
        .expect("^BQ should exist in tables");
    let args = bq.args.as_ref().expect("^BQ should have args");
    let mag_arg = args
        .iter()
        .filter_map(|au| match au {
            zpl_toolchain_spec_tables::ArgUnion::Single(a) => Some(a.as_ref()),
            zpl_toolchain_spec_tables::ArgUnion::OneOf { one_of } => one_of
                .iter()
                .find(|a| a.name.as_deref() == Some("magnification_factor")),
        })
        .find(|a| a.name.as_deref() == Some("magnification_factor"))
        .expect("^BQ should have magnification_factor arg");
    assert!(
        mag_arg.default_by_dpi.is_some(),
        "^BQ.magnification_factor should have defaultByDpi"
    );
    let dpi_map = mag_arg.default_by_dpi.as_ref().unwrap();
    assert_eq!(dpi_map.get("203").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(dpi_map.get("300").and_then(|v| v.as_i64()), Some(3));
    assert_eq!(dpi_map.get("600").and_then(|v| v.as_i64()), Some(6));
}

// ─── Diagnostic Structured Context ───────────────────────────────────────────

#[test]
fn context_arity() {
    let tables = &*common::TABLES;
    // ^PW has arity 1 — give it 3 args
    let ast = parse_with_tables("^XA^PW100,200,300^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    let d = find_diag(&vr.issues, codes::ARITY);
    let ctx = d
        .context
        .as_ref()
        .expect("arity diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^PW");
    assert_eq!(ctx.get("arity").unwrap(), "1");
    assert_eq!(ctx.get("actual").unwrap(), "3");
}

#[test]
fn context_out_of_range() {
    let tables = &*common::TABLES;
    // ^PW range is typically [1, 32000] — give it 0
    let ast = parse_with_tables("^XA^PW0^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    let d = find_diag(&vr.issues, codes::OUT_OF_RANGE);
    let ctx = d
        .context
        .as_ref()
        .expect("out_of_range diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^PW");
    assert!(ctx.contains_key("min"), "should have min key");
    assert!(ctx.contains_key("max"), "should have max key");
    assert_eq!(ctx.get("value").unwrap(), "0");
}

#[test]
fn context_invalid_enum() {
    let tables = &*common::TABLES;
    // ^BC orientation (arg 0) must be N/R/I/B; give it X
    let ast = parse_with_tables("^XA^BY2^BCX^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    let d = find_diag(&vr.issues, codes::INVALID_ENUM);
    let ctx = d
        .context
        .as_ref()
        .expect("invalid_enum diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^BC");
    assert_eq!(ctx.get("value").unwrap(), "X");
    assert!(ctx.contains_key("arg"), "should have arg key");
}

#[test]
fn context_profile_constraint() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":100}}"#,
    );
    let ast = parse_with_tables("^XA^PW200^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    let d = find_diag(&vr.issues, codes::PROFILE_CONSTRAINT);
    let ctx = d
        .context
        .as_ref()
        .expect("profile_constraint diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^PW");
    assert!(ctx.contains_key("field"), "should have field key");
    assert!(ctx.contains_key("op"), "should have op key");
    assert!(ctx.contains_key("limit"), "should have limit key");
    assert!(ctx.contains_key("actual"), "should have actual key");
}

#[test]
fn context_printer_gate_command_level() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"features":{"rfid":false}}"#,
    );
    // ^RF requires rfid gate
    let ast = parse_with_tables("^XA^RF^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    let d = find_diag(&vr.issues, codes::PRINTER_GATE);
    let ctx = d
        .context
        .as_ref()
        .expect("printer_gate diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^RF");
    assert_eq!(ctx.get("gate").unwrap(), "rfid");
    assert_eq!(ctx.get("level").unwrap(), "command");
    assert!(ctx.contains_key("profile"), "should have profile key");
}

#[test]
fn context_media_mode_unsupported() {
    let tables = &*common::TABLES;
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"supported_modes":["T"]}}"#,
    );
    let ast = parse_with_tables("^XA^MMC^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    let d = find_diag(&vr.issues, codes::MEDIA_MODE_UNSUPPORTED);
    let ctx = d
        .context
        .as_ref()
        .expect("media_mode diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^MM");
    assert_eq!(ctx.get("kind").unwrap(), "mode");
    assert_eq!(ctx.get("value").unwrap(), "C");
    assert!(ctx.contains_key("supported"), "should have supported key");
    assert!(ctx.contains_key("profile"), "should have profile key");
}

#[test]
fn media_mode_context_mt_carries_profile_method() {
    let tables = &*common::TABLES;
    // ^MT T (thermal transfer) with profile direct_thermal only — ZPL1403 media sanity
    let profile = common::profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"media":{"print_method":"direct_thermal"}}"#,
    );
    let ast = parse_with_tables("^XA^MTT^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    let d = find_diag(&vr.issues, codes::MEDIA_MODE_UNSUPPORTED);
    let ctx = d
        .context
        .as_ref()
        .expect("^MT diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^MT");
    assert_eq!(ctx.get("kind").unwrap(), "method");
    assert!(
        ctx.contains_key("profile_method"),
        "ZPL1403 ^MT should carry profile_method"
    );
}

#[test]
fn context_structural_field_data_without_origin() {
    let tables = &*common::TABLES;
    let ast = parse_with_tables("^XA^FDHello^FS^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    let d = find_diag(&vr.issues, codes::FIELD_DATA_WITHOUT_ORIGIN);
    let ctx = d
        .context
        .as_ref()
        .expect("structural diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^FD");
}

#[test]
fn context_empty_label_has_no_context() {
    let tables = &*common::TABLES;
    let ast = parse_with_tables("^XA^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    let d = find_diag(&vr.issues, codes::EMPTY_LABEL);
    assert!(
        d.context.is_none(),
        "empty label diagnostic should have no context (self-explanatory)"
    );
}

#[test]
fn context_host_command_in_label() {
    let tables = &*common::TABLES;
    let ast = parse_with_tables("^XA~TA000^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    let d = find_diag(&vr.issues, codes::HOST_COMMAND_IN_LABEL);
    let ctx = d
        .context
        .as_ref()
        .expect("host_command_in_label diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "~TA");
}

// ─── requires_field enforcement ──────────────────────────────────────────────

#[test]
fn requires_field_fh_outside_field_emits_warning() {
    let tables = &*common::TABLES;
    // ^FH outside a field (no preceding ^FO) should trigger ZPL2201
    let result = parse_with_tables("^XA^FH^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN && d.message.contains("^FH")),
        "^FH outside field should emit ZPL2201: {:?}",
        vr.issues,
    );
}

#[test]
fn requires_field_fh_inside_field_passes() {
    let tables = &*common::TABLES;
    // ^FH inside a field (after ^FO) should NOT trigger ZPL2201
    let result = parse_with_tables("^XA^FO10,10^FH^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN && d.message.contains("^FH")),
        "^FH inside field should not emit ZPL2201: {:?}",
        vr.issues,
    );
}

#[test]
fn requires_field_fr_outside_field_emits_warning() {
    let tables = &*common::TABLES;
    // ^FR outside a field should trigger ZPL2201
    let result = parse_with_tables("^XA^FR^FO10,10^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN && d.message.contains("^FR")),
        "^FR outside field should emit ZPL2201: {:?}",
        vr.issues,
    );
}

#[test]
fn requires_field_fr_inside_field_passes() {
    let tables = &*common::TABLES;
    // ^FR inside a field (after ^FO) should NOT trigger ZPL2201
    let result = parse_with_tables("^XA^FO10,10^FR^FDHello^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN && d.message.contains("^FR")),
        "^FR inside field should not emit ZPL2201: {:?}",
        vr.issues,
    );
}

#[test]
fn requires_field_sn_outside_field_emits_warning() {
    let tables = &*common::TABLES;
    // ^SN outside a field should trigger ZPL2201
    let result = parse_with_tables("^XA^SN001^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN && d.message.contains("^SN")),
        "^SN outside field should emit ZPL2201: {:?}",
        vr.issues,
    );
}

#[test]
fn requires_field_sf_outside_field_emits_warning() {
    let tables = &*common::TABLES;
    // ^SF outside a field should trigger ZPL2201
    let result = parse_with_tables("^XA^SFD^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN && d.message.contains("^SF")),
        "^SF outside field should emit ZPL2201: {:?}",
        vr.issues,
    );
}

#[test]
fn requires_field_sn_sf_inside_field_passes() {
    let tables = &*common::TABLES;
    // ^SN/^SF inside a field should NOT trigger ZPL2201
    let result = parse_with_tables("^XA^FO10,10^FN1^SN001^SF%d^FDStart^FS^XZ", Some(tables));
    let vr = validate::validate(&result.ast, tables);
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::FIELD_DATA_WITHOUT_ORIGIN
                && (d.message.contains("^SN") || d.message.contains("^SF"))),
        "^SN/^SF inside field should not emit ZPL2201: {:?}",
        vr.issues,
    );
}

// ─── ^MU Unit Conversion ────────────────────────────────────────────────────

#[test]
fn mu_units_inches_conversion() {
    let tables = &*common::TABLES;
    // Profile with DPI 203. ^MUI sets inches.
    // ^A height=5 in inches → 5*203=1015 dots, within [10,32000] — should pass.
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);
    let ast = parse_with_tables("^XA^MUI^FO10,10^AA,N,5^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^A")),
        "^A height=5 inches (1015 dots) should be in range with ^MUI: {:?}",
        vr.issues,
    );
}

#[test]
fn mu_units_mm_conversion() {
    let tables = &*common::TABLES;
    // Profile with DPI 203. ^MUM sets millimeters.
    // ^A height=2 in mm → 2*203/25.4 ≈ 15.98 dots, within [10,32000] — should pass.
    // ^A height=1 in mm → 1*203/25.4 ≈ 7.99 dots, below 10 — should fail.
    let profile = common::profile_from_json(r#"{"id":"test","schema_version":"1.0.0","dpi":203}"#);

    // 2mm should pass
    let ast = parse_with_tables("^XA^MUM^FO10,10^AA,N,2^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, Some(&profile));
    assert!(
        !vr.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^A")),
        "^A height=2 mm (~16 dots) should be in range with ^MUM: {:?}",
        vr.issues,
    );

    // 1mm should fail (≈8 dots, below min 10)
    let ast2 = parse_with_tables("^XA^MUM^FO10,10^AA,N,1^FDtest^FS^XZ", Some(tables));
    let vr2 = validate_with_profile(&ast2.ast, tables, Some(&profile));
    assert!(
        vr2.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^A")),
        "^A height=1 mm (~8 dots) should be out of range with ^MUM: {:?}",
        vr2.issues,
    );
}

#[test]
fn mu_units_dots_default() {
    let tables = &*common::TABLES;
    // No ^MU — default is dots. ^A height=5 is below [10,32000] — should flag.
    let ast = parse_with_tables("^XA^FO10,10^AA,N,5^FDtest^FS^XZ", Some(tables));
    let vr = validate_with_profile(&ast.ast, tables, None);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^A")),
        "^A height=5 dots should be out of range without ^MU: {:?}",
        vr.issues,
    );

    // ^A height=100 should be within range
    let ast2 = parse_with_tables("^XA^FO10,10^AA,N,100^FDtest^FS^XZ", Some(tables));
    let vr2 = validate_with_profile(&ast2.ast, tables, None);
    assert!(
        !vr2.issues
            .iter()
            .any(|d| d.id == codes::OUT_OF_RANGE && d.message.contains("^A")),
        "^A height=100 dots should be in range without ^MU: {:?}",
        vr2.issues,
    );
}

// ─── Synthetic Coverage for Currently Unused Spec Paths ──────────────────────

fn mutate_command_in_tables<F>(
    base: &zpl_toolchain_spec_tables::ParserTables,
    code: &str,
    f: F,
) -> zpl_toolchain_spec_tables::ParserTables
where
    F: FnOnce(&mut zpl_toolchain_spec_tables::CommandEntry),
{
    let mut tables = base.clone();
    let idx = tables
        .commands
        .iter()
        .position(|cmd| cmd.codes.iter().any(|c| c == code))
        .unwrap_or_else(|| panic!("expected command {code} in parser tables"));
    f(&mut tables.commands[idx]);
    tables
}

#[test]
fn diag_zpl1105_string_too_short_via_synthetic_constraint() {
    let tables = mutate_command_in_tables(&common::TABLES, "^BY", |cmd| {
        let args = cmd.args.as_mut().expect("^BY should have args");
        let first_arg = args.get_mut(0).expect("^BY first arg should exist");
        match first_arg {
            ArgUnion::Single(arg) => arg.min_length = Some(2),
            ArgUnion::OneOf { .. } => panic!("expected ^BY arg 0 to be single"),
        }
    });

    let result = parse_with_tables("^XA^BY1,2,10^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::STRING_TOO_SHORT),
        "expected STRING_TOO_SHORT from synthetic min_length: {:?}",
        vr.issues
    );
}

#[test]
fn diag_zpl1106_string_too_long_via_synthetic_constraint() {
    let tables = mutate_command_in_tables(&common::TABLES, "^BY", |cmd| {
        let args = cmd.args.as_mut().expect("^BY should have args");
        let first_arg = args.get_mut(0).expect("^BY first arg should exist");
        match first_arg {
            ArgUnion::Single(arg) => arg.max_length = Some(1),
            ArgUnion::OneOf { .. } => panic!("expected ^BY arg 0 to be single"),
        }
    });

    let result = parse_with_tables("^XA^BY12,2,10^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::STRING_TOO_LONG),
        "expected STRING_TOO_LONG from synthetic max_length: {:?}",
        vr.issues
    );
}

#[test]
fn diag_zpl2102_incompatible_command_via_synthetic_constraint() {
    let tables = mutate_command_in_tables(&common::TABLES, "^A", |cmd| {
        let constraints = cmd.constraints.get_or_insert_with(Vec::new);
        constraints.push(Constraint {
            kind: ConstraintKind::Incompatible,
            expr: Some("^FO".to_string()),
            message: "synthetic incompatible test".to_string(),
            severity: None,
            scope: None,
        });
    });

    let result = parse_with_tables("^XA^FO10,10^A0N,30,30^FDx^FS^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        vr.issues
            .iter()
            .any(|d| d.id == codes::INCOMPATIBLE_COMMAND),
        "expected INCOMPATIBLE_COMMAND from synthetic incompatible constraint: {:?}",
        vr.issues
    );
}

#[test]
fn diag_zpl2104_order_after_via_synthetic_constraint() {
    let tables = mutate_command_in_tables(&common::TABLES, "^A", |cmd| {
        let constraints = cmd.constraints.get_or_insert_with(Vec::new);
        constraints.push(Constraint {
            kind: ConstraintKind::Order,
            expr: Some("after:^FO".to_string()),
            message: "synthetic order-after test".to_string(),
            severity: None,
            scope: None,
        });
    });

    // ^A appears before ^FO so synthetic "after:^FO" should fail.
    let result = parse_with_tables("^XA^A0N,30,30^FO10,10^FDx^FS^XZ", Some(&tables));
    let vr = validate::validate(&result.ast, &tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::ORDER_AFTER),
        "expected ORDER_AFTER from synthetic order-after constraint: {:?}",
        vr.issues
    );
}

// ─── Diagnostic ID Compile-Time Safety ───────────────────────────────────────

/// Verify that all diagnostic codes used in the validator have corresponding
/// explain() entries in the diagnostics crate. This catches typos in diagnostic IDs.
#[test]
fn all_diagnostic_ids_have_explanations() {
    use zpl_toolchain_diagnostics::explain;

    // All diagnostic codes used in the validator and parser.
    // Keep this list in sync when adding new codes.
    let validator_codes = [
        codes::ARITY,
        codes::INVALID_ENUM,
        codes::EMPTY_FIELD_DATA,
        codes::STRING_TOO_SHORT,
        codes::STRING_TOO_LONG,
        codes::EXPECTED_INTEGER,
        codes::EXPECTED_NUMERIC,
        codes::EXPECTED_CHAR,
        codes::OUT_OF_RANGE,
        codes::ROUNDING_VIOLATION,
        codes::PROFILE_CONSTRAINT,
        codes::PRINTER_GATE,
        codes::MEDIA_MODE_UNSUPPORTED,
        codes::REQUIRED_MISSING,
        codes::REQUIRED_EMPTY,
        codes::REQUIRED_COMMAND,
        codes::INCOMPATIBLE_COMMAND,
        codes::ORDER_BEFORE,
        codes::ORDER_AFTER,
        codes::FIELD_DATA_WITHOUT_ORIGIN,
        codes::EMPTY_LABEL,
        codes::FIELD_NOT_CLOSED,
        codes::ORPHANED_FIELD_SEPARATOR,
        codes::HOST_COMMAND_IN_LABEL,
        codes::DUPLICATE_FIELD_NUMBER,
        codes::POSITION_OUT_OF_BOUNDS,
        codes::UNKNOWN_FONT,
        codes::INVALID_HEX_ESCAPE,
        codes::REDUNDANT_STATE,
        codes::SERIALIZATION_WITHOUT_FIELD_NUMBER,
        codes::GF_DATA_LENGTH_MISMATCH,
        codes::GF_BOUNDS_OVERFLOW,
        codes::GF_MEMORY_EXCEEDED,
        codes::MISSING_EXPLICIT_DIMENSIONS,
        codes::OBJECT_BOUNDS_OVERFLOW,
        codes::BARCODE_INVALID_CHAR,
        codes::BARCODE_DATA_LENGTH,
        codes::NOTE,
    ];

    let parser_codes = [
        codes::PARSER_NO_LABELS,
        codes::PARSER_INVALID_COMMAND,
        codes::PARSER_UNKNOWN_COMMAND,
        codes::PARSER_MISSING_TERMINATOR,
        codes::PARSER_MISSING_FIELD_SEPARATOR,
        codes::PARSER_FIELD_DATA_INTERRUPTED,
        codes::PARSER_STRAY_CONTENT,
        codes::PARSER_NON_ASCII_ARG,
    ];

    let mut missing = Vec::new();
    for code in validator_codes.iter().chain(parser_codes.iter()) {
        if explain(code).is_none() {
            missing.push(*code);
        }
    }

    assert!(
        missing.is_empty(),
        "Diagnostic codes without explain() entries: {:?}",
        missing
    );
}

// NOTE: Some validator code paths are not currently exercised by real spec data
// (e.g., arg-level minLength/maxLength and certain constraint expressions). The
// synthetic tests above intentionally mutate parser tables in-memory so those
// diagnostic paths remain covered against regressions.
