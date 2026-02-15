//! CLI tests for the `zpl format` subcommand.

use std::fs;
use std::process::Command;

use assert_cmd::cargo;

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

#[test]
fn format_help_shows_compaction_flag() {
    let output = zpl_cmd()
        .args(["format", "--help"])
        .output()
        .expect("run format help");
    assert!(
        output.status.success(),
        "expected format help to succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--compaction"),
        "missing --compaction in format help output: {stdout}"
    );
    assert!(
        stdout.contains("--comment-placement"),
        "missing --comment-placement in format help output: {stdout}"
    );
}

#[test]
fn format_check_json_reports_not_formatted_with_field_compaction() {
    // Expanded line-oriented field block; compaction=field should transform this.
    let input = "^XA\n^PW609\n^LL406\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n";
    let (_dir, path) = write_temp_zpl(input);

    let output = zpl_cmd()
        .args([
            "format",
            &path,
            "--tables",
            &tables_path(),
            "--indent",
            "none",
            "--compaction",
            "field",
            "--check",
            "--output",
            "json",
        ])
        .output()
        .expect("run format --check json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected check mode to exit 1 for non-formatted input, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid format json");
    assert_eq!(json["mode"], "check");
    assert_eq!(json["file"], path);
    assert_eq!(json["already_formatted"], false);
    assert_eq!(json["status"], "not formatted");
    assert!(
        json["diagnostics"].is_array(),
        "expected diagnostics array in format check json: {stdout}"
    );
}

#[test]
fn format_write_json_rewrites_file_with_field_compaction() {
    let input = "^XA\n^PW609\n^LL406\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n";
    let (_dir, path) = write_temp_zpl(input);

    let output = zpl_cmd()
        .args([
            "format",
            &path,
            "--tables",
            &tables_path(),
            "--indent",
            "none",
            "--compaction",
            "field",
            "--write",
            "--output",
            "json",
        ])
        .output()
        .expect("run format --write json");

    assert!(
        output.status.success(),
        "expected write mode to succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid format json");
    assert_eq!(json["mode"], "write");
    assert_eq!(json["file"], path);
    assert_eq!(json["changed"], true);
    assert_eq!(json["status"], "formatted");

    let rewritten = fs::read_to_string(&path).expect("read rewritten file");
    assert!(
        rewritten.contains("^FO30,30^A0N,35,35^FDWIDGET-3000^FS"),
        "expected compacted field block in rewritten file, got:\n{rewritten}"
    );
}

#[test]
fn format_stdout_json_contains_compacted_output() {
    let input = "^XA\n^PW609\n^LL406\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n";
    let (_dir, path) = write_temp_zpl(input);

    let output = zpl_cmd()
        .args([
            "format",
            &path,
            "--tables",
            &tables_path(),
            "--indent",
            "none",
            "--compaction",
            "field",
            "--output",
            "json",
        ])
        .output()
        .expect("run format stdout json");

    assert!(
        output.status.success(),
        "expected stdout json mode to succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid format json");
    assert_eq!(json["mode"], "stdout");
    let formatted = json["formatted"]
        .as_str()
        .expect("formatted string in json output");
    assert!(
        formatted.contains("^FO30,30^A0N,35,35^FDWIDGET-3000^FS"),
        "expected compacted field block in formatted payload, got:\n{formatted}"
    );
}

#[test]
fn format_compaction_applies_with_label_indent_and_preserves_indent() {
    let input = "^XA\n^PW609\n^LL406\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n";
    let (_dir, path) = write_temp_zpl(input);

    let output = zpl_cmd()
        .args([
            "format",
            &path,
            "--tables",
            &tables_path(),
            "--indent",
            "label",
            "--compaction",
            "field",
            "--output",
            "json",
        ])
        .output()
        .expect("run format with label indent + field compaction");

    assert!(
        output.status.success(),
        "expected format to succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid format json");
    let formatted = json["formatted"]
        .as_str()
        .expect("formatted string in json output");

    assert!(
        formatted
            .lines()
            .any(|line| line == "  ^FO30,30^A0N,35,35^FDWIDGET-3000^FS"),
        "expected compacted field line to preserve label indentation, got:\n{formatted}"
    );
}

#[test]
fn format_defaults_to_inline_semicolon_comments() {
    let input = "^XA\n^PW812\n; set print width\n^XZ\n";
    let (_dir, path) = write_temp_zpl(input);

    let output = zpl_cmd()
        .args(["format", &path, "--tables", &tables_path()])
        .output()
        .expect("run format with defaults");

    assert!(
        output.status.success(),
        "expected format to succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("^PW812 ; set print width"),
        "expected default formatter to keep comments inline, got:\n{stdout}"
    );
}

#[test]
fn format_comment_placement_line_preserves_standalone_comment_lines() {
    let input = "^XA\n^PW812\n; set print width\n^XZ\n";
    let (_dir, path) = write_temp_zpl(input);
    let output = zpl_cmd()
        .args([
            "format",
            &path,
            "--tables",
            &tables_path(),
            "--comment-placement",
            "line",
        ])
        .output()
        .expect("run format with comment placement line");
    assert!(
        output.status.success(),
        "expected format to succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let formatted = if stdout.trim_start().starts_with('{') {
        serde_json::from_str::<serde_json::Value>(&stdout).expect("valid format json")["formatted"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else {
        stdout.to_string()
    };
    assert!(
        formatted.contains("^PW812\n; set print width"),
        "expected line comment placement to preserve standalone line, got:\n{formatted}"
    );
}
