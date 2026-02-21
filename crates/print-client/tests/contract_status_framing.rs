//! Contract-driven status/framing conformance tests shared with TS fixtures.

use std::io::Cursor;
use std::time::Duration;

use serde::Deserialize;
use zpl_toolchain_print_client::{HostStatus, PrinterInfo, expected_frame_count, read_frames};

#[derive(Debug, Deserialize)]
struct CommandFixture {
    command: String,
    expected_frame_count: usize,
}

#[derive(Debug, Deserialize)]
struct HostStatusExpected {
    paper_out: bool,
    paused: bool,
    head_up: bool,
    ribbon_out: bool,
    formats_in_buffer: u32,
    labels_remaining: u32,
    print_mode: String,
}

#[derive(Debug, Deserialize)]
struct HostStatusFixture {
    healthy_raw: String,
    truncated_raw: String,
    expected_healthy: HostStatusExpected,
}

#[derive(Debug, Deserialize)]
struct PrinterInfoExpected {
    model: String,
    firmware: String,
    dpi: u32,
    memory_kb: u32,
}

#[derive(Debug, Deserialize)]
struct PrinterInfoFixture {
    raw: String,
    expected: PrinterInfoExpected,
}

#[derive(Debug, Deserialize)]
struct PrintStatusFramingFixture {
    version: u32,
    commands: Vec<CommandFixture>,
    host_status: HostStatusFixture,
    printer_info: PrinterInfoFixture,
}

fn load_fixture() -> PrintStatusFramingFixture {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/fixtures/print-status-framing.v1.json"
    );
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture file {path}: {e}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse fixture file {path}: {e}"))
}

fn command_expected_frame_count(fixture: &PrintStatusFramingFixture, command: &str) -> usize {
    fixture
        .commands
        .iter()
        .find(|entry| entry.command == command)
        .unwrap_or_else(|| panic!("missing command fixture for {command}"))
        .expected_frame_count
}

#[test]
fn expected_frame_count_matches_contract_fixture() {
    let fixture = load_fixture();
    assert_eq!(fixture.version, 1, "unexpected fixture version");
    for entry in &fixture.commands {
        assert_eq!(
            expected_frame_count(entry.command.as_bytes()),
            entry.expected_frame_count,
            "frame count mismatch for command {}",
            entry.command
        );
    }
}

#[test]
fn rust_status_and_info_parse_match_contract_fixture() {
    let fixture = load_fixture();
    assert_eq!(fixture.version, 1, "unexpected fixture version");

    let hs_expected = command_expected_frame_count(&fixture, "~HS");
    let mut hs_cursor = Cursor::new(fixture.host_status.healthy_raw.as_bytes());
    let hs_frames = read_frames(&mut hs_cursor, hs_expected, Duration::from_secs(1), 1024)
        .expect("expected fixture ~HS frames should parse");
    let hs = HostStatus::parse(&hs_frames).expect("expected fixture ~HS should parse");

    assert_eq!(hs.paper_out, fixture.host_status.expected_healthy.paper_out);
    assert_eq!(hs.paused, fixture.host_status.expected_healthy.paused);
    assert_eq!(hs.head_up, fixture.host_status.expected_healthy.head_up);
    assert_eq!(
        hs.ribbon_out,
        fixture.host_status.expected_healthy.ribbon_out
    );
    assert_eq!(
        hs.formats_in_buffer,
        fixture.host_status.expected_healthy.formats_in_buffer
    );
    assert_eq!(
        hs.labels_remaining,
        fixture.host_status.expected_healthy.labels_remaining
    );
    assert_eq!(
        format!("{:?}", hs.print_mode),
        fixture.host_status.expected_healthy.print_mode
    );

    let hi_expected = command_expected_frame_count(&fixture, "~HI");
    let mut hi_cursor = Cursor::new(fixture.printer_info.raw.as_bytes());
    let hi_frames = read_frames(&mut hi_cursor, hi_expected, Duration::from_secs(1), 1024)
        .expect("expected fixture ~HI frame should parse");
    let hi = PrinterInfo::parse(&hi_frames).expect("expected fixture ~HI should parse");

    assert_eq!(hi.model, fixture.printer_info.expected.model);
    assert_eq!(hi.firmware, fixture.printer_info.expected.firmware);
    assert_eq!(hi.dpi, fixture.printer_info.expected.dpi);
    assert_eq!(hi.memory_kb, fixture.printer_info.expected.memory_kb);
}

#[test]
fn rust_framing_rejects_truncated_hs_fixture() {
    let fixture = load_fixture();
    let hs_expected = command_expected_frame_count(&fixture, "~HS");
    let mut hs_cursor = Cursor::new(fixture.host_status.truncated_raw.as_bytes());
    let result = read_frames(&mut hs_cursor, hs_expected, Duration::from_secs(1), 1024);
    assert!(result.is_err(), "truncated ~HS fixture should fail framing");
}
