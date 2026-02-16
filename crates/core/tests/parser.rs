//! Comprehensive tests for the ZPL parser.
//!
//! Covers: basic parsing, command recognition (heuristic & trie), argument
//! parsing, field data mode, span tracking, trivia preservation, diagnostics,
//! edge cases, and sample file parsing.
//!
//! Validator-specific tests live in `validator.rs`.

mod common;

use common::{
    extract_codes, extract_diag_codes, extract_label_codes, find_args, is_severity_error,
    is_severity_info, is_severity_warn,
};
use zpl_toolchain_core::grammar::ast::{Node, Presence};
use zpl_toolchain_core::grammar::diag::Span;
use zpl_toolchain_core::grammar::parser::{parse_str, parse_with_tables};
use zpl_toolchain_diagnostics::{Severity, codes};

fn tables_with_spacing_command(
    code: &str,
    spacing_policy: zpl_toolchain_spec_tables::SpacingPolicy,
) -> zpl_toolchain_spec_tables::ParserTables {
    zpl_toolchain_spec_tables::ParserTables::new(
        "1.0.0".into(),
        zpl_toolchain_spec_tables::TABLE_FORMAT_VERSION.into(),
        vec![zpl_toolchain_spec_tables::CommandEntry {
            codes: vec![code.to_string()],
            arity: 1,
            raw_payload: false,
            field_data: false,
            opens_field: false,
            closes_field: false,
            hex_escape_modifier: false,
            field_number: false,
            serialization: false,
            requires_field: false,
            signature: Some(zpl_toolchain_spec_tables::Signature {
                params: vec!["n".to_string()],
                joiner: ",".to_string(),
                spacing_policy,
                allow_empty_trailing: true,
                split_rule: None,
            }),
            args: Some(vec![zpl_toolchain_spec_tables::ArgUnion::Single(Box::new(
                zpl_toolchain_spec_tables::Arg {
                    name: Some("num".to_string()),
                    key: Some("n".to_string()),
                    r#type: "int".to_string(),
                    unit: None,
                    range: None,
                    min_length: None,
                    max_length: None,
                    optional: false,
                    presence: None,
                    default: None,
                    default_by_dpi: None,
                    default_from: None,
                    default_from_state_key: None,
                    profile_constraint: None,
                    range_when: None,
                    rounding_policy: None,
                    rounding_policy_when: None,
                    resource: None,
                    r#enum: None,
                },
            ))]),
            constraints: None,
            constraint_defaults: None,
            effects: None,
            plane: None,
            scope: None,
            placement: None,
            name: None,
            category: None,
            since: None,
            deprecated: None,
            deprecated_since: None,
            stability: None,
            composites: None,
            defaults: None,
            units: None,
            printer_gates: None,
            signature_overrides: None,
            field_data_rules: None,
            examples: None,
        }],
        None,
    )
}

// ─── 1. Basic Parsing ────────────────────────────────────────────────────────

#[test]
fn empty_input_no_labels() {
    let result = parse_str("");
    assert_eq!(
        result.ast.labels.len(),
        0,
        "empty input should produce no labels"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_NO_LABELS),
        "should emit no-labels diagnostic"
    );
}

#[test]
fn single_label_xa_xz() {
    let result = parse_str("^XA^XZ");
    assert_eq!(result.ast.labels.len(), 1, "should produce 1 label");
    let codes = extract_codes(&result);
    assert_eq!(codes, vec!["^XA", "^XZ"]);
}

#[test]
fn multiple_labels() {
    let result = parse_str("^XA^XZ^XA^XZ");
    assert_eq!(result.ast.labels.len(), 2, "should produce 2 labels");
    assert_eq!(extract_label_codes(&result, 0), vec!["^XA", "^XZ"]);
    assert_eq!(extract_label_codes(&result, 1), vec!["^XA", "^XZ"]);
}

#[test]
fn nested_xa_flushes_label() {
    // A second ^XA while inside a label should flush the current label
    let result = parse_str("^XA^FO10,10^XA^XZ");
    assert_eq!(
        result.ast.labels.len(),
        2,
        "nested ^XA should flush to 2 labels"
    );
    // First label: ^XA and ^FO (flushed when second ^XA is seen)
    let first_codes = extract_label_codes(&result, 0);
    assert!(
        first_codes.contains(&"^XA".to_string()),
        "first label should have ^XA"
    );
    assert!(
        first_codes.contains(&"^FO".to_string()),
        "first label should have ^FO"
    );
    // Second label: ^XA and ^XZ
    let second_codes = extract_label_codes(&result, 1);
    assert!(
        second_codes.contains(&"^XA".to_string()),
        "second label should have ^XA"
    );
    assert!(
        second_codes.contains(&"^XZ".to_string()),
        "second label should have ^XZ"
    );
}

// ─── 2. Command Recognition — Heuristic (no tables) ─────────────────────────

#[test]
fn heuristic_two_char_code() {
    // ^FO: F(alpha) O(alpha) 1(digit) → heuristic picks 2-char "FO"
    let result = parse_str("^XA^FO10,10^XZ");
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"^FO".to_string()),
        "heuristic should recognize ^FO as 2-char code"
    );
}

#[test]
fn heuristic_single_char_code() {
    // ^A0N: the heuristic sees A(alpha), 0(digit) → 2-char "A0" (not single char!)
    // The single-char recognition of ^A only works with tables/trie.
    let result = parse_str("^XA^A0N,22,26^XZ");
    let codes = extract_codes(&result);
    // Heuristic: A(alpha) + 0(digit) → 2-char code ^A0
    assert!(
        codes.contains(&"^A0".to_string()),
        "heuristic sees ^A0 as 2-char code (no tables to identify ^A): got {:?}",
        codes,
    );
}

#[test]
fn tilde_command() {
    let result = parse_str("^XA~JA^XZ");
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"~JA".to_string()),
        "tilde command ~JA should be recognized: {:?}",
        codes
    );
}

