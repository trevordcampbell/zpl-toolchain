//! Lightweight parse/validate/format benchmark harness for local baselines.
//!
//! Run from repository root:
//! `cargo run -p zpl_toolchain_core --example pipeline_benchmark --release`

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use zpl_toolchain_core::grammar::emit::{Compaction, EmitConfig, Indent, emit_zpl};
use zpl_toolchain_core::grammar::parser::parse_with_tables;
use zpl_toolchain_core::validate::validate;
use zpl_toolchain_spec_tables::ParserTables;

fn load_tables() -> Result<ParserTables, String> {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("generated/parser_tables.json"),
        PathBuf::from("../../generated/parser_tables.json"),
        PathBuf::from("../../../generated/parser_tables.json"),
    ];
    if let Ok(from_env) = std::env::var("ZPL_TABLES_JSON") {
        candidates.insert(0, PathBuf::from(from_env));
    }
    for path in candidates {
        if !path.exists() {
            continue;
        }
        let json = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read tables at '{}': {e}", path.display()))?;
        let tables: ParserTables = serde_json::from_str(&json)
            .map_err(|e| format!("failed to parse tables at '{}': {e}", path.display()))?;
        return Ok(tables);
    }
    Err(
        "unable to locate parser tables; set ZPL_TABLES_JSON to parser_tables.json path"
            .to_string(),
    )
}

fn run_benchmark(label: &str, input: &str, tables: &ParserTables, iterations: usize) {
    let emit_cfg = EmitConfig {
        indent: Indent::Field,
        compaction: Compaction::Field,
        ..EmitConfig::default()
    };

    let parse_start = Instant::now();
    for _ in 0..iterations {
        let _ = parse_with_tables(input, Some(tables));
    }
    let parse_elapsed = parse_start.elapsed();

    let parse_once = parse_with_tables(input, Some(tables));

    let validate_start = Instant::now();
    for _ in 0..iterations {
        let _ = validate(&parse_once.ast, tables);
    }
    let validate_elapsed = validate_start.elapsed();

    let format_start = Instant::now();
    for _ in 0..iterations {
        let _ = emit_zpl(&parse_once.ast, Some(tables), &emit_cfg);
    }
    let format_elapsed = format_start.elapsed();

    println!("Benchmark: {label}");
    println!("  input_bytes: {}", input.len());
    println!(
        "  parse:    total={:?}, per_iter={:.3} ms",
        parse_elapsed,
        parse_elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );
    println!(
        "  validate: total={:?}, per_iter={:.3} ms",
        validate_elapsed,
        validate_elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );
    println!(
        "  format:   total={:?}, per_iter={:.3} ms",
        format_elapsed,
        format_elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );
}

fn main() -> Result<(), String> {
    let tables = load_tables()?;
    let iterations = std::env::var("ZPL_BENCH_ITERS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(500);

    let sample_paths = [
        ("usps_surepost", "samples/usps_surepost_sample.zpl"),
        ("compliance", "samples/compliance_label.zpl"),
    ];

    for (label, path) in sample_paths {
        let input = fs::read_to_string(path)
            .map_err(|e| format!("failed to read sample '{}': {e}", path))?;
        run_benchmark(label, &input, &tables, iterations);
    }

    Ok(())
}
