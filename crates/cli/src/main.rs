//! ZPL CLI — parse, lint, format, and validate Zebra Programming Language files.

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
#[cfg(feature = "serial")]
use zpl_toolchain_print_client::SerialPrinter;
#[cfg(feature = "tcp")]
use zpl_toolchain_print_client::TcpPrinter;
#[cfg(feature = "usb")]
use zpl_toolchain_print_client::UsbPrinter;
use zpl_toolchain_print_client::{
    PrinterConfig, StatusQuery, resolve_printer_addr, wait_for_completion,
};

use crate::render::{Format, print_summary, render_diagnostics};

// ── Embedded tables (ADR 0005) ──────────────────────────────────────────

/// Parser tables baked into the binary at compile time.
/// Present when `generated/parser_tables.json` existed during `cargo build`.
#[cfg(has_embedded_tables)]
const EMBEDDED_TABLES_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/parser_tables.json"));

// ── CLI definition ──────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "zpl",
    version,
    about = "ZPL toolchain — parse, lint, format, validate, and print Zebra Programming Language files"
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
        /// ZPL source file to parse.
        #[arg(value_name = "FILE")]
        file: String,
        /// Override the embedded parser tables with a custom JSON file.
        #[arg(long, value_name = "PATH", hide = true)]
        tables: Option<String>,
    },

    /// Syntax-check a ZPL file (parse only, no validation).
    SyntaxCheck {
        /// ZPL source file to check.
        #[arg(value_name = "FILE")]
        file: String,
        /// Override the embedded parser tables with a custom JSON file.
        #[arg(long, value_name = "PATH", hide = true)]
        tables: Option<String>,
    },

    /// Lint: parse and validate a ZPL file against the spec and an optional
    /// printer profile.
    Lint {
        /// ZPL source file to lint.
        #[arg(value_name = "FILE")]
        file: String,
        /// Override the embedded parser tables with a custom JSON file.
        #[arg(long, value_name = "PATH", hide = true)]
        tables: Option<String>,
        /// Printer profile JSON for hardware-specific validation (see profiles/).
        #[arg(long, value_name = "PATH")]
        profile: Option<String>,
    },

    // ── File transformation ─────────────────────────────────────────
    /// Format a ZPL file (normalize whitespace, one command per line).
    Format {
        /// ZPL source file to format.
        #[arg(value_name = "FILE")]
        file: String,
        /// Override the embedded parser tables with a custom JSON file.
        #[arg(long, value_name = "PATH", hide = true)]
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

    // ── Printing ─────────────────────────────────────────────────────
    /// Send a ZPL file to a printer. Validates first (unless --no-lint).
    Print {
        /// ZPL file(s) to print.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<String>,
        /// Printer address (IP/hostname, port defaults to 9100).
        #[arg(long, short)]
        printer: String,
        /// Printer profile JSON for hardware-specific validation (see profiles/).
        #[arg(long, value_name = "PATH")]
        profile: Option<String>,
        /// Override the embedded parser tables with a custom JSON file.
        #[arg(long, value_name = "PATH", hide = true)]
        tables: Option<String>,
        /// Skip validation and send raw ZPL directly.
        #[arg(long)]
        no_lint: bool,
        /// Treat warnings as errors (abort printing on warnings).
        #[arg(long)]
        strict: bool,
        /// Validate and resolve address, but don't actually send.
        #[arg(long)]
        dry_run: bool,
        /// Query printer status (~HS) after sending.
        #[arg(long)]
        status: bool,
        /// Query printer info (~HI) and display model/firmware/DPI/memory.
        #[arg(long)]
        info: bool,
        /// Wait for printer to finish processing all labels.
        #[arg(long)]
        wait: bool,
        /// Connection timeout in seconds (scales connect/write/read proportionally).
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        timeout: Option<u64>,
        /// Timeout in seconds for --wait polling (default 120s).
        #[arg(long, default_value_t = 120, requires = "wait")]
        wait_timeout: u64,
        /// Use serial/Bluetooth SPP transport (printer address is a serial port path).
        #[cfg(feature = "serial")]
        #[arg(long)]
        serial: bool,
        /// Baud rate for serial connections (default: 9600).
        #[cfg(feature = "serial")]
        #[arg(long, default_value_t = 9600, requires = "serial")]
        baud: u32,
    },

    // ── Reference / informational ───────────────────────────────────
    /// Show spec coverage summary (developer tool — requires generated/coverage.json).
    #[command(hide = true)]
    Coverage {
        /// Path to coverage JSON file.
        #[arg(long, value_name = "PATH", default_value = "generated/coverage.json")]
        coverage: String,
        /// Show all issues (not just top 5).
        #[arg(long)]
        show_issues: bool,
        /// Output as JSON.
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
        Cmd::Print {
            files,
            printer,
            profile,
            tables,
            no_lint,
            strict,
            dry_run,
            status,
            info,
            wait,
            timeout,
            wait_timeout,
            #[cfg(feature = "serial")]
            serial,
            #[cfg(feature = "serial")]
            baud,
        } => cmd_print(PrintOpts {
            files: &files,
            printer_addr: &printer,
            profile_path: profile.as_deref(),
            tables_path: tables.as_deref(),
            no_lint,
            strict,
            dry_run,
            status,
            info,
            wait,
            timeout,
            wait_timeout,
            #[cfg(feature = "serial")]
            serial,
            #[cfg(feature = "serial")]
            baud,
            format,
        })?,
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
        "no parser tables available — this binary was built without embedded tables. \
         Download a release build from https://github.com/trevordcampbell/zpl-toolchain/releases, \
         reinstall via `cargo install zpl_toolchain_cli`, or pass --tables <PATH> to a tables JSON file",
    )?;
    let res = parse_with_tables(&input, Some(&tables));

    let prof = match profile_path {
        Some(p) => {
            let s =
                fs::read_to_string(p).with_context(|| format!("failed to read profile '{}'", p))?;
            Some(
                serde_json::from_str::<zpl_toolchain_profile::Profile>(&s)
                    .with_context(|| format!("failed to parse profile '{}'", p))?,
            )
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

/// Bundled options for the `print` subcommand.
struct PrintOpts<'a> {
    files: &'a [String],
    printer_addr: &'a str,
    profile_path: Option<&'a str>,
    tables_path: Option<&'a str>,
    no_lint: bool,
    strict: bool,
    dry_run: bool,
    status: bool,
    info: bool,
    wait: bool,
    timeout: Option<u64>,
    wait_timeout: u64,
    #[cfg(feature = "serial")]
    serial: bool,
    #[cfg(feature = "serial")]
    baud: u32,
    format: Format,
}

fn cmd_print(opts: PrintOpts<'_>) -> Result<()> {
    use std::time::Duration;

    let PrintOpts {
        files,
        printer_addr,
        profile_path,
        tables_path,
        no_lint,
        strict,
        dry_run,
        status,
        info,
        wait,
        timeout,
        wait_timeout,
        #[cfg(feature = "serial")]
        serial,
        #[cfg(feature = "serial")]
        baud,
        format,
    } = opts;

    // ── Read all files ──────────────────────────────────────────────
    let mut file_contents: Vec<(String, String)> = Vec::new();
    for path in files {
        let content =
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path))?;
        file_contents.push((path.clone(), content));
    }

    // ── Validate (unless --no-lint) ─────────────────────────────────
    let mut all_diagnostics: Vec<Diagnostic> = Vec::new();

    if !no_lint {
        let tables = resolve_tables(tables_path).context(
            "no parser tables available for pre-print validation — pass --no-lint to skip, \
             or reinstall via `cargo install zpl_toolchain_cli` which includes embedded tables",
        )?;

        let prof = match profile_path {
            Some(p) => {
                let s = fs::read_to_string(p)
                    .with_context(|| format!("failed to read profile '{}'", p))?;
                Some(
                    serde_json::from_str::<zpl_toolchain_profile::Profile>(&s)
                        .with_context(|| format!("failed to parse profile '{}'", p))?,
                )
            }
            None => None,
        };

        let mut has_errors = false;
        let mut has_warnings = false;

        for (path, content) in &file_contents {
            let res = parse_with_tables(content, Some(&tables));
            let mut vr = validate::validate_with_profile(&res.ast, &tables, prof.as_ref());
            vr.issues.extend(res.diagnostics);

            if format == Format::Pretty && !vr.issues.is_empty() {
                render_diagnostics(content, path, &vr.issues, format);
                print_summary(&vr.issues);
            }

            if vr
                .issues
                .iter()
                .any(|d| matches!(d.severity, Severity::Error))
            {
                has_errors = true;
            }
            if vr
                .issues
                .iter()
                .any(|d| matches!(d.severity, Severity::Warn))
            {
                has_warnings = true;
            }

            all_diagnostics.extend(vr.issues);
        }

        if has_errors {
            if format == Format::Json {
                let out = serde_json::json!({
                    "error": "validation_failed",
                    "message": "aborting print due to validation errors",
                    "diagnostics": all_diagnostics,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                eprintln!("error: aborting print due to validation errors");
            }
            process::exit(1);
        }
        if strict && has_warnings {
            if format == Format::Json {
                let out = serde_json::json!({
                    "error": "validation_warnings",
                    "message": "aborting print due to warnings (--strict)",
                    "diagnostics": all_diagnostics,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                eprintln!("error: aborting print due to warnings (--strict)");
            }
            process::exit(1);
        }

        // Note: all_diagnostics (warnings) are included in the final result JSON below.
    }

    // ── Dry run: resolve address and report ─────────────────────────
    if dry_run {
        // Determine transport and display address for dry-run output.
        #[cfg(feature = "serial")]
        let is_serial = serial;
        #[cfg(not(feature = "serial"))]
        let is_serial = false;

        let is_usb_addr = printer_addr == "usb" || printer_addr.starts_with("usb:");

        // Reject --serial with USB address (matches live-print validation).
        #[cfg(feature = "serial")]
        if is_serial && is_usb_addr {
            anyhow::bail!(
                "--serial cannot be used with USB printer address '{}'",
                printer_addr
            );
        }

        let (transport, display_addr) = if is_serial {
            ("serial", printer_addr.to_string())
        } else if is_usb_addr {
            #[cfg(not(feature = "usb"))]
            anyhow::bail!(
                "USB transport not available — this binary was compiled without USB support. \
                 Reinstall with default features: cargo install zpl_toolchain_cli"
            );
            #[cfg(feature = "usb")]
            if printer_addr == "usb" {
                ("usb", "usb (auto-discover Zebra)".to_string())
            } else {
                ("usb", printer_addr.to_string())
            }
        } else if looks_like_serial_port(printer_addr) {
            #[cfg(feature = "serial")]
            anyhow::bail!(
                "'{}' looks like a serial port — add --serial to use serial transport.\n  \
                 Example: zpl print <FILE> -p {} --serial",
                printer_addr,
                printer_addr
            );
            #[cfg(not(feature = "serial"))]
            anyhow::bail!(
                "'{}' looks like a serial port, but this binary was compiled without serial support. \
                 Reinstall with default features: cargo install zpl_toolchain_cli",
                printer_addr
            );
        } else {
            // TCP: resolve to verify the address is valid.
            let resolved = resolve_printer_addr(printer_addr).map_err(|e| {
                anyhow::anyhow!(
                    "failed to resolve printer address '{}': {}",
                    printer_addr,
                    e
                )
            })?;
            ("tcp", resolved.to_string())
        };

        match format {
            Format::Json => {
                let mut out = serde_json::json!({
                    "dry_run": true,
                    "transport": transport,
                    "resolved_address": display_addr,
                    "files": files,
                    "validation": if no_lint { "skipped" } else { "passed" },
                });
                if !all_diagnostics.is_empty() {
                    out["diagnostics"] = serde_json::to_value(all_diagnostics).unwrap_or_default();
                }
                println!("{}", serde_json::to_string_pretty(&out)?);
            }
            Format::Pretty => {
                eprintln!(
                    "dry run: would print {} file(s) to {} ({})",
                    files.len(),
                    display_addr,
                    transport,
                );
                for (path, _) in &file_contents {
                    eprintln!("  {}", path);
                }
                if no_lint {
                    eprintln!("  validation: skipped (--no-lint)");
                } else {
                    eprintln!("  validation: passed");
                }
            }
        }
        return Ok(());
    }

    // ── Build printer config ────────────────────────────────────────
    let config = if let Some(secs) = timeout {
        let base = Duration::from_secs(secs);
        let mut cfg = PrinterConfig::default();
        cfg.timeouts.connect = base;
        cfg.timeouts.write = base.mul_f64(6.0); // 6× connect
        cfg.timeouts.read = base.mul_f64(2.0); // 2× connect
        cfg
    } else {
        PrinterConfig::default()
    };

    // ── Connect and run print session ─────────────────────────────
    let connection_err = |e: zpl_toolchain_print_client::PrintError| {
        if format == Format::Json {
            let out = serde_json::json!({
                "error": "connection_failed",
                "message": format!("failed to connect to printer '{}': {}", printer_addr, e),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).expect("JSON serialization cannot fail")
            );
            process::exit(1);
        }
        anyhow::anyhow!("failed to connect to printer '{}': {}", printer_addr, e)
    };

    let session = SessionOpts {
        file_contents: &file_contents,
        all_diagnostics: &all_diagnostics,
        info,
        status,
        wait,
        wait_timeout,
        format,
    };

    // ── Serial transport ──────────────────────────────────────────
    #[cfg(feature = "serial")]
    if serial && (printer_addr == "usb" || printer_addr.starts_with("usb:")) {
        anyhow::bail!(
            "--serial cannot be used with USB printer address '{}'",
            printer_addr
        );
    }

    #[cfg(feature = "serial")]
    if serial {
        let mut printer =
            SerialPrinter::open(printer_addr, baud, config).map_err(connection_err)?;
        if format == Format::Pretty {
            eprintln!("connected to {} (serial, {} baud)", printer_addr, baud);
        }
        return run_print_session(&mut printer, printer_addr, &session);
    }

    // ── USB transport ────────────────────────────────────────────
    #[cfg(feature = "usb")]
    if printer_addr == "usb" {
        let mut printer = UsbPrinter::find_zebra(config).map_err(connection_err)?;
        if format == Format::Pretty {
            eprintln!("connected to USB Zebra printer");
        }
        return run_print_session(&mut printer, "usb", &session);
    }

    #[cfg(feature = "usb")]
    if let Some(vidpid) = printer_addr.strip_prefix("usb:") {
        let (vid, pid) = parse_usb_vidpid(vidpid)?;
        let mut printer = UsbPrinter::find(vid, pid, config).map_err(connection_err)?;
        if format == Format::Pretty {
            eprintln!("connected to USB printer {:04X}:{:04X}", vid, pid);
        }
        return run_print_session(&mut printer, printer_addr, &session);
    }

    #[cfg(not(feature = "usb"))]
    if printer_addr == "usb" || printer_addr.starts_with("usb:") {
        anyhow::bail!(
            "USB transport not available — this binary was compiled without USB support. \
             Reinstall with default features: cargo install zpl_toolchain_cli"
        );
    }

    // ── Detect likely serial port paths before falling through to TCP ─
    if looks_like_serial_port(printer_addr) {
        #[cfg(feature = "serial")]
        anyhow::bail!(
            "'{}' looks like a serial port — add --serial to use serial transport.\n  \
             Example: zpl print <FILE> -p {} --serial",
            printer_addr,
            printer_addr
        );
        #[cfg(not(feature = "serial"))]
        anyhow::bail!(
            "'{}' looks like a serial port, but this binary was compiled without serial support. \
             Reinstall with default features: cargo install zpl_toolchain_cli",
            printer_addr
        );
    }

    // ── TCP transport (default) ──────────────────────────────────
    {
        let mut printer = TcpPrinter::connect(printer_addr, config).map_err(connection_err)?;
        let remote = printer.remote_addr();
        if format == Format::Pretty {
            eprintln!("connected to {}", remote);
        }
        run_print_session(&mut printer, &remote.to_string(), &session)
    }
}

/// Parse a USB VID:PID string like "0A5F:0100".
#[cfg(feature = "usb")]
fn parse_usb_vidpid(s: &str) -> Result<(u16, u16)> {
    let (v, p) = s
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid USB address '{}': expected usb:VID:PID", s))?;
    let vid =
        u16::from_str_radix(v, 16).with_context(|| format!("invalid USB vendor ID '{}'", v))?;
    let pid =
        u16::from_str_radix(p, 16).with_context(|| format!("invalid USB product ID '{}'", p))?;
    Ok((vid, pid))
}

/// Options passed to the transport-agnostic print session.
struct SessionOpts<'a> {
    file_contents: &'a [(String, String)],
    all_diagnostics: &'a [Diagnostic],
    info: bool,
    status: bool,
    wait: bool,
    wait_timeout: u64,
    format: Format,
}

/// Run the print session (info → send → status → wait → result).
///
/// Generic over any transport that implements both [`Printer`] and [`StatusQuery`].
fn run_print_session<P: StatusQuery>(
    printer: &mut P,
    printer_display: &str,
    opts: &SessionOpts<'_>,
) -> Result<()> {
    use std::time::Duration;

    let SessionOpts {
        file_contents,
        all_diagnostics,
        info,
        status,
        wait,
        wait_timeout,
        format,
    } = *opts;

    // Accumulate JSON data into a single envelope for `--output json`.
    let mut json_result = serde_json::json!({
        "success": true,
        "files_sent": file_contents.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
        "printer": printer_display,
    });

    // ── Pre-send: printer info query ────────────────────────────────
    if info {
        match printer.query_info() {
            Ok(pi) => {
                if format == Format::Pretty {
                    eprintln!("printer info:");
                    eprintln!("  model:    {}", pi.model);
                    eprintln!("  firmware: {}", pi.firmware);
                    eprintln!("  dpi:      {}", pi.dpi);
                    eprintln!("  memory:   {} KB", pi.memory_kb);
                }
                if format == Format::Json {
                    json_result["printer_info"] = serde_json::to_value(&pi).unwrap_or_default();
                }
            }
            Err(e) => {
                eprintln!("warning: failed to query printer info: {}", e);
            }
        }
    }

    // ── Send each file ──────────────────────────────────────────────
    let mut files_sent: Vec<&str> = Vec::new();
    for (path, content) in file_contents {
        if let Err(e) = printer.send_zpl(content) {
            if format == Format::Json {
                let out = serde_json::json!({
                    "error": "send_failed",
                    "message": format!("failed to send '{}': {}", path, e),
                    "file": path,
                    "files_sent": files_sent,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out).expect("JSON serialization cannot fail")
                );
                process::exit(1);
            }
            return Err(anyhow::anyhow!("failed to send '{}': {}", path, e));
        }
        files_sent.push(path);
        if format == Format::Pretty {
            eprintln!("sent: {}", path);
        }
    }

    // ── Post-send: status query ─────────────────────────────────────
    if status {
        match printer.query_status() {
            Ok(hs) => {
                if format == Format::Pretty {
                    use ariadne::Fmt;

                    eprintln!("printer status:");
                    eprintln!("  mode:             {:?}", hs.print_mode);
                    eprintln!("  labels remaining: {}", hs.labels_remaining);
                    eprintln!("  formats queued:   {}", hs.formats_in_buffer);
                    eprintln!("  label length:     {} dots", hs.label_length_dots);

                    let mut alerts: Vec<String> = Vec::new();
                    if hs.paper_out {
                        alerts.push(format!("{}", "paper_out".fg(ariadne::Color::Red)));
                    }
                    if hs.ribbon_out {
                        alerts.push(format!("{}", "ribbon_out".fg(ariadne::Color::Red)));
                    }
                    if hs.head_up {
                        alerts.push(format!("{}", "head_up".fg(ariadne::Color::Red)));
                    }
                    if hs.paused {
                        alerts.push(format!("{}", "paused".fg(ariadne::Color::Yellow)));
                    }
                    if hs.over_temperature {
                        alerts.push(format!("{}", "over_temp".fg(ariadne::Color::Red)));
                    }
                    if hs.under_temperature {
                        alerts.push(format!("{}", "under_temp".fg(ariadne::Color::Yellow)));
                    }
                    if hs.corrupt_ram {
                        alerts.push(format!("{}", "corrupt_ram".fg(ariadne::Color::Red)));
                    }
                    if hs.buffer_full {
                        alerts.push(format!("{}", "buffer_full".fg(ariadne::Color::Yellow)));
                    }
                    if !alerts.is_empty() {
                        eprintln!("  alerts:           {}", alerts.join(", "));
                    }
                }
                if format == Format::Json {
                    json_result["printer_status"] = serde_json::to_value(&hs).unwrap_or_default();
                }
            }
            Err(e) => {
                eprintln!("warning: failed to query printer status: {}", e);
            }
        }
    }

    // ── Post-send: wait for completion ──────────────────────────────
    if wait {
        let poll_interval = Duration::from_millis(500);
        let wt = Duration::from_secs(wait_timeout);
        if format == Format::Pretty {
            eprintln!("waiting for printer to finish...");
        }
        match wait_for_completion(printer, poll_interval, wt) {
            Ok(()) => {
                if format == Format::Pretty {
                    eprintln!("printer finished");
                }
            }
            Err(e) => {
                if format == Format::Json {
                    json_result["success"] = serde_json::json!(false);
                    json_result["error"] = serde_json::json!("wait_timeout");
                    json_result["message"] =
                        serde_json::json!(format!("wait for completion failed: {}", e));
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json_result)
                            .expect("JSON serialization cannot fail")
                    );
                } else {
                    eprintln!("error: wait for completion failed: {}", e);
                }
                process::exit(1);
            }
        }
    }

    // ── Final result ────────────────────────────────────────────────
    match format {
        Format::Json => {
            if !all_diagnostics.is_empty() {
                json_result["diagnostics"] =
                    serde_json::to_value(all_diagnostics).unwrap_or_default();
            }
            println!("{}", serde_json::to_string_pretty(&json_result)?);
        }
        Format::Pretty => {
            eprintln!(
                "print complete: {} file(s) sent to {}",
                file_contents.len(),
                printer_display
            );
        }
    }
    Ok(())
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

/// Detect printer address strings that look like serial port paths.
///
/// Catches common patterns across platforms so the CLI can suggest
/// `--serial` instead of letting TCP resolution produce a confusing error.
fn looks_like_serial_port(addr: &str) -> bool {
    // Linux: /dev/ttyUSB0, /dev/ttyACM0, /dev/ttyS0, /dev/ttyAMA0, /dev/rfcomm0,
    //        /dev/serial/by-id/*, /dev/serial/by-path/*
    // macOS: /dev/tty.*, /dev/cu.*
    // Windows: COM1, COM3, COM10, etc.
    addr.starts_with("/dev/tty")
        || addr.starts_with("/dev/cu.")
        || addr.starts_with("/dev/rfcomm")
        || addr.starts_with("/dev/serial/")
        || (addr.len() >= 4
            && addr.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("COM"))
            && addr[3..].chars().all(|c| c.is_ascii_digit()))
}
