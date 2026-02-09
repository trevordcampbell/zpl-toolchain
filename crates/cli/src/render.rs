//! Pretty diagnostic rendering using ariadne.
//!
//! Converts the toolchain's [`Diagnostic`] type into ariadne [`Report`]s for
//! coloured, source-annotated terminal output. Falls back to structured JSON
//! when the output is piped or when the user explicitly requests it.

use std::io::{self, IsTerminal};

use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use zpl_toolchain_diagnostics::{Diagnostic, Severity};

// ── Output format ───────────────────────────────────────────────────────

/// Output format for diagnostic rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    /// Coloured, source-annotated output (ariadne).
    Pretty,
    /// Machine-readable JSON.
    Json,
}

impl Format {
    /// Resolve `Auto` to a concrete format based on whether stdout is a TTY.
    pub(crate) fn resolve_or_detect(explicit: Option<&str>) -> Self {
        match explicit {
            Some("json") => Format::Json,
            Some("pretty") => Format::Pretty,
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

// ── Unified entry point ─────────────────────────────────────────────────

/// Render diagnostics in the given format.
///
/// - `Pretty` → coloured output to stderr (source data stays on stdout).
/// - `Json`   → JSON array to stdout.
pub(crate) fn render_diagnostics(
    source: &str,
    filename: &str,
    diagnostics: &[Diagnostic],
    format: Format,
) {
    match format {
        Format::Pretty => render_diagnostics_pretty(source, filename, diagnostics),
        Format::Json => render_diagnostics_json(diagnostics),
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
