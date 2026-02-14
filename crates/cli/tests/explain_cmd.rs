//! CLI tests for the `zpl explain` subcommand.

use std::process::Command;

use assert_cmd::cargo;

fn zpl_cmd() -> Command {
    Command::new(cargo::cargo_bin!("zpl"))
}

#[test]
fn explain_known_code_json_returns_explanation() {
    let output = zpl_cmd()
        .args(["explain", "ZPL1201", "--output", "json"])
        .output()
        .expect("run explain command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(json["id"], "ZPL1201");
    assert!(json["explanation"].is_string());
}

#[test]
fn explain_unknown_code_json_returns_null_explanation() {
    let output = zpl_cmd()
        .args(["explain", "ZPL9999", "--output", "json"])
        .output()
        .expect("run explain command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(json["id"], "ZPL9999");
    assert!(json["explanation"].is_null());
}

#[test]
fn explain_pretty_shows_human_readable_text() {
    let output = zpl_cmd()
        .args(["explain", "ZPL1201", "--output", "pretty"])
        .output()
        .expect("run explain command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ZPL1201") && stdout.contains(':'),
        "unexpected output: {stdout}"
    );
}
