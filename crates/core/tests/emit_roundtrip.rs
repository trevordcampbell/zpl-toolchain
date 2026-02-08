//! Round-trip tests for the ZPL emitter.
//!
//! Gold-standard guarantee: `parse(emit(parse(input)))` produces the same AST
//! as `parse(input)` (ignoring spans, which differ after formatting).

mod common;

use zpl_toolchain_core::grammar::emit::{EmitConfig, Indent, emit_zpl, strip_spans};
use zpl_toolchain_core::grammar::parser::{parse_str, parse_with_tables};
use zpl_toolchain_spec_tables::ParserTables;

/// Assert that formatting + re-parsing produces a semantically identical AST.
fn assert_roundtrip(input: &str, tables: &ParserTables) {
    let res1 = parse_with_tables(input, Some(tables));
    let formatted = emit_zpl(&res1.ast, Some(tables), &EmitConfig::default());
    let res2 = parse_with_tables(&formatted, Some(tables));
    assert_eq!(
        strip_spans(&res1.ast),
        strip_spans(&res2.ast),
        "\n--- Round-trip failed ---\nInput:\n{}\nFormatted:\n{}\n",
        input,
        formatted
    );
}

/// Assert round-trip without tables (graceful degradation).
fn assert_roundtrip_no_tables(input: &str) {
    let res1 = parse_str(input);
    let formatted = emit_zpl(&res1.ast, None, &EmitConfig::default());
    let res2 = parse_str(&formatted);
    assert_eq!(
        strip_spans(&res1.ast),
        strip_spans(&res2.ast),
        "\n--- Round-trip (no tables) failed ---\nInput:\n{}\nFormatted:\n{}\n",
        input,
        formatted
    );
}

// ── Basic label round-trips ─────────────────────────────────────────────

#[test]
fn simple_label_roundtrip() {
    assert_roundtrip("^XA^FO50,100^A0N,30,30^FDHello^FS^XZ", &common::TABLES);
}

#[test]
fn empty_label_roundtrip() {
    assert_roundtrip("^XA^XZ", &common::TABLES);
}

#[test]
fn multiple_labels_roundtrip() {
    assert_roundtrip("^XA^FDLabel1^FS^XZ^XA^FDLabel2^FS^XZ", &common::TABLES);
}

// ── Split rule (^A command) ─────────────────────────────────────────────

#[test]
fn split_rule_a0_roundtrip() {
    assert_roundtrip("^XA^A0N,30,30^FDTest^FS^XZ", &common::TABLES);
}

#[test]
fn split_rule_a0_different_orientation() {
    assert_roundtrip("^XA^A0R,20,20^FDRotated^FS^XZ", &common::TABLES);
}

// ── Non-comma joiners ───────────────────────────────────────────────────

#[test]
fn dot_joiner_ll_roundtrip() {
    // ^LL uses "." as joiner: ^LL600.N
    assert_roundtrip("^XA^LL600^XZ", &common::TABLES);
}

// ── Field data preservation ─────────────────────────────────────────────

#[test]
fn field_data_with_commas_roundtrip() {
    assert_roundtrip("^XA^FO50,50^FDhello, world^FS^XZ", &common::TABLES);
}

#[test]
fn field_data_with_special_chars_roundtrip() {
    assert_roundtrip(
        "^XA^FO10,10^FDPrice: $5.00 (50% off)^FS^XZ",
        &common::TABLES,
    );
}

#[test]
fn field_value_fv_roundtrip() {
    assert_roundtrip(
        "^XA^FO10,10^A0N,30,30^FVDynamic Data^FS^XZ",
        &common::TABLES,
    );
}

// ── Comments ────────────────────────────────────────────────────────────

#[test]
fn semicolon_comment_roundtrip() {
    assert_roundtrip("^XA\n^PW812   ; set print width\n^XZ", &common::TABLES);
}

#[test]
fn fx_comment_preserved_in_output() {
    // Note: ^FX has an empty joiner, so the parser splits content
    // character-by-character and loses spaces (a parser-level limitation).
    // We test that ^FX content is emitted (not dropped), even if spaces
    // are not perfectly preserved.
    let tables = &common::TABLES;
    let input = "^XA^FXComment^FS^XZ";
    let res = parse_with_tables(input, Some(tables));
    let formatted = emit_zpl(&res.ast, Some(tables), &EmitConfig::default());
    assert!(
        formatted.contains("^FXComment"),
        "Expected ^FX comment to be preserved, got:\n{}",
        formatted
    );
}

// ── Trailing empty args ─────────────────────────────────────────────────

#[test]
fn trailing_empty_args_bc_roundtrip() {
    assert_roundtrip("^XA^BC,,100,,,Y^FD12345^FS^XZ", &common::TABLES);
}

