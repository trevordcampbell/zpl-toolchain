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

/// Returns a reference to the embedded parser tables (compiled-in from the spec).
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

/// Returns `None` when parser tables are not embedded at compile time.
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

// ── Print (non-WASM only) ────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
use zpl_toolchain_print_client::{Printer, PrinterConfig, StatusQuery, TcpPrinter};

/// Send ZPL to a network printer via TCP (port 9100).
///
/// If `validate` is true the ZPL is validated first (using the optional
/// printer profile); validation failures are returned as a JSON error
/// instead of sending anything to the printer.
///
/// Returns a JSON string on success: `{"success": true, "bytes_sent": N}`
/// or a JSON error object on validation failure.
#[cfg(not(target_arch = "wasm32"))]
pub fn print_zpl(
    zpl: &str,
    printer_addr: &str,
    profile_json: Option<&str>,
    validate: bool,
) -> Result<String, String> {
    // 1. If validate is true, run validation first
    if validate {
        let vr = validate_zpl(zpl, profile_json)?;
        if !vr.ok {
            let issues_json =
                serde_json::to_value(&vr.issues).map_err(|e| format!("serialize error: {e}"))?;
            return Ok(serde_json::json!({
                "success": false,
                "error": "validation_failed",
                "issues": issues_json,
            })
            .to_string());
        }
    }

    // 2. Connect to printer via TcpPrinter
    let config = PrinterConfig::default();
    let mut printer =
        TcpPrinter::connect(printer_addr, config).map_err(|e| format!("connection failed: {e}"))?;

    // 3. Send ZPL
    let bytes_sent = zpl.len();
    printer
        .send_zpl(zpl)
        .map_err(|e| format!("send failed: {e}"))?;

    // 4. Return JSON result
    Ok(serde_json::json!({
        "success": true,
        "bytes_sent": bytes_sent,
    })
    .to_string())
}

/// Query printer status via `~HS` and return the result as JSON.
///
/// Connects to the printer, sends `~HS`, parses the three-line response
/// into a [`HostStatus`](zpl_toolchain_print_client::HostStatus) struct,
/// and serializes it to JSON.
#[cfg(not(target_arch = "wasm32"))]
pub fn query_printer_status(printer_addr: &str) -> Result<String, String> {
    let config = PrinterConfig::default();
    let mut printer =
        TcpPrinter::connect(printer_addr, config).map_err(|e| format!("connection failed: {e}"))?;

    let status = printer
        .query_status()
        .map_err(|e| format!("status query failed: {e}"))?;

    serde_json::to_string(&status).map_err(|e| format!("serialize error: {e}"))
}
