//! Tests for argument union parsing (positions accepting multiple types).

#[test]
fn arg_union_accepts_either_shape() {
    // Build tables with a synthetic ^ZZU command where the first position is a union:
    // either int (key "n") or enum (key "m" with values A/B)
    let tables = zpl_toolchain_spec_tables::ParserTables::new(
        "1.0.0".into(),
        "0.3.0".into(),
        vec![zpl_toolchain_spec_tables::CommandEntry {
            codes: vec!["^ZZU".to_string()],
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
                params: vec!["x".to_string()],
                joiner: ",".to_string(),
                no_space_after_opcode: true,
                allow_empty_trailing: true,
                split_rule: None,
            }),
            args: Some(vec![zpl_toolchain_spec_tables::ArgUnion::OneOf {
                one_of: vec![
                    zpl_toolchain_spec_tables::Arg {
                        name: Some("num".to_string()),
                        key: Some("n".to_string()),
                        r#type: "int".to_string(),
                        unit: None,
                        range: Some([0.0, 100.0]),
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
                    zpl_toolchain_spec_tables::Arg {
                        name: Some("mode".to_string()),
                        key: Some("m".to_string()),
                        r#type: "enum".to_string(),
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
                        r#enum: Some(vec![
                            zpl_toolchain_spec_tables::EnumValue::Simple("A".to_string()),
                            zpl_toolchain_spec_tables::EnumValue::Simple("B".to_string()),
                        ]),
                    },
                ],
            }]),
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
    );

    // Case 1: numeric variant
    let res1 =
        zpl_toolchain_core::grammar::parser::parse_with_tables("^XA^ZZU42^XZ", Some(&tables));
    let vr1 = zpl_toolchain_core::validate::validate(&res1.ast, &tables);
    assert!(
        vr1.ok,
        "numeric union variant should pass: {:?}",
        vr1.issues
    );

    // Case 2: enum variant
    let res2 = zpl_toolchain_core::grammar::parser::parse_with_tables("^XA^ZZUA^XZ", Some(&tables));
    let vr2 = zpl_toolchain_core::validate::validate(&res2.ast, &tables);
    assert!(vr2.ok, "enum union variant should pass: {:?}", vr2.issues);
}
