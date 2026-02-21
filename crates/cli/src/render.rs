//! Pretty diagnostic rendering using ariadne.
//!
//! Converts the toolchain's [`Diagnostic`] type into ariadne [`Report`]s for
//! coloured, source-annotated terminal output. Falls back to structured JSON
//! when the output is piped or when the user explicitly requests it.
//! Supports SARIF 2.1.0 output for CI and tooling integration.

use std::io::{self, IsTerminal};

use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use zpl_toolchain_diagnostics::{Diagnostic, LineIndex, Severity};

/// One SARIF artifact entry with its source and diagnostics.
pub(crate) struct SarifArtifactInput<'a> {
    pub source: &'a str,
    pub artifact_uri: &'a str,
    pub diagnostics: &'a [Diagnostic],
}

// ── Output format ───────────────────────────────────────────────────────

/// Output format for diagnostic rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    /// Coloured, source-annotated output (ariadne).
    Pretty,
    /// Machine-readable JSON.
    Json,
    /// SARIF 2.1.0 for CI/tooling integration (GitHub Code Scanning, etc.).
    Sarif,
}

impl Format {
    /// Resolve `Auto` to a concrete format based on whether stdout is a TTY.
    pub(crate) fn resolve_or_detect(explicit: Option<&str>) -> Self {
        match explicit {
            Some("json") => Format::Json,
            Some("pretty") => Format::Pretty,
            Some("sarif") => Format::Sarif,
            // Default: pretty for interactive terminals, JSON for pipes
            _ => {
                if io::stdout().is_terminal() {
                    Format::Pretty
                } else {
                    Format::Json
                }
            }
        }
    }
}

// ── Severity mapping ────────────────────────────────────────────────────

fn report_kind(severity: &Severity) -> ReportKind<'static> {
    match severity {
        Severity::Error => ReportKind::Error,
        Severity::Warn => ReportKind::Warning,
        Severity::Info => ReportKind::Advice,
        _ => ReportKind::Warning,
    }
}

fn severity_color(severity: &Severity) -> Color {
    match severity {
        Severity::Error => Color::Red,
        Severity::Warn => Color::Yellow,
        Severity::Info => Color::Blue,
        _ => Color::White,
    }
}

// ── Pretty rendering ────────────────────────────────────────────────────

/// Render a slice of diagnostics in pretty (ariadne) format to stderr.
///
/// Diagnostics with a [`Span`] are rendered with source context (line numbers,
/// underlines, labels). Those without a span are rendered as standalone
/// messages.
pub(crate) fn render_diagnostics_pretty(source: &str, filename: &str, diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }

    let config = Config::default().with_compact(false);

    // Build the Source once (O(n) line index) and reuse across all reports.
    let mut cache = (filename, Source::from(source));

    for diag in diagnostics {
        if let Some(span) = &diag.span {
            // Clamp span to source length to avoid panics on truncated input.
            let start = span.start.min(source.len());
            let end = span.end.min(source.len()).max(start);

            let mut builder = Report::build(report_kind(&diag.severity), (filename, start..end))
                .with_code(diag.id.as_ref())
                .with_message(&diag.message)
                .with_config(config);

            // Use context for a more specific label when available,
            // otherwise fall back to the diagnostic message.
            let label_msg = make_label_message(diag);
            builder = builder.with_label(
                Label::new((filename, start..end))
                    .with_message(label_msg)
                    .with_color(severity_color(&diag.severity)),
            );

            // If context is present, add it as a note.
            if let Some(ctx) = &diag.context {
                let note: String = ctx
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                builder = builder.with_note(note);
            }

            // If an explanation exists for this code, add it as help.
            if let Some(explanation) = diag.explain() {
                builder = builder.with_help(explanation);
            }

            builder.finish().eprint(&mut cache).ok();
        } else {
            // No span — print a standalone message to stderr.
            let kind_str = match diag.severity {
                Severity::Error => "error",
                Severity::Warn => "warning",
                Severity::Info => "info",
                _ => "diagnostic",
            };
            eprintln!("{kind_str}[{}]: {}", diag.id, diag.message);

            if let Some(ctx) = &diag.context {
                let note: String = ctx
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!("  = note: {note}");
            }

            if let Some(explanation) = diag.explain() {
                eprintln!("  = help: {explanation}");
            }
        }
    }
}

