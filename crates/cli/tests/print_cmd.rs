//! CLI tests for the `zpl print` subcommand.

use std::fs;
use std::process::Command;

use assert_cmd::cargo;

// Helper to create a temp ZPL file and return its path
fn write_temp_zpl(content: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.zpl");
    fs::write(&path, content).unwrap();
    (dir, path.to_string_lossy().to_string())
}

fn zpl_cmd() -> Command {
    Command::new(cargo::cargo_bin!("zpl"))
}

const SAMPLE_ZPL: &str = "^XA\n^FO50,50^A0N,50,50^FDHello World^FS\n^XZ\n";

#[test]
fn print_help_shows_flags() {
    let output = zpl_cmd()
        .args(["print", "--help"])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--printer"),
        "missing --printer flag in help"
    );
    assert!(
        stdout.contains("--dry-run"),
        "missing --dry-run flag in help"
    );
    assert!(
        stdout.contains("--no-lint"),
        "missing --no-lint flag in help"
    );
    assert!(stdout.contains("--strict"), "missing --strict flag in help");
    assert!(stdout.contains("--status"), "missing --status flag in help");
    assert!(stdout.contains("--verify"), "missing --verify flag in help");
    assert!(stdout.contains("--info"), "missing --info flag in help");
    assert!(stdout.contains("--wait"), "missing --wait flag in help");
    #[cfg(feature = "serial")]
    {
        assert!(stdout.contains("--serial"), "missing --serial flag in help");
        assert!(stdout.contains("--baud"), "missing --baud flag in help");
        assert!(
            stdout.contains("--serial-flow-control"),
            "missing --serial-flow-control flag in help"
        );
        assert!(
            stdout.contains("--serial-parity"),
            "missing --serial-parity flag in help"
        );
        assert!(
            stdout.contains("--serial-stop-bits"),
            "missing --serial-stop-bits flag in help"
        );
        assert!(
            stdout.contains("--serial-data-bits"),
            "missing --serial-data-bits flag in help"
        );
        assert!(
            stdout.contains("--trace-io"),
            "missing --trace-io flag in help"
        );
    }
    assert!(
        stdout.contains("--timeout"),
        "missing --timeout flag in help"
    );
}

#[test]
fn print_requires_files() {
    let mut cmd = zpl_cmd();
    cmd.args(["print", "--printer", "192.168.1.100"]);
    let output = cmd.output().unwrap();

    // clap should error with exit code 2
    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("FILE") || stderr.contains("files"),
        "expected 'required' error, got: {stderr}"
    );
}

#[test]
fn print_requires_printer_flag() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let mut cmd = zpl_cmd();
    cmd.args(["print", &path]);
    let output = cmd.output().unwrap();

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--printer") || stderr.contains("required"),
        "expected --printer required error, got: {stderr}"
    );
}

#[test]
fn print_verify_conflicts_with_dry_run() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "127.0.0.1",
            "--verify",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with")
            || (stderr.contains("--verify") && stderr.contains("--dry-run")),
        "expected clap conflict for --verify/--dry-run, got: {stderr}"
    );
}

#[test]
fn print_dry_run_pretty() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "127.0.0.1",
            "--dry-run",
            "--no-lint",
            "--output",
            "pretty",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "dry-run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dry run"),
        "expected 'dry run' in output, got: {stderr}"
    );
    assert!(
        stderr.contains("tcp"),
        "expected 'tcp' transport in output, got: {stderr}"
    );
}

#[test]
fn print_dry_run_json() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "127.0.0.1:9100",
            "--dry-run",
            "--no-lint",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON output: {e}\n{stdout}"));

    assert_eq!(json["dry_run"], true);
    assert_eq!(json["transport"], "tcp");
    assert_eq!(json["resolved_address"], "127.0.0.1:9100");
    assert!(json["validation"] == "skipped");
}

#[test]
fn print_dry_run_mac_without_serial_shows_helpful_error() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "60:95:32:1C:7A:10",
            "--dry-run",
            "--no-lint",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Bluetooth MAC address") && stderr.contains("--serial"),
        "expected helpful MAC guidance, got: {stderr}"
    );
}

#[test]
#[cfg(feature = "usb")]
fn print_dry_run_usb_pretty() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "usb",
            "--dry-run",
            "--no-lint",
            "--output",
            "pretty",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("usb"),
        "expected 'usb' in output, got: {stderr}"
    );
}