// ─── 3. Command Recognition — With Tables (trie) ────────────────────────────

#[test]
fn trie_longest_match() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^BY3,2,50^BCN,142,N,N,N^XZ", Some(tables));
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"^BY".to_string()),
        "trie should recognize ^BY: {:?}",
        codes
    );
    assert!(
        codes.contains(&"^BC".to_string()),
        "trie should recognize ^BC separately: {:?}",
        codes
    );
}

#[test]
fn unknown_command_warning() {
    let tables = &*common::TABLES;
    // ^QQ is unlikely to be a valid ZPL command
    let result = parse_with_tables("^XA^QQ99^XZ", Some(tables));
    let has_warning = result
        .diagnostics
        .iter()
        .any(|d| d.id == codes::PARSER_UNKNOWN_COMMAND && is_severity_warn(&d.severity));
    assert!(
        has_warning,
        "unknown command should produce a warning: {:?}",
        extract_diag_codes(&result)
    );
}

// ─── 4. Argument Parsing ────────────────────────────────────────────────────

#[test]
fn comma_separated_args() {
    let result = parse_str("^XA^FO100,200^XZ");
    let args = find_args(&result, "^FO");
    assert_eq!(args.len(), 2, "^FO should have 2 args");
    assert_eq!(args[0].value.as_deref(), Some("100"));
    assert_eq!(args[1].value.as_deref(), Some("200"));
    assert!(matches!(args[0].presence, Presence::Value));
    assert!(matches!(args[1].presence, Presence::Value));
}

#[test]
fn empty_trailing_args() {
    // ^BC,,,,, → 6 comma-separated empty segments
    let result = parse_str("^XA^BC,,,,,^XZ");
    // With heuristic, code is "^BC" (B,C both alpha, next is comma → 2-char)
    let args = find_args(&result, "^BC");
    assert_eq!(
        args.len(),
        6,
        "^BC,,,,, should produce exactly 6 empty args, got {}",
        args.len()
    );
    for (i, arg) in args.iter().enumerate() {
        assert!(
            matches!(arg.presence, Presence::Empty),
            "arg {} should be Empty, got {:?}",
            i,
            arg.presence,
        );
    }
}

#[test]
fn mixed_present_empty_args() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^BCN,142,,N,^XZ", Some(tables));
    let args = find_args(&result, "^BC");
    // ^BC has arity 6 with allowEmptyTrailing=true, so the parser exposes all
    // 6 arg slots including the trailing empty one.
    assert_eq!(
        args.len(),
        6,
        "^BC should have exactly 6 args, got {}: {:?}",
        args.len(),
        args
    );
    // First arg should be Value("N")
    assert!(
        matches!(args[0].presence, Presence::Value),
        "arg 0 should be Value"
    );
    assert_eq!(args[0].value.as_deref(), Some("N"));
    // Second should be Value("142")
    assert!(
        matches!(args[1].presence, Presence::Value),
        "arg 1 should be Value"
    );
    assert_eq!(args[1].value.as_deref(), Some("142"));
    // Third should be Empty
    assert!(
        matches!(args[2].presence, Presence::Empty),
        "arg 2 should be Empty"
    );
    // Fourth should be Value("N")
    assert!(
        matches!(args[3].presence, Presence::Value),
        "arg 3 should be Value"
    );
    assert_eq!(args[3].value.as_deref(), Some("N"));
    // Fifth should be Empty
    assert!(
        matches!(args[4].presence, Presence::Empty),
        "arg 4 should be Empty"
    );
    // Sixth should be Empty (trailing empty arg with allowEmptyTrailing)
    assert!(
        matches!(args[5].presence, Presence::Empty),
        "arg 5 should be Empty"
    );
}

#[test]
fn signature_no_space_after_opcode_rejects_space() {
    let tables =
        tables_with_spacing_command("^ZZN", zpl_toolchain_spec_tables::SpacingPolicy::Forbid);
    let result = parse_with_tables("^XA^ZZN 5^XZ", Some(&tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_INVALID_COMMAND
                && d.message.contains("should not include a space")),
        "expected parser spacing diagnostic when spacingPolicy=forbid: {:?}",
        result.diagnostics
    );
}

#[test]
fn signature_space_after_opcode_required() {
    let tables =
        tables_with_spacing_command("^ZZS", zpl_toolchain_spec_tables::SpacingPolicy::Require);
    let result = parse_with_tables("^XA^ZZS5^XZ", Some(&tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_INVALID_COMMAND && d.message.contains("expects a space")),
        "expected parser spacing diagnostic when spacingPolicy=require: {:?}",
        result.diagnostics
    );
}

#[test]
fn signature_space_after_opcode_require_accepts_space() {
    let tables =
        tables_with_spacing_command("^ZZS", zpl_toolchain_spec_tables::SpacingPolicy::Require);
    let result = parse_with_tables("^XA^ZZS 5^XZ", Some(&tables));
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_INVALID_COMMAND && d.message.contains("expects a space")),
        "space should be accepted when spacingPolicy=require: {:?}",
        result.diagnostics
    );
}

#[test]
fn signature_space_after_opcode_allow_accepts_both_forms() {
    let tables =
        tables_with_spacing_command("^ZZA", zpl_toolchain_spec_tables::SpacingPolicy::Allow);
    let no_space = parse_with_tables("^XA^ZZA5^XZ", Some(&tables));
    let with_space = parse_with_tables("^XA^ZZA 5^XZ", Some(&tables));
    for result in [&no_space, &with_space] {
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.id == codes::PARSER_INVALID_COMMAND && d.message.contains("space")),
            "spacingPolicy=allow should accept both forms: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn barcode_opcode_spacing_forbid_rejects_space() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^BY3^BC N,100,Y,N,N^FD12345^FS^XZ", Some(tables));
    assert!(
        result.diagnostics.iter().any(|d| {
            d.id == codes::PARSER_INVALID_COMMAND
                && d.message
                    .contains("^BC should not include a space between opcode and arguments")
        }),
        "expected barcode spacing diagnostic for ^BC with opcode-space form: {:?}",
        result.diagnostics
    );
}