/// Build a concise label message from diagnostic context, avoiding duplication
/// with the report header message.
fn make_label_message(diag: &Diagnostic) -> String {
    // When context provides structured details, build a compact label from them
    // (e.g. "command=^BY, field=w, value=15").
    // Otherwise fall back to the full message (which already appears in the
    // report header, but ariadne still looks good with it inline).
    if let Some(ctx) = &diag.context
        && !ctx.is_empty()
    {
        ctx.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        diag.message.clone()
    }
}

// ── JSON rendering ──────────────────────────────────────────────────────

/// Render diagnostics as a JSON array to stdout.
pub(crate) fn render_diagnostics_json(diagnostics: &[Diagnostic]) {
    let json =
        serde_json::to_string_pretty(diagnostics).expect("Diagnostic serialization cannot fail");
    println!("{json}");
}

// ── SARIF 2.1.0 rendering ───────────────────────────────────────────────

/// Render diagnostics as SARIF 2.1.0 JSON to stdout.
///
/// Maps toolchain diagnostics to SARIF results with rule IDs, severity levels,
/// and physical locations (byte offsets or line/column when available).
/// Suitable for GitHub Code Scanning, VS Code SARIF viewers, and other CI tools.
pub(crate) fn render_diagnostics_sarif(
    source: &str,
    artifact_uri: &str,
    diagnostics: &[Diagnostic],
) {
    let single = [SarifArtifactInput {
        source,
        artifact_uri,
        diagnostics,
    }];
    render_diagnostics_sarif_multi(&single);
}

/// Render diagnostics for one or more artifacts as a single SARIF 2.1.0 log.
pub(crate) fn render_diagnostics_sarif_multi(entries: &[SarifArtifactInput<'_>]) {
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut all_diagnostics: Vec<Diagnostic> = Vec::new();
    let mut artifacts: Vec<serde_json::Value> = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let line_index = LineIndex::new(entry.source);
        for d in entry.diagnostics {
            results.push(diagnostic_to_sarif_result(
                d,
                entry.artifact_uri,
                entry.source,
                &line_index,
                index,
            ));
            all_diagnostics.push(d.clone());
        }
        artifacts.push(serde_json::json!({
            "location": { "uri": entry.artifact_uri },
            "length": entry.source.len(),
            "sourceLanguage": "zpl",
        }));
    }

    let mut extra = serde_json::Map::new();
    extra.insert("artifacts".to_string(), serde_json::Value::Array(artifacts));
    emit_sarif_run(
        "zpl-toolchain",
        collect_unique_rules(&all_diagnostics),
        results,
        true,
        Some(extra),
    )
    .expect("SARIF serialization cannot fail");
}

/// Convert a single diagnostic to a SARIF result object.
fn diagnostic_to_sarif_result(
    d: &Diagnostic,
    artifact_uri: &str,
    source: &str,
    line_index: &LineIndex,
    artifact_index: usize,
) -> serde_json::Value {
    let level = match d.severity {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Info => "note",
        _ => "warning",
    };

    let mut result = serde_json::json!({
        "ruleId": d.id.as_ref(),
        "level": level,
        "message": {
            "text": d.message
        }
    });

    // Add physical location when span is present.
    if let Some(span) = &d.span {
        let start = span.start.min(source.len());
        let end = span.end.min(source.len()).max(start);
        let (start_line, start_col) = line_index.line_col(start);
        let (end_line, end_col) = line_index.line_col(end);

        // SARIF uses 1-based line/column.
        let mut region = serde_json::json!({
            "byteOffset": start,
            "byteLength": end.saturating_sub(start),
            "startLine": start_line + 1,
            "startColumn": start_col + 1,
            "endLine": end_line + 1,
            "endColumn": end_col + 1
        });

        // Add snippet when non-empty span
        if start < end
            && end <= source.len()
            && let Some(snippet) = source.get(start..end)
        {
            region["snippet"] = serde_json::json!({"text": snippet});
        }

        result["locations"] = serde_json::json!([{
            "physicalLocation": {
                "artifactLocation": {
                    "uri": artifact_uri,
                    "index": artifact_index
                },
                "region": region
            }
        }]);
    } else {
        // No span: result applies to the artifact as a whole.
        result["locations"] = serde_json::json!([{
            "physicalLocation": {
                "artifactLocation": {
                    "uri": artifact_uri,
                    "index": artifact_index
                }
            }
        }]);
    }

    // Attach context as SARIF properties for tooling.
    if let Some(ctx) = &d.context
        && !ctx.is_empty()
    {
        let props: serde_json::Map<String, serde_json::Value> = ctx
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        result["properties"] = serde_json::Value::Object(props);
    }

    result
}