#[test]
#[cfg(feature = "usb")]
fn print_dry_run_usb_json() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "usb:0A5F:0100",
            "--dry-run",
            "--no-lint",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["dry_run"], true);
    assert_eq!(json["transport"], "usb");
    assert_eq!(json["resolved_address"], "usb:0A5F:0100");
}

#[test]
#[cfg(feature = "serial")]
fn print_dry_run_serial_json() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "/dev/ttyUSB0",
            "--serial",
            "--dry-run",
            "--no-lint",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["dry_run"], true);
    assert_eq!(json["transport"], "serial");
    assert_eq!(json["resolved_address"], "/dev/ttyUSB0");
}

#[test]
#[cfg(feature = "serial")]
fn print_serial_mac_address_shows_helpful_error() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "60:95:32:1C:7A:10",
            "--serial",
            "--dry-run",
            "--no-lint",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Bluetooth MAC address")
            && stderr.contains("OS serial port path")
            && stderr.contains("--serial"),
        "expected actionable MAC-address guidance, got: {stderr}"
    );
}

#[test]
#[cfg(feature = "serial")]
fn print_dry_run_serial_accepts_line_overrides() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "/dev/ttyUSB0",
            "--serial",
            "--dry-run",
            "--no-lint",
            "--baud",
            "9600",
            "--serial-flow-control",
            "software",
            "--serial-parity",
            "none",
            "--serial-stop-bits",
            "one",
            "--serial-data-bits",
            "eight",
            "--trace-io",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["transport"], "serial");
    assert_eq!(json["resolved_address"], "/dev/ttyUSB0");
}

#[test]
#[cfg(feature = "serial")]
fn serial_probe_help_shows_options() {
    let output = zpl_cmd()
        .args(["serial-probe", "--help"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--baud"), "missing --baud");
    assert!(
        stdout.contains("--serial-flow-control"),
        "missing --serial-flow-control"
    );
    assert!(
        stdout.contains("--serial-parity"),
        "missing --serial-parity"
    );
    assert!(
        stdout.contains("--serial-stop-bits"),
        "missing --serial-stop-bits"
    );
    assert!(
        stdout.contains("--serial-data-bits"),
        "missing --serial-data-bits"
    );
    assert!(
        stdout.contains("--send-test-label"),
        "missing --send-test-label"
    );
    assert!(stdout.contains("--trace-io"), "missing --trace-io");
}

#[test]
#[cfg(feature = "serial")]
fn serial_probe_json_reports_connect_failure() {
    let output = zpl_cmd()
        .args([
            "serial-probe",
            "/dev/does-not-exist",
            "--output",
            "json",
            "--timeout",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["stage"], "connect");
    assert!(
        json["message"]
            .as_str()
            .unwrap_or("")
            .contains("failed to open serial port")
    );
}

#[test]
fn print_tcp_path_rejects_bluetooth_mac_without_dry_run() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);
    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "60:95:32:1C:7A:10",
            "--no-lint",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Bluetooth MAC address")
            && stderr.contains("--serial")
            && stderr.contains("OS serial port path"),
        "expected actionable MAC-address guidance, got: {stderr}"
    );
}

#[test]
#[cfg(all(feature = "serial", feature = "usb"))]
fn print_serial_usb_conflict() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    let output = zpl_cmd()
        .args(["print", &path, "--printer", "usb", "--serial", "--no-lint"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--serial") && stderr.contains("usb") || stderr.contains("USB"),
        "expected serial/usb conflict error, got: {stderr}"
    );
}

#[test]
fn print_missing_file_fails() {
    let output = zpl_cmd()
        .args([
            "print",
            "nonexistent.zpl",
            "--printer",
            "192.168.1.100",
            "--no-lint",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read") || stderr.contains("No such file"),
        "expected file-not-found error, got: {stderr}"
    );
}

#[test]
fn print_dry_run_with_validation() {
    let (_dir, path) = write_temp_zpl(SAMPLE_ZPL);

    // Without --no-lint, validation runs (needs tables which may or may not be embedded)
    // This test just ensures the flag interaction works -- if tables are available,
    // validation should pass for valid ZPL.
    let output = zpl_cmd()
        .args([
            "print",
            &path,
            "--printer",
            "127.0.0.1",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Either succeeds with validation, or fails because tables aren't embedded
    if output.status.success() {
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["validation"], "passed");
    } else {
        // If tables aren't embedded, it should error about tables
        assert!(
            stderr.contains("tables") || stderr.contains("parser"),
            "expected tables error, got: {stderr}"
        );
    }
}
