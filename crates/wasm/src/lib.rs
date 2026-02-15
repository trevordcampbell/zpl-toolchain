//! WASM bindings for the ZPL toolchain.
//!
//! Exposes parse, validate, format, and explain functions to JavaScript
//! via `wasm-bindgen`. Results are returned as native JS objects using
//! `serde-wasm-bindgen` for zero-copy interop.

use wasm_bindgen::prelude::*;

use zpl_toolchain_bindings_common as common;

// ── Public API ──────────────────────────────────────────────────────────

/// Parse a ZPL string and return `{ ast, diagnostics }`.
///
/// Uses embedded parser tables when available, falls back to table-less
/// parsing otherwise.
#[wasm_bindgen]
pub fn parse(input: &str) -> Result<JsValue, JsError> {
    let result = common::parse_zpl(input);
    to_js(&result)
}

/// Parse a ZPL string with explicitly provided parser tables (JSON string).
///
/// Returns `{ ast, diagnostics }`.
#[wasm_bindgen(js_name = "parseWithTables")]
pub fn parse_with_tables_js(input: &str, tables_json: &str) -> Result<JsValue, JsError> {
    let result =
        common::parse_zpl_with_tables_json(input, tables_json).map_err(|e| JsError::new(&e))?;
    to_js(&result)
}

/// Parse and validate a ZPL string.
///
/// Returns `{ ok, issues, resolved_labels }`. Optionally accepts a printer profile JSON
/// string for contextual validation (e.g., print width bounds).
#[wasm_bindgen(js_name = "validate")]
pub fn validate_zpl(input: &str, profile_json: Option<String>) -> Result<JsValue, JsError> {
    let vr = common::validate_zpl(input, profile_json.as_deref()).map_err(|e| JsError::new(&e))?;
    to_js(&vr)
}

/// Parse and validate a ZPL string with explicitly provided parser tables (JSON string).
///
/// Returns `{ ok, issues, resolved_labels }`.
#[wasm_bindgen(js_name = "validateWithTables")]
pub fn validate_with_tables_js(
    input: &str,
    tables_json: &str,
    profile_json: Option<String>,
) -> Result<JsValue, JsError> {
    let vr = common::validate_zpl_with_tables_json(input, profile_json.as_deref(), tables_json)
        .map_err(|e| JsError::new(&e))?;
    to_js(&vr)
}

/// Format a ZPL string (normalize whitespace, one command per line).
///
/// `indent` controls indentation: `"none"` (default), `"label"`, or `"field"`.
/// `compaction` controls optional compaction: `"none"` (default) or `"field"`.
/// `comment_placement` controls semicolon comments: `"inline"` (default) or `"line"`.
/// Returns the formatted ZPL string.
#[wasm_bindgen]
pub fn format(
    input: &str,
    indent: Option<String>,
    compaction: Option<String>,
    comment_placement: Option<String>,
) -> Result<String, JsError> {
    Ok(common::format_zpl_with_options(
        input,
        indent.as_deref(),
        compaction.as_deref(),
        comment_placement.as_deref(),
    ))
}

/// Explain a diagnostic code (e.g., "ZPL1201").
///
/// Returns the explanation string, or `null` if unknown.
#[wasm_bindgen]
pub fn explain(id: &str) -> Option<String> {
    common::explain_diagnostic(id).map(|s| s.to_string())
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}