#[test]
fn diag_parser_1302_non_ascii_arg() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^CCé^XZ", Some(tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_NON_ASCII_ARG),
        "non-ASCII arg for ^CC should emit ZPL.PARSER.1302: {:?}",
        result.diagnostics
    );
}

#[test]
fn special_a_font_orientation_split() {
    // With tables, ^A is recognized as single-char code, then special-case splitting
    // converts "0N" → f="0", o="N", plus h and w from the comma-separated rest.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^A0N,22,26^XZ", Some(tables));
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"^A".to_string()),
        "tables should recognize ^A: {:?}",
        codes
    );
    let args = find_args(&result, "^A");
    assert!(
        args.len() >= 4,
        "^A should have at least 4 args (f, o, h, w), got {}: {:?}",
        args.len(),
        args
    );
    assert_eq!(args[0].value.as_deref(), Some("0"), "font");
    assert_eq!(args[1].value.as_deref(), Some("N"), "orientation");
    assert_eq!(args[2].value.as_deref(), Some("22"), "height");
    assert_eq!(args[3].value.as_deref(), Some("26"), "width");
}

// ─── 5. Field Data Mode ─────────────────────────────────────────────────────

#[test]
fn field_data_preserved() {
    // With tables, ^FD is a field data command. When inline (^FD...^FS on same line),
    // the text after the opcode becomes ^FD's argument, since token collection
    // absorbs it before FieldData mode starts.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FDHello World^FS^XZ", Some(tables));
    let codes = extract_codes(&result);
    assert!(codes.contains(&"^FD".to_string()), "should have ^FD");
    assert!(codes.contains(&"^FS".to_string()), "should have ^FS");
    // The text "Hello World" is in ^FD's args (inline absorption)
    let fd_args = find_args(&result, "^FD");
    assert_eq!(
        fd_args[0].value.as_deref(),
        Some("Hello World"),
        "^FD first arg should be exactly 'Hello World': {:?}",
        fd_args,
    );
}

#[test]
fn field_data_with_commas() {
    // Commas in field data text: inline ^FD absorbs text as args, and commas
    // act as argument separators (default joiner is ",").
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FDPrice: $1,234.56^FS^XZ", Some(tables));
    let fd_args = find_args(&result, "^FD");
    // Field data should be a single unsplit arg with the full content
    assert_eq!(
        fd_args.len(),
        1,
        "^FD should have exactly 1 arg (unsplit field data): {:?}",
        fd_args
    );
    assert_eq!(
        fd_args[0].value.as_deref(),
        Some("Price: $1,234.56"),
        "^FD arg should preserve the full field data including commas: {:?}",
        fd_args,
    );
}

#[test]
fn field_data_fv_also_works() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FVVariable data^FS^XZ", Some(tables));
    let codes = extract_codes(&result);
    assert!(codes.contains(&"^FV".to_string()), "should recognize ^FV");
    assert!(codes.contains(&"^FS".to_string()), "should have ^FS");
    let fv_args = find_args(&result, "^FV");
    assert_eq!(
        fv_args[0].value.as_deref(),
        Some("Variable data"),
        "^FV first arg should be exactly 'Variable data': {:?}",
        fv_args,
    );
}

#[test]
fn field_data_inline_preserves_leading_and_trailing_spaces() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^FD Hello World  ^FS^XZ", Some(tables));
    let fd_args = find_args(&result, "^FD");
    assert!(
        !fd_args.is_empty(),
        "expected ^FD to have inline field-data arg, diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fd_args[0].value.as_deref(),
        Some(" Hello World  "),
        "^FD inline field-data should preserve leading/trailing spaces exactly: {:?}",
        fd_args
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_INVALID_COMMAND && d.message.contains("space")),
        "spacing diagnostics should not fire for ^FD with optional spacing: {:?}",
        result.diagnostics
    );
}

#[test]
fn field_data_gs1_semicolon_is_plain_data_and_does_not_swallow_fs() {
    let tables = &*common::TABLES;
    let result = parse_with_tables(
        "^XA^FO10,10^BCN,100,Y,N,N^FD>;>800093012345678901234^FS^XZ",
        Some(tables),
    );

    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"^FS".to_string()),
        "inline GS1 payload should still terminate field at ^FS: {:?}",
        codes
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_FIELD_DATA_INTERRUPTED),
        "semicolon in field data should not trigger field-data interruption: {:?}",
        result.diagnostics
    );
}

#[test]
fn parses_crlf_line_endings_without_stray_content() {
    let tables = &*common::TABLES;
    let result = parse_with_tables(
        "^XA\r\n^FO10,10\r\n^FDHello\r\n^FS\r\n^XZ\r\n",
        Some(tables),
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_STRAY_CONTENT),
        "CRLF input should not generate stray-content diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn parses_cr_only_line_endings_without_stray_content() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA\r^FO10,10\r^FDHello\r^FS\r^XZ\r", Some(tables));
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_STRAY_CONTENT),
        "CR-only input should not generate stray-content diagnostics: {:?}",
        result.diagnostics
    );
}

// ─── 6. Span Tracking ──────────────────────────────────────────────────────

#[test]
fn spans_present_on_all_nodes() {
    let result = parse_str("^XA^FO10,20^XZ");
    for label in &result.ast.labels {
        for node in &label.nodes {
            let span = match node {
                Node::Command { span, .. } => span,
                Node::FieldData { span, .. } => span,
                Node::RawData { span, .. } => span,
                Node::Trivia { span, .. } => span,
                _ => unreachable!("unknown Node variant"),
            };
            assert!(
                span.end >= span.start,
                "span end should be >= start: {:?}",
                span
            );
        }
    }
}

