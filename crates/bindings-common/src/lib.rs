//! Shared logic for ZPL toolchain language bindings (FFI, WASM, Python).
//!
//! Contains the core workflows for parse, validate, format, and explain
//! that are common across all binding targets. Each binding crate wraps
//! these functions with its own type conversion layer.

use std::sync::OnceLock;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use zpl_toolchain_core::{
    CommentPlacement, Compaction, EmitConfig, Indent, ParseResult, ValidationResult, emit_zpl,
    parse_str, parse_with_tables, validate_with_profile,
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

/// Parse and validate ZPL input with explicitly provided parser tables JSON.
///
/// Returns a `ValidationResult` with parse diagnostics merged in.
pub fn validate_zpl_with_tables_json(
    input: &str,
    profile_json: Option<&str>,
    tables_json: &str,
) -> Result<ValidationResult, String> {
    let tables: ParserTables =
        serde_json::from_str(tables_json).map_err(|e| format!("invalid tables JSON: {}", e))?;
    let res = parse_with_tables(input, Some(&tables));

    let profile = match profile_json {
        Some(json) => {
            let p: Profile =
                load_profile_from_str(json).map_err(|e| format!("invalid profile: {}", e))?;
            Some(p)
        }
        None => None,
    };

    let mut vr = validate_with_profile(&res.ast, &tables, profile.as_ref());
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

/// Parse a compaction string into the `Compaction` enum.
pub fn parse_compaction(compaction: Option<&str>) -> Compaction {
    match compaction {
        Some("field") => Compaction::Field,
        _ => Compaction::None,
    }
}

/// Parse a comment-placement string into the `CommentPlacement` enum.
pub fn parse_comment_placement(comment_placement: Option<&str>) -> CommentPlacement {
    match comment_placement {
        Some("line") => CommentPlacement::Line,
        _ => CommentPlacement::Inline,
    }
}

/// Format ZPL input with the given indent style.
pub fn format_zpl(input: &str, indent: Option<&str>) -> String {
    format_zpl_with_options(input, indent, None, None)
}

/// Format ZPL input with indent and compaction options.
pub fn format_zpl_with_options(
    input: &str,
    indent: Option<&str>,
    compaction: Option<&str>,
    comment_placement: Option<&str>,
) -> String {
    let tables = embedded_tables();
    let res = match tables {
        Some(t) => parse_with_tables(input, Some(t)),
        None => parse_str(input),
    };

    let config = EmitConfig {
        indent: parse_indent(indent),
        compaction: parse_compaction(compaction),
        comment_placement: parse_comment_placement(comment_placement),
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PrintClientConfigInput {
    #[serde(default)]
    timeouts: TimeoutConfigInput,
    #[serde(default)]
    retry: RetryConfigInput,
    trace_io: Option<bool>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeoutConfigInput {
    connect_ms: Option<u64>,
    write_ms: Option<u64>,
    read_ms: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryConfigInput {
    max_attempts: Option<u32>,
    initial_delay_ms: Option<u64>,
    max_delay_ms: Option<u64>,
    jitter: Option<bool>,
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_nonzero(name: &str, value: u64) -> Result<Duration, String> {
    if value == 0 {
        return Err(format!("{name} must be > 0"));
    }
    Ok(Duration::from_millis(value))
}

#[cfg(not(target_arch = "wasm32"))]
fn build_printer_config(
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
) -> Result<PrinterConfig, String> {
    let mut config = PrinterConfig::default();

    // Backward-compatible coarse override: one timeout value that scales
    // connect/write/read similarly to CLI behavior.
    if let Some(ms) = timeout_ms {
        if ms == 0 {
            return Err("timeout_ms must be > 0".to_string());
        }
        let connect = Duration::from_millis(ms);
        config.timeouts.connect = connect;
        config.timeouts.write = connect.mul_f64(6.0);
        config.timeouts.read = connect.mul_f64(2.0);
    }

    let config_json = config_json.and_then(|raw| {
        if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        }
    });

    if let Some(raw_json) = config_json {
        let parsed: PrintClientConfigInput =
            serde_json::from_str(raw_json).map_err(|e| format!("invalid config_json: {e}"))?;

        if let Some(ms) = parsed.timeouts.connect_ms {
            config.timeouts.connect = ensure_nonzero("timeouts.connect_ms", ms)?;
        }
        if let Some(ms) = parsed.timeouts.write_ms {
            config.timeouts.write = ensure_nonzero("timeouts.write_ms", ms)?;
        }
        if let Some(ms) = parsed.timeouts.read_ms {
            config.timeouts.read = ensure_nonzero("timeouts.read_ms", ms)?;
        }

        if let Some(max_attempts) = parsed.retry.max_attempts {
            if max_attempts == 0 {
                return Err("retry.max_attempts must be > 0".to_string());
            }
            config.retry.max_attempts = max_attempts;
        }
        if let Some(ms) = parsed.retry.initial_delay_ms {
            config.retry.initial_delay = ensure_nonzero("retry.initial_delay_ms", ms)?;
        }
        if let Some(ms) = parsed.retry.max_delay_ms {
            config.retry.max_delay = ensure_nonzero("retry.max_delay_ms", ms)?;
        }
        if config.retry.max_delay < config.retry.initial_delay {
            return Err("retry.max_delay_ms must be >= retry.initial_delay_ms".to_string());
        }
        if let Some(jitter) = parsed.retry.jitter {
            config.retry.jitter = jitter;
        }

        if let Some(trace_io) = parsed.trace_io {
            config.trace_io = trace_io;
        }
    }

    Ok(config)
}

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
    print_zpl_with_options(zpl, printer_addr, profile_json, validate, None, None)
}

/// Send ZPL to a network printer with optional timeout/config overrides.
#[cfg(not(target_arch = "wasm32"))]
pub fn print_zpl_with_options(
    zpl: &str,
    printer_addr: &str,
    profile_json: Option<&str>,
    validate: bool,
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
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
    let config = build_printer_config(timeout_ms, config_json)?;
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
    query_printer_status_with_options(printer_addr, None, None)
}

/// Query printer status via `~HS` with optional timeout/config overrides.
#[cfg(not(target_arch = "wasm32"))]
pub fn query_printer_status_with_options(
    printer_addr: &str,
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
) -> Result<String, String> {
    let config = build_printer_config(timeout_ms, config_json)?;
    let mut printer =
        TcpPrinter::connect(printer_addr, config).map_err(|e| format!("connection failed: {e}"))?;

    let status = printer
        .query_status()
        .map_err(|e| format!("status query failed: {e}"))?;

    serde_json::to_string(&status).map_err(|e| format!("serialize error: {e}"))
}

/// Query printer info via `~HI` and return the result as JSON.
#[cfg(not(target_arch = "wasm32"))]
pub fn query_printer_info(printer_addr: &str) -> Result<String, String> {
    query_printer_info_with_options(printer_addr, None, None)
}

/// Query printer info via `~HI` with optional timeout/config overrides.
#[cfg(not(target_arch = "wasm32"))]
pub fn query_printer_info_with_options(
    printer_addr: &str,
    timeout_ms: Option<u64>,
    config_json: Option<&str>,
) -> Result<String, String> {
    let config = build_printer_config(timeout_ms, config_json)?;
    let mut printer =
        TcpPrinter::connect(printer_addr, config).map_err(|e| format!("connection failed: {e}"))?;

    let info = printer
        .query_info()
        .map_err(|e| format!("info query failed: {e}"))?;

    serde_json::to_string(&info).map_err(|e| format!("serialize error: {e}"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{build_printer_config, parse_comment_placement, parse_compaction, parse_indent};
    use std::time::Duration;
    use zpl_toolchain_core::{Compaction, Indent};

    #[test]
    fn timeout_ms_applies_scaled_timeouts() {
        let cfg = build_printer_config(Some(1_000), None).expect("config");
        assert_eq!(cfg.timeouts.connect, Duration::from_millis(1_000));
        assert_eq!(cfg.timeouts.write, Duration::from_millis(6_000));
        assert_eq!(cfg.timeouts.read, Duration::from_millis(2_000));
    }

    #[test]
    fn config_json_overrides_timeout_and_retry_fields() {
        let cfg = build_printer_config(
            Some(1_000),
            Some(
                r#"{
                    "timeouts":{"connect_ms":250,"write_ms":500,"read_ms":750},
                    "retry":{"max_attempts":4,"initial_delay_ms":10,"max_delay_ms":100,"jitter":false},
                    "trace_io":true
                }"#,
            ),
        )
        .expect("config");

        assert_eq!(cfg.timeouts.connect, Duration::from_millis(250));
        assert_eq!(cfg.timeouts.write, Duration::from_millis(500));
        assert_eq!(cfg.timeouts.read, Duration::from_millis(750));
        assert_eq!(cfg.retry.max_attempts, 4);
        assert_eq!(cfg.retry.initial_delay, Duration::from_millis(10));
        assert_eq!(cfg.retry.max_delay, Duration::from_millis(100));
        assert!(!cfg.retry.jitter);
        assert!(cfg.trace_io);
    }

    #[test]
    fn invalid_config_values_are_rejected() {
        let err = build_printer_config(None, Some(r#"{"timeouts":{"connect_ms":0}}"#))
            .expect_err("should fail");
        assert!(err.contains("connect_ms"));

        let err = build_printer_config(None, Some(r#"{"retry":{"max_attempts":0}}"#))
            .expect_err("should fail");
        assert!(err.contains("max_attempts"));

        let err = build_printer_config(
            None,
            Some(r#"{"retry":{"initial_delay_ms":50,"max_delay_ms":10}}"#),
        )
        .expect_err("should fail");
        assert!(err.contains("max_delay_ms"));
    }

    #[test]
    fn empty_config_json_is_treated_as_none() {
        let cfg = build_printer_config(Some(1_000), Some("   ")).expect("config");
        assert_eq!(cfg.timeouts.connect, Duration::from_millis(1_000));
    }

    #[test]
    fn parse_indent_and_compaction_are_independent() {
        assert_eq!(parse_indent(Some("label")), Indent::Label);
        assert_eq!(parse_compaction(Some("field")), Compaction::Field);
        assert_eq!(parse_indent(Some("field")), Indent::Field);
        assert_eq!(parse_compaction(None), Compaction::None);
    }

    #[test]
    fn parse_comment_placement_defaults_to_inline() {
        assert_eq!(
            parse_comment_placement(None),
            zpl_toolchain_core::CommentPlacement::Inline
        );
        assert_eq!(
            parse_comment_placement(Some("line")),
            zpl_toolchain_core::CommentPlacement::Line
        );
        assert_eq!(
            parse_comment_placement(Some("bogus")),
            zpl_toolchain_core::CommentPlacement::Inline
        );
    }
}
