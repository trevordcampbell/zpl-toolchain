//! Python bindings for the ZPL toolchain.
//!
//! Exposes parse, validate, format, and explain functions to Python
//! via PyO3. Structured APIs return native Python dict/list objects.

use pyo3::prelude::*;

use zpl_toolchain_bindings_common as common;

fn to_python_value(py: Python<'_>, json_text: String) -> PyResult<Py<PyAny>> {
    let json_mod = py.import("json")?;
    let loads = json_mod.getattr("loads")?;
    let obj = loads.call1((json_text,))?;
    Ok(obj.unbind())
}

fn json_result_to_python(
    py: Python<'_>,
    json: Result<String, serde_json::Error>,
) -> PyResult<Py<PyAny>> {
    let json = json.map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    to_python_value(py, json)
}

// ── Public API ──────────────────────────────────────────────────────────

/// Parse a ZPL string and return `{ ast, diagnostics }` as a Python dict by default.
///
/// Uses embedded parser tables and raises when unavailable.
#[pyfunction]
fn parse(py: Python<'_>, input: &str) -> PyResult<Py<PyAny>> {
    let result = common::parse_zpl(input).map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
    json_result_to_python(py, serde_json::to_string(&result))
}

/// Parse a ZPL string with explicitly provided parser tables (JSON string).
///
/// Returns `{ ast, diagnostics }` as a Python dict by default.
#[pyfunction]
fn parse_with_tables(py: Python<'_>, input: &str, tables_json: &str) -> PyResult<Py<PyAny>> {
    let result = common::parse_zpl_with_tables_json(input, tables_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    json_result_to_python(py, serde_json::to_string(&result))
}

/// Parse and validate a ZPL string.
///
/// Returns `{ ok, issues, resolved_labels }` as a Python dict by default. Optionally accepts a
/// printer profile JSON string for contextual validation.
#[pyfunction]
#[pyo3(signature = (input, profile_json=None))]
fn validate(py: Python<'_>, input: &str, profile_json: Option<&str>) -> PyResult<Py<PyAny>> {
    let vr = common::validate_zpl(input, profile_json)
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
    json_result_to_python(py, serde_json::to_string(&vr))
}

/// Parse and validate using explicit parser tables (JSON string).
///
/// Returns `{ ok, issues, resolved_labels }` as a Python dict.
#[pyfunction]
#[pyo3(signature = (input, tables_json, profile_json=None))]
fn validate_with_tables(
    py: Python<'_>,
    input: &str,
    tables_json: &str,
    profile_json: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let vr = common::validate_zpl_with_tables_json(input, profile_json, tables_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    json_result_to_python(py, serde_json::to_string(&vr))
}

/// Format a ZPL string (normalize whitespace, one command per line).
///
/// `indent` controls indentation: `"none"` (default), `"label"`, or `"field"`.
/// `compaction` controls optional compaction: `"none"` (default) or `"field"`.
/// Returns the formatted ZPL string.
#[pyfunction]
#[pyo3(signature = (input, indent=None, compaction=None))]
fn format(input: &str, indent: Option<&str>, compaction: Option<&str>) -> PyResult<String> {
    common::format_zpl_with_options(input, indent, compaction)
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)
}

/// Explain a diagnostic code (e.g., "ZPL1201").
///
/// Returns the explanation string, or None if unknown.
#[pyfunction]
fn explain(id: &str) -> Option<String> {
    common::explain_diagnostic(id).map(|s| s.to_string())
}

// ── Print (non-WASM only) ────────────────────────────────────────────

/// Send ZPL to a network printer via TCP (port 9100).
///
/// If `validate` is true (the default) the ZPL is validated first using
/// the optional `profile_json`. Validation failures are returned as JSON
/// instead of sending anything to the printer.
///
/// Returns a Python dict.
#[cfg(not(target_arch = "wasm32"))]
#[pyfunction]
#[pyo3(signature = (zpl, printer_addr, profile_json=None, validate=true))]
fn print_zpl(
    py: Python<'_>,
    zpl: &str,
    printer_addr: &str,
    profile_json: Option<&str>,
    validate: bool,
) -> PyResult<Py<PyAny>> {
    print_zpl_with_options(py, zpl, printer_addr, profile_json, validate, None, None)
}

/// Send ZPL to a network printer via TCP with optional timeout/config overrides.
#[cfg(not(target_arch = "wasm32"))]
#[pyfunction]
#[pyo3(signature = (zpl, printer_addr, profile_json=None, validate=true, timeout_ms=None, config_json=None))]
fn print_zpl_with_options(
    py: Python<'_>,
    zpl: &str,
    printer_addr: &str,
    profile_json: Option<&str>,
    validate: bool,
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let json = common::print_zpl_with_options(
        zpl,
        printer_addr,
        profile_json,
        validate,
        timeout_ms,
        config_json,
    )
    .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
    to_python_value(py, json)
}

/// Query printer status via `~HS` and return a Python dict by default.
///
/// Connects to the printer at `printer_addr`, sends the `~HS` command,
/// and returns the parsed host-status fields as JSON.
#[cfg(not(target_arch = "wasm32"))]
#[pyfunction]
fn query_printer_status(py: Python<'_>, printer_addr: &str) -> PyResult<Py<PyAny>> {
    query_printer_status_with_options(py, printer_addr, None, None)
}

/// Query printer status via `~HS` with optional timeout/config overrides.
#[cfg(not(target_arch = "wasm32"))]
#[pyfunction]
#[pyo3(signature = (printer_addr, timeout_ms=None, config_json=None))]
fn query_printer_status_with_options(
    py: Python<'_>,
    printer_addr: &str,
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let json = common::query_printer_status_with_options(printer_addr, timeout_ms, config_json)
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
    to_python_value(py, json)
}

/// Query printer info via `~HI` and return a Python dict by default.
#[cfg(not(target_arch = "wasm32"))]
#[pyfunction]
fn query_printer_info(py: Python<'_>, printer_addr: &str) -> PyResult<Py<PyAny>> {
    query_printer_info_with_options(py, printer_addr, None, None)
}

/// Query printer info via `~HI` with optional timeout/config overrides.
#[cfg(not(target_arch = "wasm32"))]
#[pyfunction]
#[pyo3(signature = (printer_addr, timeout_ms=None, config_json=None))]
fn query_printer_info_with_options(
    py: Python<'_>,
    printer_addr: &str,
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let json = common::query_printer_info_with_options(printer_addr, timeout_ms, config_json)
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)?;
    to_python_value(py, json)
}