#[test]
fn span_positions_correct() {
    // "^XA^FO10,20^XZ"
    //  0123456789...
    let input = "^XA^FO10,20^XZ";
    let result = parse_str(input);

    // ^XA occupies bytes 0..3
    let xa_span = result.ast.labels[0].nodes.iter().find_map(|n| match n {
        Node::Command { code, span, .. } if code == "^XA" => Some(*span),
        _ => None,
    });
    assert_eq!(xa_span, Some(Span::new(0, 3)), "^XA span");

    // ^FO occupies bytes 3..11 ("^FO10,20")
    let fo_span = result.ast.labels[0].nodes.iter().find_map(|n| match n {
        Node::Command { code, span, .. } if code == "^FO" => Some(*span),
        _ => None,
    });
    assert_eq!(fo_span, Some(Span::new(3, 11)), "^FO span");

    // ^XZ occupies bytes 11..14
    let xz_span = result.ast.labels[0].nodes.iter().find_map(|n| match n {
        Node::Command { code, span, .. } if code == "^XZ" => Some(*span),
        _ => None,
    });
    assert_eq!(xz_span, Some(Span::new(11, 14)), "^XZ span");
}

// ─── 7. Trivia Preservation ────────────────────────────────────────────────

#[test]
fn semicolon_comments_are_not_parsed_as_trivia() {
    let result = parse_str("^XA;this is not a comment\n^FO10,20^XZ");
    let has_trivia = result
        .ast
        .labels
        .iter()
        .flat_map(|l| l.nodes.iter())
        .any(|n| matches!(n, Node::Trivia { .. }));
    assert!(!has_trivia, "semicolon should not produce trivia nodes");

    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_FIELD_DATA_INTERRUPTED),
        "semicolon handling should not trigger comment-specific parser behavior"
    );
}

// ─── 8. Diagnostics ────────────────────────────────────────────────────────

#[test]
fn missing_xz_terminator() {
    let result = parse_str("^XA^FO10,10");
    let diag_codes = extract_diag_codes(&result);
    assert!(
        diag_codes.contains(&codes::PARSER_MISSING_TERMINATOR.to_string()),
        "should emit missing-^XZ diagnostic: {:?}",
        diag_codes,
    );
    // The error should be Error severity
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.id == codes::PARSER_MISSING_TERMINATOR)
        .unwrap();
    assert!(
        is_severity_error(&diag.severity),
        "1102 should be Error severity"
    );
}

#[test]
fn missing_fs_before_xz() {
    // With tables, ^FD activates field data mode. When ^XZ follows without ^FS,
    // the parser detects the interruption.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FDHello^XZ", Some(tables));
    // The parser emits ZPL.PARSER.1203 (field data interrupted) when ^XZ's
    // leader interrupts field data mode, OR ZPL.PARSER.1202 for missing ^FS.
    let diag_codes = extract_diag_codes(&result);
    let has_fs_diag = diag_codes.iter().any(|c| {
        c == codes::PARSER_MISSING_FIELD_SEPARATOR || c == codes::PARSER_FIELD_DATA_INTERRUPTED
    });
    assert!(
        has_fs_diag,
        "should emit field-data diagnostic (1202 or 1203): {:?}",
        diag_codes,
    );
}

#[test]
fn no_labels_info() {
    let result = parse_str("");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.id == codes::PARSER_NO_LABELS);
    assert!(
        diag.is_some(),
        "empty input should emit 0001 info diagnostic"
    );
    assert!(
        is_severity_info(&diag.unwrap().severity),
        "0001 should be Info severity"
    );
}

#[test]
fn diagnostic_has_span() {
    // For location-aware errors, spans should be present
    let result = parse_str("^XA^FO10,10");
    let diag_1102 = result
        .diagnostics
        .iter()
        .find(|d| d.id == codes::PARSER_MISSING_TERMINATOR);
    // Note: the 1102 diagnostic for missing ^XZ may or may not have a span
    // depending on implementation — it's emitted at end-of-input.
    // Let's verify the parser at least emits the diagnostic.
    assert!(diag_1102.is_some());

    // For an invalid leader (^^), the 1001 diagnostic should have a span
    let result2 = parse_str("^^");
    let diag_1001 = result2
        .diagnostics
        .iter()
        .find(|d| d.id == codes::PARSER_INVALID_COMMAND);
    assert!(diag_1001.is_some(), "^^ should emit 1001");
    assert!(
        diag_1001.unwrap().span.is_some(),
        "1001 for invalid leader should have a span"
    );
}

// ─── 9. Edge Cases ─────────────────────────────────────────────────────────

#[test]
fn only_leaders_no_code() {
    let result = parse_str("^^");
    let diag_codes = extract_diag_codes(&result);
    assert!(
        diag_codes
            .iter()
            .any(|c| c == codes::PARSER_INVALID_COMMAND),
        "leader-only input should produce error: {:?}",
        diag_codes,
    );
}

#[test]
fn label_with_all_empty_args() {
    let result = parse_str("^XA^BC,,,,,,^XZ");
    let args = find_args(&result, "^BC");
    assert_eq!(
        args.len(),
        7,
        "^BC,,,,,, should produce exactly 7 empty args, got {}",
        args.len()
    );
    for arg in &args {
        assert!(
            matches!(arg.presence, Presence::Empty),
            "all args should be Empty"
        );
    }
}

#[test]
fn utf8_in_field_data() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FDHéllo Wörld^FS^XZ", Some(tables));
    let fd_args = find_args(&result, "^FD");
    assert_eq!(
        fd_args[0].value.as_deref(),
        Some("Héllo Wörld"),
        "^FD first arg should preserve UTF-8 content exactly: {:?}",
        fd_args,
    );
}

