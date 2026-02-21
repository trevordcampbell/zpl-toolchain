//! Contract-driven job lifecycle conformance tests.
//! Loads contracts/fixtures/print-job-lifecycle.v1.json and asserts phase parity.

use serde::Deserialize;
use zpl_toolchain_print_client::JobPhase;

#[derive(Debug, Deserialize)]
struct LifecycleFixture {
    version: u32,
    phases: Vec<String>,
}

fn load_fixture() -> LifecycleFixture {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/fixtures/print-job-lifecycle.v1.json"
    );
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture file {path}: {e}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse fixture file {path}: {e}"))
}

fn job_phase_to_str(p: JobPhase) -> &'static str {
    match p {
        JobPhase::Queued => "queued",
        JobPhase::Sending => "sending",
        JobPhase::Sent => "sent",
        JobPhase::Printing => "printing",
        JobPhase::Completed => "completed",
        JobPhase::Failed => "failed",
        JobPhase::Aborted => "aborted",
        _ => unreachable!("JobPhase has more variants than contract; update test"),
    }
}

#[test]
fn job_phases_match_contract_fixture() {
    let fixture = load_fixture();
    assert_eq!(fixture.version, 1, "unexpected fixture version");

    let contract_phases: Vec<&str> = fixture.phases.iter().map(String::as_str).collect();

    let rust_phases = [
        JobPhase::Queued,
        JobPhase::Sending,
        JobPhase::Sent,
        JobPhase::Printing,
        JobPhase::Completed,
        JobPhase::Failed,
        JobPhase::Aborted,
    ];

    for phase in &rust_phases {
        let s = job_phase_to_str(*phase);
        assert!(
            contract_phases.contains(&s),
            "JobPhase {:?} ({}) missing from contract phases: {:?}",
            phase,
            s,
            contract_phases
        );
    }

    assert_eq!(
        rust_phases.len(),
        contract_phases.len(),
        "Rust JobPhase count must match contract phases"
    );
}
