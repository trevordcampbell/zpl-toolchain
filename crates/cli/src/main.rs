mod render;

use std::fs;

use std::process;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use zpl_toolchain_core::grammar::{
    dump::to_pretty_json,
    emit::{EmitConfig, Indent, emit_zpl},
    parser::{parse_str, parse_with_tables},
    tables::ParserTables,
};
use zpl_toolchain_core::validate;
use zpl_toolchain_diagnostics::{self as diag, Diagnostic, Severity};

use crate::render::{Format, print_summary, render_diagnostics};

// ── Embedded tables (ADR 0005) ──────────────────────────────────────────

/// Parser tables baked into the binary at compile time.
/// Present when `generated/parser_tables.json` existed during `cargo build`.
#[cfg(has_embedded_tables)]
const EMBEDDED_TABLES_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/parser_tables.json"));

#[cfg(not(has_embedded_tables))]
const EMBEDDED_TABLES_JSON: &str = "";

// ── CLI definition ──────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "zpl",
    version,
    about = "ZPL toolchain — parse, lint, format, and validate Zebra Programming Language files"
)]
struct Cli {
    /// Output mode: "pretty" for coloured terminal output, "json" for
    /// machine-readable JSON. Defaults to "pretty" when stdout is a TTY,
    /// "json" otherwise.
    #[arg(long, global = true, value_parser = ["pretty", "json"])]
    output: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    // ── File analysis commands (progressive: parse → check → lint) ───
    /// Parse a ZPL file and print its AST.
    Parse {
        file: String,
        /// Path to parser tables JSON. When omitted, uses tables embedded at
        /// compile time (if available) or falls back to table-less parsing.
        #[arg(long)]
        tables: Option<String>,
    },

    /// Syntax-check a ZPL file.
    SyntaxCheck {
        file: String,
        /// Path to parser tables JSON (see `parse --help`).
        #[arg(long)]
        tables: Option<String>,
    },

    /// Lint: parse and validate a ZPL file with spec tables and an optional
    /// printer profile.
    Lint {
        file: String,
        /// Path to parser tables JSON (see `parse --help`).
        #[arg(long)]
        tables: Option<String>,
        #[arg(long)]
        profile: Option<String>,
    },

    // ── File transformation ─────────────────────────────────────────
    /// Format a ZPL file (normalize whitespace, one command per line).
    Format {
        file: String,
        /// Path to parser tables JSON (see `parse --help`).
        #[arg(long)]
        tables: Option<String>,
        /// Write formatted output back to the file (in-place).
        #[arg(long, short, conflicts_with = "check")]
        write: bool,
        /// Check if the file is already formatted (exit 1 if not). For CI.
        #[arg(long, conflicts_with = "write")]
        check: bool,
        /// Indentation style.
        #[arg(long, value_enum, default_value_t = IndentStyle::None)]
        indent: IndentStyle,
    },

    // ── Reference / informational ───────────────────────────────────
    /// Show human-readable summary of generated/coverage.json.
    Coverage {
        #[arg(long, default_value = "generated/coverage.json")]
        coverage: String,
        #[arg(long)]
        show_issues: bool,
        #[arg(long)]
        json: bool,
    },

    /// Explain a diagnostic ID (e.g. ZPL1201).
    Explain { id: String },
}

/// Indentation style for the `format` command.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum IndentStyle {
    /// No indentation (flat — matches conventional ZPL style).
    None,
    /// 2-space indent for commands inside ^XA/^XZ blocks.
    Label,
    /// Label + additional 2-space indent inside ^FO...^FS field blocks.
    Field,
}

impl From<IndentStyle> for Indent {
    fn from(s: IndentStyle) -> Self {
        match s {
            IndentStyle::None => Indent::None,
            IndentStyle::Label => Indent::Label,
            IndentStyle::Field => Indent::Field,
        }
    }
}