#[test]
fn trailing_args_trimmed() {
    // ^FO has 3 params (x, y, z). When only 2 given, the trailing empty
    // should be trimmed.
    let tables = &common::TABLES;
    let input = "^XA^FO50,100^XZ";
    let res = parse_with_tables(input, Some(tables));
    let formatted = emit_zpl(&res.ast, Some(tables), &EmitConfig::default());
    assert!(
        formatted.contains("^FO50,100"),
        "Expected ^FO50,100 without trailing comma, got:\n{}",
        formatted,
    );
    assert!(
        !formatted.contains("^FO50,100,"),
        "Unexpected trailing comma in:\n{}",
        formatted,
    );
}

// ── No-tables fallback ──────────────────────────────────────────────────

#[test]
fn no_tables_simple_roundtrip() {
    assert_roundtrip_no_tables("^XA^FO50,100^FDHello^FS^XZ");
}

// ── Graphic box (many params) ───────────────────────────────────────────

#[test]
fn graphic_box_gb_roundtrip() {
    assert_roundtrip("^XA^FO0,0^GB812,4,4,B,0^FS^XZ", &common::TABLES);
}

// ── Indentation modes ───────────────────────────────────────────────────

#[test]
fn indent_label_mode() {
    let tables = &common::TABLES;
    let input = "^XA^FO50,100^FDHello^FS^XZ";
    let config = EmitConfig {
        indent: Indent::Label,
    };
    let res = parse_with_tables(input, Some(tables));
    let formatted = emit_zpl(&res.ast, Some(tables), &config);

    // Commands inside ^XA/^XZ should be indented with 2 spaces.
    for line in formatted.lines() {
        if line == "^XA" || line == "^XZ" {
            assert!(!line.starts_with("  "), "^XA/^XZ should not be indented");
        } else if !line.is_empty() {
            assert!(
                line.starts_with("  "),
                "Expected indentation, got: {:?}",
                line
            );
        }
    }
}

#[test]
fn indent_field_mode() {
    let tables = &common::TABLES;
    let input = "^XA^FO50,100^FDHello^FS^XZ";
    let config = EmitConfig {
        indent: Indent::Field,
    };
    let res = parse_with_tables(input, Some(tables));
    let formatted = emit_zpl(&res.ast, Some(tables), &config);

    // Inside a field block (after ^FO), commands should have 4-space indent.
    let lines: Vec<&str> = formatted.lines().collect();
    for line in &lines {
        if line.contains("^FD") || line.contains("^FS") {
            assert!(
                line.starts_with("    "),
                "Expected 4-space indent inside field, got: {:?}",
                line
            );
        }
    }
}

#[test]
fn indent_none_is_flat() {
    let tables = &common::TABLES;
    let input = "^XA^FO50,100^FDHello^FS^XZ";
    let config = EmitConfig {
        indent: Indent::None,
    };
    let res = parse_with_tables(input, Some(tables));
    let formatted = emit_zpl(&res.ast, Some(tables), &config);

    for line in formatted.lines() {
        assert!(
            !line.starts_with(' '),
            "Indent::None should produce no leading spaces, got: {:?}",
            line
        );
    }
}

// ── USPS sample file round-trip ─────────────────────────────────────────

#[test]
fn usps_surepost_sample_roundtrip() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/usps_surepost_sample.zpl");
    let input = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    assert_roundtrip(&input, &common::TABLES);
}

// ── Idempotency ─────────────────────────────────────────────────────────

#[test]
fn format_is_idempotent() {
    let tables = &common::TABLES;
    let input = "^XA^FO50,100^A0N,30,30^FDHello World^FS^GB200,100,3^FS^XZ";
    let config = EmitConfig::default();

    let res1 = parse_with_tables(input, Some(tables));
    let fmt1 = emit_zpl(&res1.ast, Some(tables), &config);

    let res2 = parse_with_tables(&fmt1, Some(tables));
    let fmt2 = emit_zpl(&res2.ast, Some(tables), &config);

    assert_eq!(fmt1, fmt2, "Formatting should be idempotent");
}

// ── Prefix/delimiter change ─────────────────────────────────────────────

#[test]
fn prefix_change_cc_roundtrip() {
    assert_roundtrip("^XA^CC*\n*FO50,100\n*FDTest\n*FS\n*XZ", &common::TABLES);
}

// ── Commands with no args ───────────────────────────────────────────────

#[test]
fn no_arg_commands_roundtrip() {
    assert_roundtrip("^XA^FS^XZ", &common::TABLES);
}

// ── Hex escape field data ───────────────────────────────────────────────

#[test]
fn hex_escape_field_data_roundtrip() {
    assert_roundtrip("^XA^FO10,10^FH_^FDHello_0AWorld^FS^XZ", &common::TABLES);
}
