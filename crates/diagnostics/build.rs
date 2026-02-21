//! Build script for generating diagnostic code data structures at compile time.
//!
//! This script reads `spec/diagnostics.jsonc` and generates Rust files:
//! - `generated_codes.rs`: Contains public constants mapping diagnostic constant names to their IDs
//! - `generated_explain.rs`: Contains a match expression mapping diagnostic IDs to their descriptions
//! - `generated_policy.rs`: Contains policy constants derived from diagnostic spec metadata
//! - `generated_severity.rs`: Contains code → default severity lookup
//! - `generated_templates.rs`: Contains (code, variant) → message template lookup

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use zpl_toolchain_jsonc_strip::strip_jsonc;

fn main() {
    let spec_path = Path::new("spec/diagnostics.jsonc");
    println!("cargo:rerun-if-changed={}", spec_path.display());

    let raw = fs::read_to_string(spec_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", spec_path.display()));

    let stripped = strip_jsonc(&raw);

    let spec: serde_json::Value =
        serde_json::from_str(&stripped).expect("failed to parse diagnostics.jsonc as JSON");

    let diagnostics = spec["diagnostics"]
        .as_array()
        .expect("diagnostics.jsonc: expected `diagnostics` array");

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // ── Duplicate / validity checks ─────────────────────────────────────
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    // ── generated_codes.rs ──────────────────────────────────────────────
    let mut codes =
        String::from("// Auto-generated from spec/diagnostics.jsonc — DO NOT EDIT.\n\n");

    for (i, entry) in diagnostics.iter().enumerate() {
        let id = entry["id"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] missing `id`"));
        let const_name = entry["constName"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] (id={id}) missing `constName`"));
        let summary = entry["summary"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] (id={id}) missing `summary`"));

        // Validate constName is a valid Rust identifier (SCREAMING_SNAKE_CASE)
        assert!(
            !const_name.is_empty()
                && const_name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                && const_name.as_bytes()[0].is_ascii_uppercase(),
            "diagnostics[{i}] (id={id}): constName '{const_name}' is not a valid SCREAMING_SNAKE_CASE identifier"
        );

        // Check for duplicates
        assert!(
            seen_ids.insert(id.to_string()),
            "diagnostics[{i}]: duplicate id '{id}'"
        );
        assert!(
            seen_names.insert(const_name.to_string()),
            "diagnostics[{i}] (id={id}): duplicate constName '{const_name}'"
        );

        codes.push_str(&format!("/// {summary}\n"));
        codes.push_str(&format!("pub const {const_name}: &str = \"{id}\";\n\n"));
    }

    fs::write(out_path.join("generated_codes.rs"), &codes)
        .expect("failed to write generated_codes.rs");

    // ── generated_explain.rs ────────────────────────────────────────────
    let mut explain = String::from("match id {\n");

    for (i, entry) in diagnostics.iter().enumerate() {
        let id = entry["id"].as_str().unwrap();
        let description = entry["description"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] (id={id}) missing `description`"));
        let escaped = escape_rust_string_literal(description);
        explain.push_str(&format!("    \"{id}\" => Some(\"{escaped}\"),\n"));
    }

    explain.push_str("    _ => None,\n}\n");

    fs::write(out_path.join("generated_explain.rs"), &explain)
        .expect("failed to write generated_explain.rs");

    // ── generated_policy.rs ─────────────────────────────────────────────
    let object_bounds = diagnostics
        .iter()
        .find(|entry| entry["id"].as_str() == Some("ZPL2311"))
        .expect("diagnostics.jsonc: expected ZPL2311 entry for object bounds policy");
    let policy = object_bounds["objectBoundsPolicy"]
        .as_object()
        .expect("diagnostics.jsonc: ZPL2311 must define objectBoundsPolicy object");
    let low_confidence_max_overflow_dots = policy["lowConfidenceMaxOverflowDots"]
        .as_f64()
        .expect("diagnostics.jsonc: ZPL2311.objectBoundsPolicy.lowConfidenceMaxOverflowDots must be a number");
    let low_confidence_max_overflow_ratio = policy["lowConfidenceMaxOverflowRatio"]
        .as_f64()
        .expect("diagnostics.jsonc: ZPL2311.objectBoundsPolicy.lowConfidenceMaxOverflowRatio must be a number");
    let low_confidence_severity = policy["lowConfidenceSeverity"].as_str().expect(
        "diagnostics.jsonc: ZPL2311.objectBoundsPolicy.lowConfidenceSeverity must be a string",
    );
    let low_confidence_severity_rs = match low_confidence_severity {
        "error" => "Severity::Error",
        "warn" => "Severity::Warn",
        "info" => "Severity::Info",
        other => panic!(
            "diagnostics.jsonc: ZPL2311.objectBoundsPolicy.lowConfidenceSeverity has invalid value '{other}'"
        ),
    };
    let mut generated_policy =
        String::from("// Auto-generated from spec/diagnostics.jsonc — DO NOT EDIT.\n\n");
    let overflow_dots_literal = format!("{low_confidence_max_overflow_dots:.6}");
    let overflow_ratio_literal = format!("{low_confidence_max_overflow_ratio:.6}");
    generated_policy
        .push_str("/// Maximum overflow in dots that is treated as low-confidence for ZPL2311.\n");
    generated_policy.push_str(&format!(
        "pub const OBJECT_BOUNDS_LOW_CONFIDENCE_MAX_OVERFLOW_DOTS: f64 = {overflow_dots_literal};\n"
    ));
    generated_policy.push_str(
        "/// Maximum overflow ratio (overflow/label-dimension) treated as low-confidence for ZPL2311.\n",
    );
    generated_policy.push_str(&format!(
        "pub const OBJECT_BOUNDS_LOW_CONFIDENCE_MAX_OVERFLOW_RATIO: f64 = {overflow_ratio_literal};\n"
    ));
    generated_policy.push_str("/// Severity used for low-confidence ZPL2311 diagnostics.\n");
    generated_policy.push_str(&format!(
        "pub const OBJECT_BOUNDS_LOW_CONFIDENCE_SEVERITY: Severity = {low_confidence_severity_rs};\n"
    ));
    fs::write(out_path.join("generated_policy.rs"), &generated_policy)
        .expect("failed to write generated_policy.rs");

    // ── generated_severity.rs ───────────────────────────────────────────
    let mut severity =
        String::from("// Auto-generated from spec/diagnostics.jsonc — DO NOT EDIT.\n\n");
    severity.push_str("match id {\n");
    for (i, entry) in diagnostics.iter().enumerate() {
        let id = entry["id"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] missing `id`"));
        let sev = entry["severity"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] (id={id}) missing `severity`"));
        let sev_rs = match sev {
            "error" => "Severity::Error",
            "warn" => "Severity::Warn",
            "info" => "Severity::Info",
            other => panic!("diagnostics[{i}] (id={id}): invalid severity '{other}'"),
        };
        severity.push_str(&format!("    \"{id}\" => Some({sev_rs}),\n"));
    }
    severity.push_str("    _ => None,\n}\n");
    fs::write(out_path.join("generated_severity.rs"), &severity)
        .expect("failed to write generated_severity.rs");

    // ── generated_templates.rs ──────────────────────────────────────────
    let mut templates =
        String::from("// Auto-generated from spec/diagnostics.jsonc — DO NOT EDIT.\n\n");
    templates.push_str("match (id, variant) {\n");
    for (i, entry) in diagnostics.iter().enumerate() {
        let id = entry["id"]
            .as_str()
            .unwrap_or_else(|| panic!("diagnostics[{i}] missing `id`"));
        let context_keys: HashSet<String> = entry["contextKeys"]
            .as_array()
            .unwrap_or_else(|| panic!("diagnostics[{i}] (id={id}) missing `contextKeys`"))
            .iter()
            .map(|k| {
                k.as_str().unwrap_or_else(|| {
                    panic!("diagnostics[{i}] (id={id}) contextKeys entries must be strings")
                })
            })
            .map(str::to_string)
            .collect();
        if let Some(map) = entry["messageTemplates"].as_object() {
            for (variant, template) in map {
                let template = template.as_str().unwrap_or_else(|| {
                    panic!("diagnostics[{i}] (id={id}) messageTemplates.{variant} must be a string")
                });
                let placeholders = extract_template_placeholders(template);
                for placeholder in placeholders {
                    assert!(
                        context_keys.contains(&placeholder),
                        "diagnostics[{i}] (id={id}) messageTemplates.{variant} references placeholder '{{{placeholder}}}' not listed in contextKeys"
                    );
                }
                let escaped = escape_rust_string_literal(template);
                templates.push_str(&format!(
                    "    (\"{id}\", \"{variant}\") => Some(\"{escaped}\"),\n"
                ));
            }
        }
    }
    templates.push_str("    _ => None,\n}\n");
    fs::write(out_path.join("generated_templates.rs"), &templates)
        .expect("failed to write generated_templates.rs");
}

fn escape_rust_string_literal(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

fn extract_template_placeholders(template: &str) -> HashSet<String> {
    let mut placeholders = HashSet::new();
    let mut scan_from = 0usize;
    while let Some(open_rel) = template[scan_from..].find('{') {
        let open = scan_from + open_rel;
        let after_open = open + 1;
        if let Some(close_rel) = template[after_open..].find('}') {
            let close = after_open + close_rel;
            let key = template[after_open..close].trim();
            if !key.is_empty() {
                placeholders.insert(key.to_string());
            }
            scan_from = close + 1;
        } else {
            break;
        }
    }
    placeholders
}