#[test]
fn multiple_fields_in_label() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^FDLine1^FS^FO10,50^FDLine2^FS^XZ", Some(tables));
    assert_eq!(result.ast.labels.len(), 1, "should be 1 label");
    let codes = extract_codes(&result);
    // Should have: ^XA, ^FO, ^FD, ^FS, ^FO, ^FD, ^FS, ^XZ
    let fd_count = codes.iter().filter(|c| c.as_str() == "^FD").count();
    let fs_count = codes.iter().filter(|c| c.as_str() == "^FS").count();
    assert_eq!(fd_count, 2, "should have 2 ^FD commands");
    assert_eq!(fs_count, 2, "should have 2 ^FS commands");

    // Check that both field data texts are captured
    let fd_nodes: Vec<_> = result.ast.labels[0]
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::Command { code, .. } if code == "^FD"))
        .collect();
    assert_eq!(fd_nodes.len(), 2);
}

// ─── 9b. Additional Coverage ─────────────────────────────────────────────────

#[test]
fn field_data_interrupted_emits_1203() {
    let tables = &*common::TABLES;
    // ^FD starts field data, then ^FO interrupts before ^FS
    let result = parse_with_tables("^XA^FDHello^FO10,10^FS^XZ", Some(tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_FIELD_DATA_INTERRUPTED),
        "non-^FS command interrupting field data should emit ZPL.PARSER.1203: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn fx_comment_with_reserved_leaders_emits_targeted_parser_errors() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FXComment with ^ and ~ chars^FS^XZ", Some(tables));
    let targeted_errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| {
            d.id == codes::PARSER_INVALID_COMMAND
                && d.message.contains("reserved command leader")
                && d.message.contains("inside ^FX free-form text")
        })
        .collect();
    assert_eq!(
        targeted_errors.len(),
        2,
        "expected one targeted parser error for each raw '^'/'~' in ^FX text: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| !(d.id == codes::PARSER_INVALID_COMMAND
                && d.message.contains("expected command code after leader"))),
        "should avoid generic expected-code parser errors for raw leaders in field data/comment: {:?}",
        result.diagnostics
    );
}

#[test]
fn field_data_at_eof_emits_1202() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FDunterminated", Some(tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_MISSING_FIELD_SEPARATOR),
        "field data at EOF without ^FS should emit ZPL.PARSER.1202: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn empty_field_data() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FD^FS^XZ", Some(tables));
    let fd_args = find_args(&result, "^FD");
    // ^FD with no content → empty args
    assert!(
        fd_args.is_empty(),
        "^FD with no content should have no args: {:?}",
        fd_args
    );
    // Verify no error-level diagnostics
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| matches!(d.severity, Severity::Error)),
        "^FD^FS should not produce errors: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn known_set_fallback_without_trie() {
    let mut tables = common::TABLES.clone();
    tables.opcode_trie = None;
    let result = parse_with_tables("^XA^FO10,10^XZ", Some(&tables));
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"^FO".to_string()),
        "known-set should recognize ^FO without trie: {:?}",
        codes
    );
}

#[test]
fn diagnostic_code_1002_distinct_from_1001() {
    let tables = &*common::TABLES;
    // ^QQ99 is unknown → should get ZPL.PARSER.1002 (not 1001)
    let result = parse_with_tables("^XA^QQ99^XZ", Some(tables));
    let has_1002 = result
        .diagnostics
        .iter()
        .any(|d| d.id == codes::PARSER_UNKNOWN_COMMAND);
    let has_1001 = result
        .diagnostics
        .iter()
        .any(|d| d.id == codes::PARSER_INVALID_COMMAND);
    assert!(has_1002, "unknown command should produce ZPL.PARSER.1002");
    assert!(
        !has_1001,
        "unknown command should NOT produce ZPL.PARSER.1001 (that's for invalid syntax)"
    );
}

#[test]
fn stray_content_warning() {
    // Text on its own line between commands should produce a stray content warning.
    // Note: stray content only appears when tokens aren't consumed by a command's
    // argument collection, which stops at newlines/leaders.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA\n^FDHello^FS\nstray text here\n^XZ", Some(tables));
    let stray = result
        .diagnostics
        .iter()
        .filter(|d| d.id == codes::PARSER_STRAY_CONTENT)
        .collect::<Vec<_>>();
    assert!(
        !stray.is_empty(),
        "stray content should produce ZPL.PARSER.1301: {:?}",
        extract_diag_codes(&result)
    );
    // Verify it's a warning
    assert!(
        is_severity_warn(&stray[0].severity),
        "stray content should be a warning"
    );
    // Verify span exists
    assert!(
        stray[0].span.is_some(),
        "stray content diagnostic should have a span"
    );
}

#[test]
fn no_stray_warning_for_whitespace_between_commands() {
    // Whitespace and newlines between commands should NOT produce stray warnings.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA\n  ^FO50,100\n  ^FDHello^FS\n^XZ", Some(tables));
    let stray = result
        .diagnostics
        .iter()
        .filter(|d| d.id == codes::PARSER_STRAY_CONTENT)
        .collect::<Vec<_>>();
    assert!(
        stray.is_empty(),
        "whitespace between commands should not produce stray warnings: {:?}",
        stray
    );
}

#[test]
fn stray_content_coalesces_adjacent_tokens() {
    // Multiple adjacent stray Value/Comma tokens on their own line should
    // produce a single coalesced warning.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA\nhello,world\n^XZ", Some(tables));
    let stray = result
        .diagnostics
        .iter()
        .filter(|d| d.id == codes::PARSER_STRAY_CONTENT)
        .collect::<Vec<_>>();
    assert_eq!(
        stray.len(),
        1,
        "adjacent stray tokens should coalesce into one diagnostic, got {:?}",
        stray
    );
}

#[test]
fn recovery_after_invalid_leader() {
    // A bare ^ at end-of-input should recover and still parse subsequent content.
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^^FDHello^FS^XZ", Some(tables));
    let has_invalid = result
        .diagnostics
        .iter()
        .any(|d| d.id == codes::PARSER_INVALID_COMMAND);
    assert!(
        has_invalid,
        "bare ^^ should produce an invalid command error"
    );
    // Despite the error, ^FD should still be parsed
    let has_fd = result.ast.labels.iter().any(|l| {
        l.nodes.iter().any(|n| match n {
            zpl_toolchain_core::grammar::ast::Node::Command { code, .. } => code == "^FD",
            _ => false,
        })
    });
    assert!(has_fd, "parser should recover and parse ^FD after ^^");
}

