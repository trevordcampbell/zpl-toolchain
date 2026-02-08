//! Shared logic for ZPL toolchain language bindings (FFI, WASM, Python).
//!
//! Contains the core workflows for parse, validate, format, and explain
//! that are common across all binding targets. Each binding crate wraps
//! these functions with its own type conversion layer.

use std::sync::OnceLock;

use zpl_toolchain_core::{
    EmitConfig, Indent, ParseResult, ValidationResult, emit_zpl, parse_str, parse_with_tables,
    validate_with_profile,
};
use zpl_toolchain_profile::{Profile, load_profile_from_str};
use zpl_toolchain_spec_tables::ParserTables;

// ── Embedded tables ─────────────────────────────────────────────────────

static TABLES: OnceLock<Option<ParserTables>> = OnceLock::new();

#[cfg(has_embedded_tables)]
pub fn embedded_tables() -> Option<&'static ParserTables> {
    TABLES
        .get_or_init(|| {
            let json = include_str!(concat!(env!("OUT_DIR"), "/parser_tables.json"));
            Some(
                serde_json::from_str(json)
                    .expect("embedded parser_tables.json is invalid — this is a build-system bug"),
            )
        })
        .as_ref()
}

#[cfg(not(has_embedded_tables))]
pub fn embedded_tables() -> Option<&'static ParserTables> {
    None
}

// ── Parse ───────────────────────────────────────────────────────────────

/// Parse ZPL input using embedded tables if available, otherwise table-less.
pub fn parse_zpl(input: &str) -> ParseResult {
    match embedded_tables() {
        Some(t) => parse_with_tables(input, Some(t)),
        None => parse_str(input),
    }
}

/// Parse ZPL input with explicitly provided tables JSON.
pub fn parse_zpl_with_tables_json(input: &str, tables_json: &str) -> Result<ParseResult, String> {
    let tables: ParserTables =
        serde_json::from_str(tables_json).map_err(|e| format!("invalid tables JSON: {}", e))?;
    Ok(parse_with_tables(input, Some(&tables)))
}

// ── Validate ────────────────────────────────────────────────────────────

/// Parse and validate ZPL input with an optional profile.
///
/// Returns a `ValidationResult` with parse diagnostics merged in.
/// Requires embedded tables; returns `Err` if not available.
pub fn validate_zpl(input: &str, profile_json: Option<&str>) -> Result<ValidationResult, String> {
    let tables = embedded_tables()
        .ok_or_else(|| "parser tables required for validation but not embedded".to_string())?;

    let res = parse_with_tables(input, Some(tables));

    let profile = match profile_json {
        Some(json) => {
            let p: Profile =
                load_profile_from_str(json).map_err(|e| format!("invalid profile: {}", e))?;
            Some(p)
        }
        None => None,
    };

    let mut vr = validate_with_profile(&res.ast, tables, profile.as_ref());
    // Prepend parse diagnostics before validation diagnostics for source-order output.
    let mut all_issues = res.diagnostics;
    all_issues.extend(vr.issues);
    vr.issues = all_issues;
    Ok(vr)
}

// ── Format ──────────────────────────────────────────────────────────────

/// Parse an indent string into the `Indent` enum.
pub fn parse_indent(indent: Option<&str>) -> Indent {
    match indent {
        Some("label") => Indent::Label,
        Some("field") => Indent::Field,
        _ => Indent::None,
    }
}

/// Format ZPL input with the given indent style.
pub fn format_zpl(input: &str, indent: Option<&str>) -> String {
    let tables = embedded_tables();
    let res = match tables {
        Some(t) => parse_with_tables(input, Some(t)),
        None => parse_str(input),
    };

    let config = EmitConfig {
        indent: parse_indent(indent),
    };
    emit_zpl(&res.ast, tables, &config)
}

// ── Explain ─────────────────────────────────────────────────────────────

/// Explain a diagnostic code, returning the human-readable description.
pub fn explain_diagnostic(id: &str) -> Option<&'static str> {
    zpl_toolchain_diagnostics::explain(id)
}
