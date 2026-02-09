//! Tests for rich field data parsing and validation (hex escapes, field numbers).

mod common;

use zpl_toolchain_diagnostics::codes;

// NOTE: This file uses synthetic command codes (e.g., ^ZZR, ^ZZC) constructed
// directly in ParserTables for the sole purpose of exercising rich-field
// behaviors like roundingPolicy and conditionalRange without coupling to
// specific real ZPL commands.

#[test]
fn rich_fields_tables_emit_and_load() {
    let tables = &*common::TABLES;
    let mut root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    assert!(
        root.join("generated/constraints_bundle.json").exists(),
        "missing constraints_bundle.json"
    );
    assert!(!tables.commands.is_empty());
}

#[test]
fn rounding_policy_to_multiple_warns() {
    // Build a small tables struct programmatically with a synthetic command ^TR
    let tables = zpl_toolchain_spec_tables::ParserTables::new(
        "1.0.0".into(),
        "0.3.0".into(),
        vec![zpl_toolchain_spec_tables::CommandEntry {
            codes: vec!["^ZZR".to_string()],
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
                    default: None,
                    default_by_dpi: None,
                    default_from: None,
                    profile_constraint: None,
                    range_when: None,
                    rounding_policy: Some(zpl_toolchain_spec_tables::RoundingPolicy {
                        unit: None,
                        mode: zpl_toolchain_spec_tables::RoundingMode::ToMultiple,
                        multiple: Some(5.0),
                    }),
                    rounding_policy_when: None,
                    r#enum: None,
                },
            ))]),

            constraints: None,
            effects: None,
            plane: None,
            scope: None,
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
        }],
        None,
    );
    let input = "^XA^ZZR12^XZ"; // 12 is not a multiple of 5
    let res = zpl_toolchain_core::grammar::parser::parse_with_tables(input, Some(&tables));
    let vr = zpl_toolchain_core::validate::validate(&res.ast, &tables);
    assert!(
        vr.issues.iter().any(|d| d.id == codes::ROUNDING_VIOLATION),
        "expected rounding warning, got: {:?}",
        vr.issues
    );
}

#[test]
fn conditional_range_enforced() {
    // Build a small tables struct with ^TC having two args: a (int) and b (enum),
    // and a conditional range on a when b == X
    let tables = zpl_toolchain_spec_tables::ParserTables::new(
        "1.0.0".into(),
        "0.3.0".into(),
        vec![zpl_toolchain_spec_tables::CommandEntry {
            codes: vec!["^ZZC".to_string()],
            arity: 2,
            raw_payload: false,
            field_data: false,
            opens_field: false,
            closes_field: false,
            hex_escape_modifier: false,
            field_number: false,
            serialization: false,
            requires_field: false,
            signature: Some(zpl_toolchain_spec_tables::Signature {
                params: vec!["a".to_string(), "b".to_string()],
                joiner: ",".to_string(),
                allow_empty_trailing: true,
                split_rule: None,
            }),
            args: Some(vec![
                zpl_toolchain_spec_tables::ArgUnion::Single(Box::new(
                    zpl_toolchain_spec_tables::Arg {
                        name: Some("a".to_string()),
                        key: Some("a".to_string()),
                        r#type: "int".to_string(),
                        unit: None,
                        range: Some([0.0, 100.0]),
                        min_length: None,
                        max_length: None,
                        optional: false,
                        default: None,
                        default_by_dpi: None,
                        default_from: None,
                        profile_constraint: None,
                        range_when: Some(vec![zpl_toolchain_spec_tables::ConditionalRange {
                            when: "arg:bIsValue:X".to_string(),
                            range: [50.0, 100.0],
                        }]),
                        rounding_policy: None,
                        rounding_policy_when: None,
                        r#enum: None,
                    },
                )),
                zpl_toolchain_spec_tables::ArgUnion::Single(Box::new(
                    zpl_toolchain_spec_tables::Arg {
                        name: Some("b".to_string()),
                        key: Some("b".to_string()),
                        r#type: "enum".to_string(),
                        unit: None,
                        range: None,
                        min_length: None,
                        max_length: None,
                        optional: false,
                        default: None,
                        default_by_dpi: None,
                        default_from: None,
                        profile_constraint: None,
                        range_when: None,
                        rounding_policy: None,
                        rounding_policy_when: None,
                        r#enum: Some(vec![
                            zpl_toolchain_spec_tables::EnumValue::Simple("X".to_string()),
                            zpl_toolchain_spec_tables::EnumValue::Simple("Y".to_string()),
                        ]),
                    },
                )),
            ]),

            constraints: None,
            effects: None,
            plane: None,
            scope: None,
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
        }],
        None,
    );

    // Case 1: b = X and a = 40 → violates conditional range [50,100]
    let res1 =
        zpl_toolchain_core::grammar::parser::parse_with_tables("^XA^ZZC40,X^XZ", Some(&tables));
    let vr1 = zpl_toolchain_core::validate::validate(&res1.ast, &tables);
    assert!(
        vr1.issues.iter().any(|d| d.id == codes::OUT_OF_RANGE),
        "expected out-of-range error, got: {:?}",
        vr1.issues
    );

    // Case 2: b = Y and a = 40 → OK under base range [0,100]
    let res2 =
        zpl_toolchain_core::grammar::parser::parse_with_tables("^XA^ZZC40,Y^XZ", Some(&tables));
    let vr2 = zpl_toolchain_core::validate::validate(&res2.ast, &tables);
    assert!(vr2.ok, "unexpected error with b=Y: {:?}", vr2.issues);
}
