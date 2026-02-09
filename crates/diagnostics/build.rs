//! Build script for generating diagnostic code data structures at compile time.
//!
//! This script reads `spec/diagnostics.jsonc` and generates two Rust files:
//! - `generated_codes.rs`: Contains public constants mapping diagnostic constant names to their IDs
//! - `generated_explain.rs`: Contains a match expression mapping diagnostic IDs to their descriptions

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let spec_path = Path::new("spec/diagnostics.jsonc");
    println!("cargo:rerun-if-changed={}", spec_path.display());

    let raw = fs::read_to_string(spec_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", spec_path.display()));

    // Use the shared JSONC stripper from spec-compiler (handles line, inline,
    // and block comments correctly). We inline a minimal version here to avoid
    // adding a build dependency on spec-compiler.
    let stripped = strip_jsonc_comments(&raw);

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
        // Escape backslashes, quotes, and newlines for valid Rust string literals.
        let escaped = description
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        explain.push_str(&format!("    \"{id}\" => Some(\"{escaped}\"),\n"));
    }

    explain.push_str("    _ => None,\n}\n");

    fs::write(out_path.join("generated_explain.rs"), &explain)
        .expect("failed to write generated_explain.rs");
}

/// Strip JSONC comments (// line, /* block */) while preserving string contents.
/// Operates on chars (not bytes) to correctly handle UTF-8.
fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_str = false;

    while i < len {
        let c = chars[i];

        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < len {
                i += 1;
                out.push(chars[i]);
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == '/' && i + 1 < len {
            let c2 = chars[i + 1];
            if c2 == '/' {
                // Line comment — skip to end of line
                i += 2;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if c2 == '*' {
                // Block comment — skip to */
                i += 2;
                while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(len);
                continue;
            }
        }

        out.push(c);
        i += 1;
    }
    out
}