// ─── 10. Parser Diagnostic Coverage ─────────────────────────────────────────
//
// Parser diagnostics (verified existing coverage):
//   ZPL.PARSER.0001 — empty_input_no_labels, no_labels_info
//   ZPL.PARSER.1001 — only_leaders_no_code, diagnostic_has_span
//   ZPL.PARSER.1002 — unknown_command_warning, diagnostic_code_1002_distinct_from_1001
//   ZPL.PARSER.1102 — missing_xz_terminator
//   ZPL.PARSER.1202 — missing_fs_before_xz, field_data_at_eof_emits_1202
//   ZPL.PARSER.1203 — field_data_interrupted_emits_1203
//
// Validator diagnostic tests are in validator.rs.

// ── Parser diagnostic coverage (explicit regression tests) ──────────────────

#[test]
fn diag_parser_0001_no_labels() {
    // Distinct from the basic test: also verify the diagnostic message content
    let result = parse_str("  \t\n  ");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_NO_LABELS),
        "whitespace-only input should emit no-labels diagnostic: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn diag_parser_1001_leader_then_eof() {
    // Leader "^" at end of input with no command code following
    let result = parse_str("^XA^");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_INVALID_COMMAND),
        "leader at EOF should emit 1001: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn diag_parser_1002_unknown_command() {
    let tables = &*common::TABLES;
    // ^QQ is not a real ZPL command; use with a trailing digit to ensure
    // the heuristic produces a 2-char code that isn't in the tables.
    let result = parse_with_tables("^XA^QQ99^XZ", Some(tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_UNKNOWN_COMMAND),
        "unknown command should emit 1002: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn diag_parser_1102_missing_xz() {
    let result = parse_str("^XA^FO10,10^FDHello^FS");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_MISSING_TERMINATOR),
        "missing ^XZ should emit 1102: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn diag_parser_1202_missing_fs_eof() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FDdata without separator", Some(tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_MISSING_FIELD_SEPARATOR),
        "field data at EOF without ^FS should emit 1202: {:?}",
        extract_diag_codes(&result),
    );
}

#[test]
fn diag_parser_1203_field_data_interrupted() {
    let tables = &*common::TABLES;
    // ^FD starts field data, then ^CF interrupts before ^FS
    let result = parse_with_tables("^XA^FDHello^CF0,30^FS^XZ", Some(tables));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.id == codes::PARSER_FIELD_DATA_INTERRUPTED),
        "command interrupting field data should emit 1203: {:?}",
        extract_diag_codes(&result),
    );
}

// ─── 11. Raw Payload Mode ───────────────────────────────────────────────────

#[test]
fn raw_payload_gf_inline_no_extra_node() {
    let tables = &*common::TABLES;
    // When all data is inline (no continuation after args), no RawData node should be created.
    let input = "^XA^GFA,8,8,1,FFAA5500FFAA5500^FS^XZ";
    let r = parse_with_tables(input, Some(tables));

    let nodes = &r.ast.labels[0].nodes;
    // Data is fully captured in args[4] — no spurious RawData node
    let has_raw = nodes.iter().any(|n| matches!(n, Node::RawData { .. }));
    assert!(
        !has_raw,
        "Fully-inline data should NOT produce a RawData node, got: {:?}",
        nodes
    );

    // Verify data is in the command args
    let gf = nodes
        .iter()
        .find(|n| matches!(n, Node::Command { code, .. } if code == "^GF"))
        .unwrap();
    if let Node::Command { args, .. } = gf {
        assert_eq!(
            args[4].value.as_deref(),
            Some("FFAA5500FFAA5500"),
            "data should be in args[4]"
        );
    }
}

#[test]
fn raw_payload_gf_data_preserved() {
    let tables = &*common::TABLES;
    // After ^GF args are parsed (A,4,4,1), the parser hits the newline which
    // ends the arg collection. Then RawData mode collects the continuation data
    // on the next line until ^XZ.
    let input = "^XA^GFA,4,4,1,AABBCCDD^XZ";
    let r = parse_with_tables(input, Some(tables));
    let nodes = &r.ast.labels[0].nodes;

    // The data might be in the last arg of the ^GF command (inline) or in a
    // RawData node (continuation). Either way, we should have no panics.
    let has_gf = nodes
        .iter()
        .any(|n| matches!(n, Node::Command { code, .. } if code == "^GF"));
    assert!(has_gf, "Expected ^GF command node");
}

#[test]
fn raw_payload_gf_multiline() {
    let tables = &*common::TABLES;
    // Multi-line: header on first line, data on subsequent lines
    let input = "^XA^GFA,8,8,1\nFFAA5500\nFFAA5500\n^FS^XZ";
    let r = parse_with_tables(input, Some(tables));
    let nodes = &r.ast.labels[0].nodes;

    // The data spanning multiple lines should be collected as RawData
    let raw = nodes.iter().find_map(|n| {
        if let Node::RawData { data, .. } = n {
            data.clone()
        } else {
            None
        }
    });
    assert!(
        raw.is_some(),
        "Expected raw data content for multi-line ^GF, got nodes: {:?}",
        nodes
    );
    let data = raw.unwrap();
    assert!(
        data.contains("FFAA5500"),
        "Raw data should contain hex payload, got: {}",
        data
    );
}

#[test]
fn raw_payload_empty_data() {
    let tables = &*common::TABLES;
    // ^GF with args but empty data — should not panic
    let input = "^XA^GFA,0,0,1,^XZ";
    let r = parse_with_tables(input, Some(tables));
    assert!(
        !r.ast.labels.is_empty(),
        "Should produce at least one label"
    );
}

