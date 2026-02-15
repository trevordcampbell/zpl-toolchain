//! CLI regression tests for aliases, stdin input, and lint JSON output contract.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use assert_cmd::cargo;

const SAMPLE_ZPL: &str = "^XA\n^FO50,50^A0N,30,30^FDHello^FS\n^XZ\n";

fn zpl_cmd() -> Command {
    Command::new(cargo::cargo_bin!("zpl"))
}

fn write_temp_zpl(content: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.zpl");
    fs::write(&path, content).expect("write temp zpl");
    (dir, path.to_string_lossy().to_string())
}

fn tables_path() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../generated/parser_tables.json");
    path.to_string_lossy().to_string()
}

fn run_with_stdin(args: &[&str], stdin_body: &str) -> std::process::Output {
    let mut child = zpl_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zpl command");

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        stdin
            .write_all(stdin_body.as_bytes())
            .expect("write stdin body");
    }

    child.wait_with_output().expect("wait for output")
}

#[test]
fn check_alias_is_available() {
    let output = zpl_cmd()
        .args(["check", "--help"])
        .output()
        .expect("run check help");
    assert!(
        output.status.success(),
        "expected check alias to be available, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn validate_alias_is_available() {
    let output = zpl_cmd()
        .args(["validate", "--help"])
        .output()
        .expect("run validate help");
    assert!(
        output.status.success(),
        "expected validate alias to be available, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn parse_supports_stdin_dash_path() {
    let output = run_with_stdin(&["parse", "-", "--output", "json"], SAMPLE_ZPL);
    assert!(
        output.status.success(),
        "parse stdin should succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid parse json");
    assert_eq!(json["ast"]["labels"].as_array().map(|v| v.len()), Some(1));
}

#[test]
fn lint_json_includes_resolved_labels() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);
    let output = zpl_cmd()
        .args([
            "lint",
            &path,
            "--tables",
            &tables_path(),
            "--output",
            "json",
        ])
        .output()
        .expect("run lint");

    assert!(
        output.status.success(),
        "lint should succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid lint json");
    assert!(
        json.get("ok").is_some(),
        "expected ok in lint json output: {stdout}"
    );
    assert!(
        json.get("diagnostics").and_then(|v| v.as_array()).is_some(),
        "expected diagnostics array in lint json output: {stdout}"
    );
    assert!(
        json.get("resolved_labels").is_some(),
        "expected resolved_labels in lint json output: {stdout}"
    );
}

#[test]
fn lint_note_audience_problem_filters_contextual_notes() {
    let (_dir, path) = write_temp_zpl("^XA\n^BY2,3,80\n^XZ\n");

    let all_output = zpl_cmd()
        .args([
            "lint",
            &path,
            "--tables",
            &tables_path(),
            "--output",
            "json",
            "--note-audience",
            "all",
        ])
        .output()
        .expect("run lint with all note audiences");
    assert!(
        all_output.status.success(),
        "lint with all note audiences should succeed, stderr={}",
        String::from_utf8_lossy(&all_output.stderr)
    );
    let all_json: serde_json::Value =
        serde_json::from_slice(&all_output.stdout).expect("valid lint json");
    let all_note_count = all_json["diagnostics"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|diag| diag.get("id").and_then(|value| value.as_str()) == Some("ZPL3001"))
                .count()
        })
        .unwrap_or(0);
    assert!(all_note_count >= 1, "expected contextual note in all mode");

    let problem_output = zpl_cmd()
        .args([
            "lint",
            &path,
            "--tables",
            &tables_path(),
            "--output",
            "json",
            "--note-audience",
            "problem",
        ])
        .output()
        .expect("run lint with problem note audience");
    assert!(
        problem_output.status.success(),
        "lint with problem note audience should succeed, stderr={}",
        String::from_utf8_lossy(&problem_output.stderr)
    );
    let problem_json: serde_json::Value =
        serde_json::from_slice(&problem_output.stdout).expect("valid lint json");
    let problem_note_count = problem_json["diagnostics"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|diag| diag.get("id").and_then(|value| value.as_str()) == Some("ZPL3001"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(
        problem_note_count, 0,
        "expected contextual notes to be filtered out in problem mode"
    );
}
