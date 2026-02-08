//! ZPL spec compiler — validates spec files and generates parser tables.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use zpl_toolchain_spec_compiler::{SCHEMA_VERSION, pipeline, write_json_pretty};

#[derive(Parser, Debug)]
#[command(name = "zpl-spec-compiler", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Validate spec files: load, deserialize, and run cross-field checks
    Check {
        /// Spec directory containing commands/ subfolder
        #[arg(long, default_value = "spec")]
        spec_dir: PathBuf,
    },
    /// Build artifacts (parser tables, docs, coverage)
    Build {
        #[arg(long, default_value = "spec")]
        spec_dir: PathBuf,
        #[arg(long, default_value = "generated")]
        out_dir: PathBuf,
        /// Fail the build if cross-field validation produces any warnings.
        #[arg(long)]
        strict: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Check { spec_dir } => check(spec_dir)?,
        Cmd::Build {
            spec_dir,
            out_dir,
            strict,
        } => build(spec_dir, out_dir, strict)?,
    }
    Ok(())
}

/// Warn on stderr about any schema versions that don't match the expected version.
fn warn_schema_versions(versions: &std::collections::BTreeSet<String>) {
    for sv in versions {
        if sv != SCHEMA_VERSION {
            eprintln!(
                "warn: unexpected schemaVersion '{}' (expected '{}')",
                sv, SCHEMA_VERSION
            );
        }
    }
}

fn check(spec_dir: PathBuf) -> Result<()> {
    // 1. Load spec files (deserializes all JSONC into typed structs)
    let loaded = pipeline::load_spec_files(&spec_dir)?;
    eprintln!(
        "loaded {} command(s) from {:?}",
        loaded.commands.len(),
        spec_dir
    );

    // 2. Validate schema versions
    warn_schema_versions(&loaded.schema_versions);

    // 3. Cross-field validation
    let validation_errors = pipeline::validate_cross_field(&loaded.commands, &spec_dir);
    let mut issue_count = 0usize;
    for ve in &validation_errors {
        for err in &ve.errors {
            eprintln!("warn [{}]: {}", ve.code, err);
            issue_count += 1;
        }
    }

    // 4. Report summary
    let ok = validation_errors.is_empty();
    println!(
        "{}",
        serde_json::json!({
            "ok": ok,
            "commands_loaded": loaded.commands.len(),
            "schema_versions": loaded.schema_versions.iter().cloned().collect::<Vec<_>>(),
            "validation_issues": issue_count,
            "commands_with_issues": validation_errors.len(),
        })
    );

    if !ok {
        std::process::exit(1);
    }

    Ok(())
}

fn build(spec_dir: PathBuf, out_dir: PathBuf, strict: bool) -> Result<()> {
    // 1. Load spec files into typed structs
    let loaded = pipeline::load_spec_files(&spec_dir)?;

    // 2. Validate schema versions
    warn_schema_versions(&loaded.schema_versions);

    // 3. Cross-field validation (non-fatal warnings)
    let validation_errors = pipeline::validate_cross_field(&loaded.commands, &spec_dir);
    for ve in &validation_errors {
        for err in &ve.errors {
            eprintln!("warn [{}]: {}", ve.code, err);
        }
    }

    if strict && !validation_errors.is_empty() {
        anyhow::bail!(
            "strict mode: {} command(s) with validation issues",
            validation_errors.len()
        );
    }

    // 4. Load master code list
    let master_codes = pipeline::load_master_codes("docs/public/schema/zpl-commands.jsonc");

    // 5. Generate parser tables (includes opcode trie inline)
    let tables = pipeline::generate_tables(&loaded.commands, &loaded.schema_versions)?;

    // 6. Generate docs bundle (written as separate file, not embedded in parser_tables)
    let docs_bundle =
        pipeline::generate_docs_bundle(&loaded.commands, &loaded.schema_versions, &master_codes)?;
    write_json_pretty(out_dir.join("docs_bundle.json"), &docs_bundle)?;

    // 7. Generate constraints bundle
    let constraints_bundle =
        pipeline::generate_constraints_bundle(&loaded.commands, &loaded.schema_versions)?;
    write_json_pretty(out_dir.join("constraints_bundle.json"), &constraints_bundle)?;

    // 8. Generate coverage report
    let coverage = pipeline::generate_coverage(
        &loaded.commands,
        &loaded.schema_versions,
        &master_codes,
        &validation_errors,
    );
    write_json_pretty(out_dir.join("coverage.json"), &coverage)?;

    // 9. Write parser tables
    write_json_pretty(out_dir.join("parser_tables.json"), &tables)?;

    println!("{}", serde_json::json!({"ok": true}));
    Ok(())
}