/// Build unique rule entries for diagnostics (SARIF tool.driver.rules).
fn collect_unique_rules(diagnostics: &[Diagnostic]) -> Vec<serde_json::Value> {
    use std::collections::BTreeSet;
    let mut seen = BTreeSet::new();
    let mut rules = Vec::new();
    for d in diagnostics {
        let id = d.id.as_ref();
        if seen.insert(id) {
            let short = d.explain().unwrap_or(d.message.as_str());
            let mut rule = serde_json::json!({
                "id": id,
                "shortDescription": {"text": short}
            });
            if let Some(help) = d.explain() {
                rule["fullDescription"] = serde_json::json!({"text": help});
            }
            rules.push(rule);
        }
    }
    rules
}

pub(crate) fn sarif_rule(rule_id: &str, short_description: &str) -> serde_json::Value {
    serde_json::json!({
        "id": rule_id,
        "name": rule_id,
        "shortDescription": { "text": short_description },
    })
}

pub(crate) fn sarif_result(rule_id: &str, level: &str, message: String) -> serde_json::Value {
    serde_json::json!({
        "ruleId": rule_id,
        "level": level,
        "message": { "text": message },
    })
}

pub(crate) fn emit_sarif_run(
    tool_name: &str,
    rules: Vec<serde_json::Value>,
    results: Vec<serde_json::Value>,
    execution_successful: bool,
    extra_run_fields: Option<serde_json::Map<String, serde_json::Value>>,
) -> anyhow::Result<()> {
    let mut rules = rules;
    rules.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    rules.dedup_by(|a, b| a["id"] == b["id"]);

    let mut run = serde_json::json!({
        "tool": {
            "driver": {
                "name": tool_name,
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://github.com/trevordcampbell/zpl-toolchain",
                "rules": rules,
            }
        },
        "invocations": [
            {
                "executionSuccessful": execution_successful,
            }
        ],
        "results": results,
    });
    if let Some(extra) = extra_run_fields {
        for (key, value) in extra {
            run[key] = value;
        }
    }
    let sarif = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [run]
    });
    println!("{}", serde_json::to_string_pretty(&sarif)?);
    Ok(())
}

// ── Unified entry point ─────────────────────────────────────────────────

/// Render diagnostics in the given format.
///
/// - `Pretty` → coloured output to stderr (source data stays on stdout).
/// - `Json`   → JSON array to stdout.
/// - `Sarif`  → SARIF 2.1.0 JSON to stdout.
pub(crate) fn render_diagnostics(
    source: &str,
    filename: &str,
    diagnostics: &[Diagnostic],
    format: Format,
) {
    match format {
        Format::Pretty => render_diagnostics_pretty(source, filename, diagnostics),
        Format::Json => render_diagnostics_json(diagnostics),
        Format::Sarif => {
            let uri = artifact_uri_for_file(filename);
            render_diagnostics_sarif(source, &uri, diagnostics);
        }
    }
}

/// Convert CLI file path to SARIF artifact URI.
fn artifact_uri_for_file(file: &str) -> String {
    if file == "-" {
        "stdin".to_string()
    } else {
        // Use path as-is; consumers can resolve relative paths.
        file.to_string()
    }
}

// ── Summary line ────────────────────────────────────────────────────────

/// Print a coloured summary line showing error/warning/info counts.
///
/// Example: `2 errors, 1 warning, 0 info`
pub(crate) fn print_summary(diagnostics: &[Diagnostic]) {
    use ariadne::Fmt;

    let (mut errors, mut warnings, mut infos) = (0usize, 0usize, 0usize);
    for d in diagnostics {
        match d.severity {
            Severity::Error => errors += 1,
            Severity::Warn => warnings += 1,
            Severity::Info => infos += 1,
            _ => warnings += 1,
        }
    }

    // Only print summary when there are diagnostics.
    if errors + warnings + infos == 0 {
        return;
    }

    let mut parts = Vec::new();
    if errors > 0 {
        let s = if errors == 1 { "" } else { "s" };
        parts.push(format!("{}", format!("{errors} error{s}").fg(Color::Red)));
    }
    if warnings > 0 {
        let s = if warnings == 1 { "" } else { "s" };
        parts.push(format!(
            "{}",
            format!("{warnings} warning{s}").fg(Color::Yellow)
        ));
    }
    if infos > 0 {
        parts.push(format!("{}", format!("{infos} info").fg(Color::Blue)));
    }
    eprintln!("{}", parts.join(", "));
}
