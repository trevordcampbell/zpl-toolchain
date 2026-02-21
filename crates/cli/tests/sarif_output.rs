//! CLI tests for SARIF 2.1.0 output.

use std::fs;
use std::process::Command;

use assert_cmd::cargo;

fn zpl_cmd() -> Command {
    Command::new(cargo::cargo_bin!("zpl"))
}

fn tables_path() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../generated/parser_tables.json");
    path.to_string_lossy().to_string()
}

fn write_temp_zpl(content: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.zpl");
    fs::write(&path, content).expect("write temp zpl");
    (dir, path.to_string_lossy().to_string())
}

#[test]
fn lint_sarif_output_shape() {
    // Valid ZPL - produces empty results
    let (_dir, path) = write_temp_zpl("^XA\n^FO50,50^A0N,30,30^FDHello^FS\n^XZ\n");
    let output = zpl_cmd()
        .args([
            "lint",
            &path,
            "--tables",
            &tables_path(),
            "--output",
            "sarif",
        ])
        .output()
        .expect("run lint");

    assert!(
        output.status.success(),
        "lint --output sarif should succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sarif: serde_json::Value =
        serde_json::from_str(&stdout).expect("lint SARIF output must be valid JSON");

    assert_eq!(sarif["version"].as_str(), Some("2.1.0"));
    assert!(sarif["$schema"].as_str().is_some());
    assert!(sarif["$schema"].as_str().unwrap().contains("sarif"));
    assert!(sarif["runs"].as_array().is_some());
    let runs = sarif["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert!(run["tool"]["driver"]["name"].as_str().is_some());
    assert_eq!(
        run["tool"]["driver"]["name"].as_str(),
        Some("zpl-toolchain")
    );
    assert!(run["tool"]["driver"]["version"].as_str().is_some());
    assert!(run["results"].as_array().is_some());
    assert!(run["artifacts"].as_array().is_some());
}

#[test]
fn lint_sarif_with_diagnostics_maps_fields() {
    // Invalid ZPL - unclosed field, empty field data; triggers parser/validator diagnostics
    let (_dir, path) = write_temp_zpl("^XA\n^FO0,0^A0N,30,30^FD\n^XZ\n");
    let output = zpl_cmd()
        .args([
            "lint",
            &path,
            "--tables",
            &tables_path(),
            "--output",
            "sarif",
        ])
        .output()
        .expect("run lint");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sarif: serde_json::Value =
        serde_json::from_str(&stdout).expect("lint SARIF output must be valid JSON");

    let results = sarif["runs"][0]["results"]
        .as_array()
        .expect("results must be array");
    assert!(
        !results.is_empty(),
        "invalid ZPL should produce at least one SARIF result"
    );

    let result = &results[0];
    assert!(result["ruleId"].as_str().is_some());
    assert!(result["message"]["text"].as_str().is_some());
    let level = result["level"].as_str().unwrap();
    assert!(
        ["error", "warning", "note"].contains(&level),
        "level must be error, warning, or note, got {level}"
    );

    if let Some(locs) = result["locations"].as_array() {
        if !locs.is_empty() {
            let loc = &locs[0];
            let phys = &loc["physicalLocation"];
            assert!(phys["artifactLocation"]["uri"].as_str().is_some());
            if let Some(region) = phys.get("region") {
                assert!(
                    region["startLine"].as_u64().is_some()
                        || region["byteOffset"].as_u64().is_some(),
                    "region must have startLine or byteOffset"
                );
            }
        }
    }
}

#[test]
fn syntax_check_sarif_output() {
    let (_dir, path) = write_temp_zpl("^XA\n^XZ\n");
    let output = zpl_cmd()
        .args([
            "syntax-check",
            &path,
            "--tables",
            &tables_path(),
            "--output",
            "sarif",
        ])
        .output()
        .expect("run syntax-check");

    assert!(
        output.status.success(),
        "syntax-check --output sarif should succeed"
    );
    let sarif: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("valid SARIF JSON");
    assert_eq!(sarif["version"].as_str(), Some("2.1.0"));
}