// ── Main ────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();
    let format = Format::resolve_or_detect(cli.output.as_deref());

    match cli.cmd {
        Cmd::Parse { file, tables } => cmd_parse(&file, tables.as_deref(), format)?,
        Cmd::SyntaxCheck { file, tables } => cmd_syntax_check(&file, tables.as_deref(), format)?,
        Cmd::Lint {
            file,
            tables,
            profile,
        } => cmd_lint(&file, tables.as_deref(), profile.as_deref(), format)?,
        Cmd::Format {
            file,
            tables,
            write,
            check,
            indent,
        } => cmd_format(&file, tables.as_deref(), write, check, indent, format)?,
        Cmd::Coverage {
            coverage,
            show_issues,
            json,
        } => cmd_coverage(&coverage, show_issues, json)?,
        Cmd::Explain { id } => cmd_explain(&id, format)?,
    }

    Ok(())
}

// ── Commands ────────────────────────────────────────────────────────────

fn cmd_parse(file: &str, tables_path: Option<&str>, format: Format) -> Result<()> {
    let input = fs::read_to_string(file)?;
    let res = parse_with_resolved_tables(tables_path, &input)?;

    match format {
        Format::Json => {
            // Single valid JSON object to stdout.
            let out = serde_json::json!({
                "ast": res.ast,
                "diagnostics": res.diagnostics,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Pretty => {
            // AST to stdout, diagnostics to stderr.
            println!("{}", to_pretty_json(&res.ast));
            if !res.diagnostics.is_empty() {
                render_diagnostics(&input, file, &res.diagnostics, format);
                print_summary(&res.diagnostics);
            }
        }
    }

    exit_on_errors(&res.diagnostics);
    Ok(())
}

fn cmd_syntax_check(file: &str, tables_path: Option<&str>, format: Format) -> Result<()> {
    let input = fs::read_to_string(file)?;
    let res = parse_with_resolved_tables(tables_path, &input)?;
    let ok = !res
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Error));

    match format {
        Format::Json => {
            let out = serde_json::json!({
                "ok": ok,
                "diagnostics": res.diagnostics,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Pretty => {
            render_diagnostics(&input, file, &res.diagnostics, format);
            print_summary(&res.diagnostics);
            if ok {
                eprintln!("syntax ok");
            }
        }
    }

    exit_on_errors(&res.diagnostics);
    Ok(())
}

fn cmd_lint(
    file: &str,
    tables_path: Option<&str>,
    profile_path: Option<&str>,
    format: Format,
) -> Result<()> {
    let input = fs::read_to_string(file)?;
    let tables = resolve_tables(tables_path).context(
        "parser tables are required for lint; use --tables or rebuild with embedded tables",
    )?;
    let res = parse_with_tables(&input, Some(&tables));

    let prof = match profile_path {
        Some(p) => {
            let s = fs::read_to_string(p)?;
            Some(serde_json::from_str::<zpl_toolchain_profile::Profile>(&s)?)
        }
        None => None,
    };

    let mut vr = validate::validate_with_profile(&res.ast, &tables, prof.as_ref());
    // Merge parser diagnostics into lint surface.
    vr.issues.extend(res.diagnostics);

    match format {
        Format::Json => {
            let out = serde_json::json!({
                "ok": vr.ok,
                "issues": vr.issues,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Pretty => {
            render_diagnostics(&input, file, &vr.issues, format);
            print_summary(&vr.issues);
            if vr.ok {
                eprintln!("lint ok");
            }
        }
    }

    exit_on_errors(&vr.issues);
    Ok(())
}

fn cmd_format(
    file: &str,
    tables_path: Option<&str>,
    write: bool,
    check: bool,
    indent: IndentStyle,
    format: Format,
) -> Result<()> {
    let input = fs::read_to_string(file)?;
    let tables = resolve_tables(tables_path);
    let res = match tables.as_ref() {
        Some(t) => parse_with_tables(&input, Some(t)),
        None => parse_str(&input),
    };

    // Surface parse diagnostics so the user knows if the input has issues.
    if !res.diagnostics.is_empty() {
        render_diagnostics(&input, file, &res.diagnostics, format);
        print_summary(&res.diagnostics);
    }

    let config = EmitConfig {
        indent: indent.into(),
    };
    let formatted = emit_zpl(&res.ast, tables.as_ref(), &config);

    let already_formatted = formatted == input;

    if check {
        status_message(
            format,
            already_formatted,
            "already formatted",
            "not formatted",
            file,
        );
        if !already_formatted {
            process::exit(1);
        }
    } else if write {
        if !already_formatted {
            fs::write(file, &formatted)?;
        }
        status_message(
            format,
            !already_formatted,
            "formatted",
            "already formatted",
            file,
        );
    } else {
        // Default: print formatted output to stdout.
        print!("{}", formatted);
    }

    Ok(())
}

/// Emit a status message for --check / --write in the appropriate format.
fn status_message(format: Format, condition: bool, if_true: &str, if_false: &str, file: &str) {
    let msg = if condition { if_true } else { if_false };
    match format {
        Format::Json => {
            let out = serde_json::json!({ "status": msg, "file": file });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).expect("status JSON serialization cannot fail")
            );
        }
        Format::Pretty => {
            eprintln!("{}: {}", msg, file);
        }
    }
}

fn cmd_coverage(coverage_path: &str, show_issues: bool, json: bool) -> Result<()> {
    let text = fs::read_to_string(coverage_path)?;
    let v: serde_json::Value = serde_json::from_str(&text)?;

    let master_total = v.get("master_total").and_then(|x| x.as_u64()).unwrap_or(0);
    let present = v
        .get("present_in_spec_count")
        .and_then(|x| x.as_u64())
        .or_else(|| v.get("present_in_spec").and_then(|x| x.as_u64()))
        .unwrap_or(0);
    let missing = v
        .get("missing_in_spec_count")
        .and_then(|x| x.as_u64())
        .or_else(|| {
            v.get("missing_in_spec")
                .and_then(|x| x.as_array().map(|a| a.len() as u64))
        })
        .unwrap_or(0);
    let pct = if master_total > 0 {
        (present as f64) * 100.0 / (master_total as f64)
    } else {
        0.0
    };

    if json {
        let summary = serde_json::json!({
            "master_total": master_total,
            "present": present,
            "missing": missing,
            "percent_present": format!("{:.1}", pct),
            "with_signature": v.get("with_signature").and_then(|x| x.as_u64()).unwrap_or(0),
            "with_args": v.get("with_args").and_then(|x| x.as_u64()).unwrap_or(0),
            "with_constraints": v.get("with_constraints").and_then(|x| x.as_u64()).unwrap_or(0),
            "with_docs": v.get("with_docs").and_then(|x| x.as_u64()).unwrap_or(0),
        });
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!(
        "coverage: present={}/{} ({:.1}%) missing={}",
        present, master_total, pct, missing
    );

    let with_sig = v
        .get("with_signature")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let with_args = v.get("with_args").and_then(|x| x.as_u64()).unwrap_or(0);
    let with_cons = v
        .get("with_constraints")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let with_docs = v.get("with_docs").and_then(|x| x.as_u64()).unwrap_or(0);
    println!(
        "with: signature={} args={} constraints={} docs={}",
        with_sig, with_args, with_cons, with_docs
    );

    // Aggregate missing fields across present codes.
    if let Some(missing_by_code) = v.get("missing_by_code").and_then(|x| x.as_object()) {
        use std::collections::BTreeMap;
        let mut agg: BTreeMap<String, usize> = BTreeMap::new();
        for arr in missing_by_code.values() {
            if let Some(items) = arr.as_array() {
                for it in items {
                    if let Some(s) = it.as_str() {
                        *agg.entry(s.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
        if !agg.is_empty() {
            let mut parts: Vec<String> = agg.iter().map(|(k, v)| format!("{}:{}", k, v)).collect();
            parts.sort();
            println!("missing fields (counts): {}", parts.join(" "));
        }
    }

    if let Some(per_code) = v.get("per_code").and_then(|x| x.as_object()) {
        let mut issues: Vec<(String, Vec<String>)> = Vec::new();
        for (code, entry) in per_code.iter() {
            if let Some(arr) = entry.get("validation_errors").and_then(|x| x.as_array())
                && !arr.is_empty()
            {
                let msgs: Vec<String> = arr
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect();
                issues.push((code.clone(), msgs));
            }
        }
        if !issues.is_empty() && !json {
            println!("spec issues: {} codes with validation errors", issues.len());
            let iter: Vec<(usize, &(String, Vec<String>))> = if show_issues {
                issues.iter().enumerate().collect()
            } else {
                issues.iter().enumerate().take(5).collect()
            };
            for (i, (code, msgs)) in iter.into_iter() {
                if show_issues {
                    for msg in msgs.iter() {
                        println!("  {}. {}: {}", i + 1, code, msg);
                    }
                } else if let Some(first) = msgs.first() {
                    println!("  {}. {}: {}", i + 1, code, first);
                }
            }
            let mut counts: Vec<(&String, usize)> =
                issues.iter().map(|(c, m)| (c, m.len())).collect();
            counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            let brief: Vec<String> = counts
                .iter()
                .take(10)
                .map(|(c, k)| format!("{}:{}", c, k))
                .collect();
            println!("issues by code (top 10): {}", brief.join(" "));
            println!("tip: use 'zpl explain <ID>' to describe diagnostic IDs reported by lint.");
        }
    }

    Ok(())
}

fn cmd_explain(id: &str, format: Format) -> Result<()> {
    match format {
        Format::Json => {
            let text = diag::explain(id);
            let out = serde_json::json!({
                "id": id,
                "explanation": text,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Pretty => {
            // Explanation is the expected output — write to stdout, not stderr.
            if let Some(text) = diag::explain(id) {
                use ariadne::Fmt;
                println!("{}: {}", id.fg(ariadne::Color::Cyan), text);
            } else {
                println!("{}: (no explanation available)", id);
            }
        }
    }
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Exit with code 1 if any diagnostic is an error.
/// Warnings and info do not cause a non-zero exit.
fn exit_on_errors(diagnostics: &[Diagnostic]) {
    if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Error))
    {
        process::exit(1);
    }
}

/// Resolve parser tables from (in priority order):
///   1. Explicit `--tables` path
///   2. Embedded tables compiled into the binary
///
/// Returns `None` only if neither source is available.
fn resolve_tables(explicit_path: Option<&str>) -> Option<ParserTables> {
    // 1. Explicit file path takes priority.
    if let Some(path) = explicit_path {
        let json = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("error: failed to read tables file '{}': {}", path, e);
            std::process::exit(1);
        });
        let tables = serde_json::from_str(&json).unwrap_or_else(|e| {
            eprintln!("error: failed to parse tables file '{}': {}", path, e);
            std::process::exit(1);
        });
        return Some(tables);
    }

    // 2. Embedded tables (compiled in via build.rs).
    embedded_tables()
}

/// Return embedded tables when compiled in, `None` otherwise.
#[cfg(has_embedded_tables)]
fn embedded_tables() -> Option<ParserTables> {
    serde_json::from_str(EMBEDDED_TABLES_JSON).ok()
}

#[cfg(not(has_embedded_tables))]
fn embedded_tables() -> Option<ParserTables> {
    None
}

/// Parse input with the best available tables. Falls back to table-less
/// parsing when no tables can be resolved.
fn parse_with_resolved_tables(
    tables_path: Option<&str>,
    input: &str,
) -> Result<zpl_toolchain_core::grammar::parser::ParseResult> {
    let tables = resolve_tables(tables_path);
    Ok(match tables.as_ref() {
        Some(t) => parse_with_tables(input, Some(t)),
        None => parse_str(input),
    })
}
