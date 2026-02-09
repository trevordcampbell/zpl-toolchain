//! Shared test helpers for `zpl_toolchain_core` integration tests.

#![allow(unreachable_pub)]

use std::sync::LazyLock;
use zpl_toolchain_core::grammar::ast::{ArgSlot, Node};
use zpl_toolchain_core::grammar::parser::ParseResult;
use zpl_toolchain_diagnostics::Diagnostic;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::ParserTables;

/// Tables loaded once per test binary via LazyLock — avoids repeated disk I/O.
pub static TABLES: LazyLock<ParserTables> = LazyLock::new(|| {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../generated/parser_tables.json");
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("failed to parse {}: {}", path.display(), e))
});

// ─── Parse-result helpers ────────────────────────────────────────────────────

/// Collect command codes (in order) from all labels.
#[allow(dead_code)]
pub fn extract_codes(result: &ParseResult) -> Vec<String> {
    result
        .ast
        .labels
        .iter()
        .flat_map(|l| l.nodes.iter())
        .filter_map(|n| match n {
            Node::Command { code, .. } => Some(code.clone()),
            _ => None,
        })
        .collect()
}

/// Collect command codes for a specific label index.
#[allow(dead_code)]
pub fn extract_label_codes(result: &ParseResult, label_idx: usize) -> Vec<String> {
    result.ast.labels[label_idx]
        .nodes
        .iter()
        .filter_map(|n| match n {
            Node::Command { code, .. } => Some(code.clone()),
            _ => None,
        })
        .collect()
}

/// Collect diagnostic codes from parser diagnostics.
#[allow(dead_code)]
pub fn extract_diag_codes(result: &ParseResult) -> Vec<String> {
    result
        .diagnostics
        .iter()
        .map(|d| d.id.to_string())
        .collect()
}

/// Find the first Command node matching the given code and return its args.
#[allow(dead_code)]
pub fn find_args(result: &ParseResult, target_code: &str) -> Vec<ArgSlot> {
    result
        .ast
        .labels
        .iter()
        .flat_map(|l| l.nodes.iter())
        .find_map(|n| match n {
            Node::Command { code, args, .. } if code == target_code => Some(args.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Find first diagnostic with the given code.
#[allow(dead_code)]
pub fn find_diag<'a>(issues: &'a [Diagnostic], code: &str) -> &'a Diagnostic {
    issues
        .iter()
        .find(|d| &*d.id == code)
        .unwrap_or_else(|| panic!("expected diagnostic {code}"))
}

// ─── Severity helpers ────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn is_severity_error(s: &zpl_toolchain_diagnostics::Severity) -> bool {
    matches!(s, zpl_toolchain_diagnostics::Severity::Error)
}

#[allow(dead_code)]
pub fn is_severity_warn(s: &zpl_toolchain_diagnostics::Severity) -> bool {
    matches!(s, zpl_toolchain_diagnostics::Severity::Warn)
}

#[allow(dead_code)]
pub fn is_severity_info(s: &zpl_toolchain_diagnostics::Severity) -> bool {
    matches!(s, zpl_toolchain_diagnostics::Severity::Info)
}

// ─── Profile fixture helpers ─────────────────────────────────────────────────

#[allow(dead_code)]
pub fn profile_from_json(json: &str) -> Profile {
    serde_json::from_str(json).expect("invalid profile JSON in test fixture")
}

#[allow(dead_code)]
pub fn profile_800x1200() -> Profile {
    profile_from_json(
        r#"{"id":"test","schema_version":"1.0.0","dpi":203,"page":{"width_dots":800,"height_dots":1200}}"#,
    )
}
