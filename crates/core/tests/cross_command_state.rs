//! Tests for cross-command state validation.
//! Verifies that state-producing commands (^BY, ^CF, ^FW) properly satisfy
//! defaultFrom requirements on consumer commands (^BC, ^A).

mod common;

#[test]
fn by_effects_recorded_for_bc() {
    let tables = &*common::TABLES;

    // ^BY command should have effects
    let by_cmd = tables
        .commands
        .iter()
        .find(|c| c.codes.contains(&"^BY".to_string()));
    assert!(by_cmd.is_some(), "^BY should be in tables");
    let by_effects = by_cmd.unwrap().effects.as_ref();
    assert!(by_effects.is_some(), "^BY should have effects");
    assert!(
        by_effects
            .unwrap()
            .sets
            .contains(&"barcode.moduleWidth".to_string()),
        "^BY effects should set barcode.moduleWidth"
    );

    // ^BC height arg should have defaultFrom ^BY
    let bc_cmd = tables
        .commands
        .iter()
        .find(|c| c.codes.contains(&"^BC".to_string()));
    assert!(bc_cmd.is_some(), "^BC should be in tables");
    if let Some(args) = bc_cmd.unwrap().args.as_ref() {
        let height_arg = args.iter().find_map(|a| match a {
            zpl_toolchain_spec_tables::ArgUnion::Single(arg) if arg.key.as_deref() == Some("h") => {
                Some(arg)
            }
            _ => None,
        });
        assert!(height_arg.is_some(), "^BC should have height arg");
        assert_eq!(
            height_arg.unwrap().default_from.as_deref(),
            Some("^BY"),
            "^BC height should defaultFrom ^BY"
        );
    }
}

#[test]
fn cf_fw_effects_recorded_for_a() {
    let tables = &*common::TABLES;

    // ^CF should have effects
    let cf_cmd = tables
        .commands
        .iter()
        .find(|c| c.codes.contains(&"^CF".to_string()));
    assert!(cf_cmd.is_some(), "^CF should be in tables");
    assert!(cf_cmd.unwrap().effects.is_some(), "^CF should have effects");

    // ^FW should have effects
    let fw_cmd = tables
        .commands
        .iter()
        .find(|c| c.codes.contains(&"^FW".to_string()));
    assert!(fw_cmd.is_some(), "^FW should be in tables");
    assert!(fw_cmd.unwrap().effects.is_some(), "^FW should have effects");

    // ^A font arg should have defaultFrom ^CF
    let a_cmd = tables
        .commands
        .iter()
        .find(|c| c.codes.contains(&"^A".to_string()));
    assert!(a_cmd.is_some(), "^A should be in tables");
    if let Some(args) = a_cmd.unwrap().args.as_ref() {
        let font_arg = args.iter().find_map(|a| match a {
            zpl_toolchain_spec_tables::ArgUnion::Single(arg) if arg.key.as_deref() == Some("f") => {
                Some(arg)
            }
            _ => None,
        });
        assert!(font_arg.is_some(), "^A should have font arg");
        assert_eq!(
            font_arg.unwrap().default_from.as_deref(),
            Some("^CF"),
            "^A font should defaultFrom ^CF"
        );

        // orientation should defaultFrom ^FW
        let orient_arg = args.iter().find_map(|a| match a {
            zpl_toolchain_spec_tables::ArgUnion::Single(arg) if arg.key.as_deref() == Some("o") => {
                Some(arg)
            }
            _ => None,
        });
        assert!(orient_arg.is_some(), "^A should have orientation arg");
        assert_eq!(
            orient_arg.unwrap().default_from.as_deref(),
            Some("^FW"),
            "^A orientation should defaultFrom ^FW"
        );
    }
}

#[test]
fn validator_no_missing_error_when_producer_seen() {
    let tables = &*common::TABLES;

    // ^A with ^CF and ^FW preceding — no missing-arg errors expected
    let input = "^XA^CFA,30,20^FWN^A^FDHello^FS^XZ";
    let res = zpl_toolchain_core::grammar::parser::parse_with_tables(input, Some(tables));
    let vr = zpl_toolchain_core::validate::validate(&res.ast, tables);

    // Should have no ZPL1501 errors about ^A args being required but missing
    let missing_errors: Vec<_> = vr
        .issues
        .iter()
        .filter(|d| d.id == "ZPL1501" && d.message.contains("^A"))
        .collect();
    assert!(
        missing_errors.is_empty(),
        "^A args should not be flagged as missing when ^CF/^FW precede it: {:?}",
        missing_errors
    );
}
