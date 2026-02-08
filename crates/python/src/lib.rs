//! Python bindings for the ZPL toolchain.
//!
//! Exposes parse, validate, format, and explain functions to Python
//! via PyO3. Results are returned as JSON strings for simplicity and
//! zero-dependency interop — callers can `json.loads()` the result.

use pyo3::prelude::*;

use zpl_toolchain_bindings_common as common;

// ── Public API ──────────────────────────────────────────────────────────

/// Parse a ZPL string and return a JSON string with `{ ast, diagnostics }`.
///
/// Uses embedded parser tables when available, falls back to table-less
/// parsing otherwise.
#[pyfunction]
fn parse(input: &str) -> PyResult<String> {
    let result = common::parse_zpl(input);
    serde_json::to_string(&result)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Parse a ZPL string with explicitly provided parser tables (JSON string).
///
/// Returns a JSON string with `{ ast, diagnostics }`.
#[pyfunction]
fn parse_with_tables(input: &str, tables_json: &str) -> PyResult<String> {
    let result = common::parse_zpl_with_tables_json(input, tables_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    serde_json::to_string(&result)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Parse and validate a ZPL string.
///
/// Returns a JSON string with `{ ok, issues }`. Optionally accepts a
/// printer profile JSON string for contextual validation.
#[pyfunction]
#[pyo3(signature = (input, profile_json=None))]
fn validate(input: &str, profile_json: Option<&str>) -> PyResult<String> {
    let vr = common::validate_zpl(input, profile_json)
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
    serde_json::to_string(&vr).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Format a ZPL string (normalize whitespace, one command per line).
///
/// `indent` controls indentation: `"none"` (default), `"label"`, or `"field"`.
/// Returns the formatted ZPL string.
#[pyfunction]
#[pyo3(signature = (input, indent=None))]
fn format(input: &str, indent: Option<&str>) -> PyResult<String> {
    Ok(common::format_zpl(input, indent))
}

/// Explain a diagnostic code (e.g., "ZPL1201").
///
/// Returns the explanation string, or None if unknown.
#[pyfunction]
fn explain(id: &str) -> Option<String> {
    common::explain_diagnostic(id).map(|s| s.to_string())
}

// ── Module ──────────────────────────────────────────────────────────────

/// ZPL toolchain — parse, validate, and format Zebra Programming Language files.
#[pymodule]
fn zpl_toolchain(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(parse_with_tables, m)?)?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(format, m)?)?;
    m.add_function(wrap_pyfunction!(explain, m)?)?;
    Ok(())
}