// ── Module ──────────────────────────────────────────────────────────────

/// ZPL toolchain — parse, validate, and format Zebra Programming Language files.
#[pymodule]
fn zpl_toolchain(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(parse_with_tables, m)?)?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(validate_with_tables, m)?)?;
    m.add_function(wrap_pyfunction!(format, m)?)?;
    m.add_function(wrap_pyfunction!(explain, m)?)?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        m.add_function(wrap_pyfunction!(print_zpl, m)?)?;
        m.add_function(wrap_pyfunction!(print_zpl_with_options, m)?)?;
        m.add_function(wrap_pyfunction!(query_printer_status, m)?)?;
        m.add_function(wrap_pyfunction!(query_printer_status_with_options, m)?)?;
        m.add_function(wrap_pyfunction!(query_printer_info, m)?)?;
        m.add_function(wrap_pyfunction!(query_printer_info_with_options, m)?)?;
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{
        print_zpl_with_options, query_printer_info_with_options, query_printer_status_with_options,
        validate_with_tables,
    };
    use pyo3::Python;

    #[test]
    fn print_with_options_rejects_zero_timeout() {
        Python::with_gil(|py| {
            let err =
                print_zpl_with_options(py, "^XA^XZ", "127.0.0.1:9100", None, false, Some(0), None)
                    .expect_err("timeout=0 should fail before I/O");
            assert!(err.to_string().contains("timeout_ms must be > 0"));
        });
    }

    #[test]
    fn query_status_with_options_rejects_zero_timeout() {
        Python::with_gil(|py| {
            let err = query_printer_status_with_options(py, "127.0.0.1:9100", Some(0), None)
                .expect_err("timeout=0 should fail before I/O");
            assert!(err.to_string().contains("timeout_ms must be > 0"));
        });
    }

    #[test]
    fn query_info_with_options_rejects_zero_timeout() {
        Python::with_gil(|py| {
            let err = query_printer_info_with_options(py, "127.0.0.1:9100", Some(0), None)
                .expect_err("timeout=0 should fail before I/O");
            assert!(err.to_string().contains("timeout_ms must be > 0"));
        });
    }

    #[test]
    fn validate_with_tables_rejects_invalid_tables_json() {
        Python::with_gil(|py| {
            let err = validate_with_tables(py, "^XA^XZ", "{invalid", None)
                .expect_err("invalid tables json should fail");
            assert!(err.to_string().contains("invalid"));
        });
    }
}