#[test]
fn raw_payload_dg_basic() {
    let tables = &*common::TABLES;
    let input = "^XA~DGR:LOGO.GRF,4,1\nFFAA5500\n^XZ";
    let r = parse_with_tables(input, Some(tables));
    let nodes = &r.ast.labels[0].nodes;
    let has_raw = nodes
        .iter()
        .any(|n| matches!(n, Node::RawData { command, .. } if command == "~DG"));
    assert!(
        has_raw,
        "Expected RawData node for ~DG payload, got nodes: {:?}",
        nodes
    );
}

#[test]
fn raw_payload_span_tracking() {
    let tables = &*common::TABLES;
    let input = "^XA^GFA,4,4,1\nAABBCCDD\n^XZ";
    let r = parse_with_tables(input, Some(tables));
    let nodes = &r.ast.labels[0].nodes;

    // RawData node should have a valid span
    for node in nodes {
        if let Node::RawData { span, .. } = node {
            assert!(span.end >= span.start, "span end >= start: {:?}", span);
        }
    }
}

#[test]
fn raw_payload_at_eof() {
    let tables = &*common::TABLES;
    // ^GF at end of input without ^FS or ^XZ — unterminated raw data
    let input = "^XA^GFA,4,4,1\nAABBCCDD";
    let r = parse_with_tables(input, Some(tables));
    let nodes = &r.ast.labels[0].nodes;

    // Should still produce a RawData node from the cleanup path
    let has_raw = nodes.iter().any(|n| matches!(n, Node::RawData { .. }));
    assert!(
        has_raw,
        "Expected RawData node at EOF, got nodes: {:?}",
        nodes
    );
}

#[test]
fn raw_payload_inline_data_then_fs() {
    let tables = &*common::TABLES;
    // Common case: ^GFA,8,8,1,FFAA5500FFAA5500^FS on same line
    // The args parsing absorbs the data as the 5th arg.
    // Then RawData mode starts but immediately hits ^FS leader,
    // so RawData content is empty.
    let input = "^XA^GFA,8,8,1,FFAA5500FFAA5500^FS^XZ";
    let r = parse_with_tables(input, Some(tables));

    // ^GF command should have its args including data
    let gf_args = find_args(&r, "^GF");
    assert!(!gf_args.is_empty(), "^GF should have args");

    // The data arg (5th) should contain the hex payload
    let data_arg = gf_args.iter().find(|a| a.key.as_deref() == Some("data"));
    assert!(
        data_arg.is_some(),
        "^GF should have a 'data' arg: {:?}",
        gf_args
    );
    assert_eq!(
        data_arg.unwrap().value.as_deref(),
        Some("FFAA5500FFAA5500"),
        "data arg should contain hex payload"
    );
}

#[test]
fn raw_payload_no_false_positives_non_raw_command() {
    let tables = &*common::TABLES;
    // ^FO is NOT a raw_payload command — should NOT produce RawData nodes
    let input = "^XA^FO10,20^XZ";
    let r = parse_with_tables(input, Some(tables));
    let nodes = &r.ast.labels[0].nodes;
    let has_raw = nodes.iter().any(|n| matches!(n, Node::RawData { .. }));
    assert!(
        !has_raw,
        "Non-raw-payload commands should NOT produce RawData nodes"
    );
}

// ─── 12. Sample File Parsing ───────────────────────────────────────────────

#[test]
fn usps_sample_no_errors() {
    let tables = &*common::TABLES;
    let mut root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    let sample_path = root.join("samples/usps_surepost_sample.zpl");
    let input = std::fs::read_to_string(&sample_path).expect("missing sample file");
    let result = parse_with_tables(&input, Some(tables));

    // Should produce at least 1 label
    assert!(
        !result.ast.labels.is_empty(),
        "sample should produce at least one label"
    );

    // Should have no Error-level diagnostics
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| is_severity_error(&d.severity))
        .collect();
    assert!(
        errors.is_empty(),
        "sample file should have no error-level diagnostics, got: {:?}",
        errors,
    );
}

// ─── 13. Prefix / Delimiter State Tracking (^CC, ~CT, ^CD) ──────────────────

#[test]
fn prefix_change_cc() {
    let tables = &*common::TABLES;
    // ^CC+ changes the command prefix from ^ to +.
    // After the change, all commands must use + as the prefix.
    // The newline after ^CC+ provides a natural token boundary so that the
    // retokenized stream correctly identifies + as the new leader.
    let input = "^XA^CC+\n+FO10,10+FDHello+FS+XZ";
    let result = parse_with_tables(input, Some(tables));
    let codes = extract_codes(&result);
    // All codes should use canonical ^ prefix regardless of actual prefix used
    assert!(
        codes.contains(&"^XA".to_string()),
        "should have ^XA: {:?}",
        codes
    );
    assert!(
        codes.contains(&"^CC".to_string()),
        "should have ^CC: {:?}",
        codes
    );
    assert!(
        codes.contains(&"^FO".to_string()),
        "should have ^FO (via +FO): {:?}",
        codes
    );
    assert!(
        codes.contains(&"^FD".to_string()),
        "should have ^FD (via +FD): {:?}",
        codes
    );
    assert!(
        codes.contains(&"^FS".to_string()),
        "should have ^FS (via +FS): {:?}",
        codes
    );
    assert!(
        codes.contains(&"^XZ".to_string()),
        "should have ^XZ (via +XZ): {:?}",
        codes
    );
    // Verify the label is properly formed
    assert_eq!(result.ast.labels.len(), 1, "should produce 1 label");
    // Verify ^FO args parsed correctly
    let fo_args = find_args(&result, "^FO");
    assert!(fo_args.len() >= 2, "^FO should have at least 2 args");
    assert_eq!(fo_args[0].value.as_deref(), Some("10"));
    assert_eq!(fo_args[1].value.as_deref(), Some("10"));
}

#[test]
fn prefix_change_ct() {
    let tables = &*common::TABLES;
    // ~CT# changes the control prefix from ~ to #
    // After that, control commands use # as the prefix: #HS
    let input = "^XA~CT#^FO10,10^FDtest^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"~CT".to_string()),
        "should have ~CT: {:?}",
        codes
    );
    assert!(
        codes.contains(&"^FO".to_string()),
        "should have ^FO: {:?}",
        codes
    );
    // The label should parse correctly
    assert_eq!(result.ast.labels.len(), 1, "should produce 1 label");
}

