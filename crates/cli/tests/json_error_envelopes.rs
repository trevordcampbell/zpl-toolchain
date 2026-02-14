//! Ensure CLI command failures honor `--output json`.

use std::fs;
use std::process::Command;

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

#[test]
fn parse_missing_file_emits_json_error_envelope() {
    let output = zpl_cmd()
        .args(["parse", "nope-does-not-exist.zpl", "--output", "json"])
        .output()
        .expect("run parse command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json envelope");
    assert_eq!(json["success"], false);
    assert_eq!(json["error"], "command_failed");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|m| m.contains("No such file") || m.contains("failed to")),
        "unexpected message: {}",
        json["message"]
    );
}

#[test]
fn lint_invalid_tables_path_emits_json_error_envelope() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);
    let output = zpl_cmd()
        .args([
            "lint",
            &path,
            "--tables",
            "missing-parser-tables.json",
            "--output",
            "json",
        ])
        .output()
        .expect("run lint command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json envelope");
    assert_eq!(json["success"], false);
    assert_eq!(json["error"], "command_failed");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|m| m.contains("tables file")),
        "unexpected message: {}",
        json["message"]
    );
}

#[test]
fn print_serial_usb_conflict_emits_json_error_envelope() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);
    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "usb",
            "--serial",
            "--dry-run",
            "--no-lint",
            "--output",
            "json",
        ])
        .output()
        .expect("run print command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json envelope");
    assert_eq!(json["success"], false);
    assert_eq!(json["error"], "command_failed");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|m| m.contains("--serial") && m.contains("USB")),
        "unexpected message: {}",
        json["message"]
    );
}

#[test]
fn format_missing_file_emits_json_error_envelope() {
    let output = zpl_cmd()
        .args(["format", "missing-file.zpl", "--output", "json"])
        .output()
        .expect("run format command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json envelope");
    assert_eq!(json["success"], false);
    assert_eq!(json["error"], "command_failed");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|m| m.contains("No such file") || m.contains("failed to")),
        "unexpected message: {}",
        json["message"]
    );
}
