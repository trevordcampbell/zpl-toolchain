//! CLI tests for the `zpl doctor` subcommand.

use std::fs;
use std::process::Command;

use assert_cmd::cargo;

fn zpl_cmd() -> Command {
    Command::new(cargo::cargo_bin!("zpl"))
}

#[test]
fn doctor_help_shows_flags() {
    let output = zpl_cmd()
        .args(["doctor", "--help"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--printer"), "missing --printer");
    assert!(stdout.contains("--profile"), "missing --profile");
    assert!(stdout.contains("--output"), "missing --output");
    assert!(stdout.contains("--timeout"), "missing --timeout");
}

#[test]
fn doctor_json_includes_tables_check() {
    let output = zpl_cmd()
        .args(["doctor", "--output", "json"])
        .output()
        .expect("run doctor command");

    assert!(
        output.status.success(),
        "doctor should succeed, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid doctor json");
    assert!(json.get("success").is_some(), "missing success field");
    assert!(json.get("tables").is_some(), "missing tables field");
    assert!(
        json["tables"].get("ok").is_some(),
        "missing tables.ok field"
    );
}

#[test]
fn doctor_invalid_profile_emits_json_error_envelope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile_path = dir.path().join("bad-profile.json");
    fs::write(&profile_path, "{ not json").expect("write profile fixture");

    let output = zpl_cmd()
        .args([
            "doctor",
            "--profile",
            &profile_path.to_string_lossy(),
            "--output",
            "json",
        ])
        .output()
        .expect("run doctor command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid doctor json");
    assert_eq!(json["success"], false);
    assert_eq!(json["profile"]["ok"], false);
    assert!(
        json["profile"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("failed to parse/validate profile")),
        "unexpected message: {}",
        json["profile"]["message"]
    );
}

#[test]
fn doctor_missing_profile_emits_json_error_envelope() {
    let output = zpl_cmd()
        .args([
            "doctor",
            "--profile",
            "nope-does-not-exist-profile.json",
            "--output",
            "json",
        ])
        .output()
        .expect("run doctor command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid doctor json");
    assert_eq!(json["success"], false);
    assert_eq!(json["profile"]["ok"], false);
    assert!(
        json["profile"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("failed to read profile")),
        "unexpected message: {}",
        json["profile"]["message"]
    );
}

#[test]
fn doctor_invalid_tables_emits_doctor_json_shape() {
    let output = zpl_cmd()
        .args([
            "doctor",
            "--tables",
            "nope-does-not-exist-tables.json",
            "--output",
            "json",
        ])
        .output()
        .expect("run doctor command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid doctor json");
    assert_eq!(json["success"], false);
    assert_eq!(json["tables"]["ok"], false);
    assert!(
        json["tables"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("tables file")),
        "unexpected message: {}",
        json["tables"]["message"]
    );
}

#[test]
fn doctor_invalid_profile_emits_sarif_result() {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile_path = dir.path().join("bad-profile.json");
    fs::write(&profile_path, "{ not json").expect("write profile fixture");

    let output = zpl_cmd()
        .args([
            "doctor",
            "--profile",
            &profile_path.to_string_lossy(),
            "--output",
            "sarif",
        ])
        .output()
        .expect("run doctor command");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let sarif: serde_json::Value = serde_json::from_str(&stdout).expect("valid sarif json");
    assert_eq!(sarif["version"], "2.1.0");
    let results = sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results array");
    assert!(
        results.iter().any(|result| {
            result["ruleId"] == "DOCTOR_PROFILE_INVALID"
                && result["message"]["text"]
                    .as_str()
                    .is_some_and(|m| m.contains("failed to parse/validate profile"))
        }),
        "expected profile failure SARIF result, got: {}",
        sarif
    );
}