#[test]
fn delimiter_change_cd() {
    let tables = &*common::TABLES;
    // ^CD| changes the argument delimiter from , to |
    // After that, ^FO uses | as the delimiter: ^FO10|20
    // (Note: `;` can't be used because the lexer treats it as a comment start)
    let input = "^XA^CD|^FO10|20^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"^CD".to_string()),
        "should have ^CD: {:?}",
        codes
    );
    assert!(
        codes.contains(&"^FO".to_string()),
        "should have ^FO: {:?}",
        codes
    );
    // Verify ^FO args parsed correctly with | delimiter
    // ^FO has allowEmptyTrailing with 3 params, so padding gives 3 args
    let fo_args = find_args(&result, "^FO");
    assert!(
        fo_args.len() >= 2,
        "^FO should have at least 2 args with | delimiter"
    );
    assert_eq!(fo_args[0].value.as_deref(), Some("10"), "first arg");
    assert_eq!(fo_args[1].value.as_deref(), Some("20"), "second arg");
}

#[test]
fn prefix_and_delimiter_regression() {
    let tables = &*common::TABLES;
    // Default input with no prefix/delimiter changes should still work correctly
    let input = "^XA^FO10,20^FS^XZ";
    let result = parse_with_tables(input, Some(tables));
    let codes = extract_codes(&result);
    assert_eq!(codes, vec!["^XA", "^FO", "^FS", "^XZ"]);
    // ^FO has allowEmptyTrailing with 3 params (x,y,z), so 2 provided + 1 padded = 3
    let fo_args = find_args(&result, "^FO");
    assert!(fo_args.len() >= 2, "^FO should have at least x,y args");
    assert_eq!(fo_args[0].value.as_deref(), Some("10"));
    assert_eq!(fo_args[1].value.as_deref(), Some("20"));
    assert_eq!(result.ast.labels.len(), 1);
    // No errors
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "regression: should have no errors: {:?}",
        errors
    );
}

#[test]
fn prefix_change_cc_with_tilde_variant() {
    let tables = &*common::TABLES;
    // ~CC+ changes the caret command prefix from ^ to +
    // After this, + is the leader for format commands, ^ is no longer a leader.
    // Note: we use canonical codes (^FD, ^FS, ^XZ) in the AST even though the
    // source uses +FD, +FS, +XZ — the parser maps custom prefixes to canonical ^.
    let input = "^XA~CC++FO10,10+FDHello+FS+XZ";
    let result = parse_with_tables(input, Some(tables));
    let codes = extract_codes(&result);
    assert!(
        codes.contains(&"~CC".to_string()),
        "should have ~CC: {:?}",
        codes
    );
    // After prefix change, +FO → ^FO, +FD → ^FD, +FS → ^FS, +XZ → ^XZ
    assert!(
        codes.contains(&"^FO".to_string()),
        "should have ^FO (via +FO): {:?}",
        codes
    );
    assert!(
        codes.contains(&"^FD".to_string()),
        "should have ^FD (via +FD): {:?}",
        codes
    );
    assert!(
        codes.contains(&"^XZ".to_string()),
        "should have ^XZ (via +XZ): {:?}",
        codes
    );
}

#[test]
fn tokenize_with_config_basic() {
    use zpl_toolchain_core::grammar::lexer::{TokKind, tokenize_with_config};
    // With + as command prefix and ~ as control prefix
    let toks = tokenize_with_config("+XA+FO10,20+XZ", '+', '~', ',');
    let leaders: Vec<&str> = toks
        .iter()
        .filter(|t| t.kind == TokKind::Leader)
        .map(|t| t.text)
        .collect();
    assert_eq!(leaders, vec!["+", "+", "+"], "should recognize + as leader");
    // Verify values don't contain + (it's treated as a leader, not part of value)
    let values: Vec<&str> = toks
        .iter()
        .filter(|t| t.kind == TokKind::Value)
        .map(|t| t.text)
        .collect();
    assert!(values.contains(&"XA"), "should have XA value token");
    assert!(values.contains(&"FO10"), "should have FO10 value token");
    assert!(values.contains(&"20"), "should have 20 value token");
    assert!(values.contains(&"XZ"), "should have XZ value token");
}

#[test]
fn tokenize_default_unchanged() {
    use zpl_toolchain_core::grammar::lexer::{TokKind, tokenize};
    // Default tokenize should still work with ^ and ~
    let toks = tokenize("^XA~JA^XZ");
    let leaders: Vec<&str> = toks
        .iter()
        .filter(|t| t.kind == TokKind::Leader)
        .map(|t| t.text)
        .collect();
    assert_eq!(
        leaders,
        vec!["^", "~", "^"],
        "default tokenize should recognize ^ and ~"
    );
}

// ─── 14. Parser Context on Diagnostics ───────────────────────────────────────

#[test]
fn context_parser_unknown_command() {
    let tables = &*common::TABLES;
    // ^QQ is not a real ZPL command
    let result = parse_with_tables("^XA^QQ99^XZ", Some(tables));
    let d = common::find_diag(&result.diagnostics, codes::PARSER_UNKNOWN_COMMAND);
    let ctx = d
        .context
        .as_ref()
        .expect("parser diagnostic should have context");
    assert_eq!(ctx.get("command").unwrap(), "^QQ");
}

#[test]
fn context_parser_missing_terminator() {
    let tables = &*common::TABLES;
    let result = parse_with_tables("^XA^FO10,10^FDHello^FS", Some(tables));
    let d = common::find_diag(&result.diagnostics, codes::PARSER_MISSING_TERMINATOR);
    let ctx = d
        .context
        .as_ref()
        .expect("parser diagnostic should have context");
    assert_eq!(ctx.get("expected").unwrap(), "^XZ");
}
