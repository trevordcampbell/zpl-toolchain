//! ZPL spec compiler — reads ZPL command specification YAML/JSONC files and
//! compiles them into parser tables, documentation bundles, and coverage
//! reports. This is an internal build-time tool, not a runtime dependency.

pub mod pipeline;
pub mod source;

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// The current spec schema version that this compiler expects.
pub const SCHEMA_VERSION: &str = "1.1.1";

/// Strip `//` and `/* */` comments from JSONC input.
///
/// Correctly handles:
/// - Escaped quotes inside strings (`\"`)
/// - Comment-like sequences inside strings (e.g., `"http://example.com"`)
/// - Backslash escapes (`\\`)
/// - Multi-byte UTF-8 characters (operates on `char` iterators, not raw bytes)
pub fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    let mut in_str = false;

    while i < len {
        let c = chars[i];

        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < len {
                // Escaped character — push the next char verbatim and skip it
                i += 1;
                out.push(chars[i]);
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        // Outside a string
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == '/' && i + 1 < len {
            let c2 = chars[i + 1];
            // Line comment
            if c2 == '/' {
                i += 2;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            // Block comment
            if c2 == '*' {
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

/// Parse a JSONC string into a `serde_json::Value`, stripping comments first.
pub fn parse_jsonc(input: &str) -> Result<Value> {
    let stripped = strip_jsonc(input);
    let v: Value =
        serde_json::from_str(&stripped).context("invalid JSON/JSONC after stripping comments")?;
    Ok(v)
}

/// Serialize a JSON value to a pretty-printed file, creating parent directories as needed.
pub fn write_json_pretty<P: AsRef<Path>>(path: P, v: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(v)?;
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}

/// Build a prefix trie over all command opcodes, returned as a JSON value.
///
/// Used for efficient prefix-based opcode lookup during parsing.
pub fn build_opcode_trie(commands: &[serde_json::Value]) -> serde_json::Value {
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Node {
        children: BTreeMap<char, Node>,
        terminal: bool,
    }
    fn insert(node: &mut Node, s: &str) {
        let mut cur = node;
        for ch in s.chars() {
            cur = cur.children.entry(ch).or_default();
        }
        cur.terminal = true;
    }
    let mut root = Node::default();
    for cmd in commands {
        if let Some(arr) = cmd.get("codes").and_then(|c| c.as_array()) {
            for code in arr {
                if let Some(s) = code.as_str() {
                    insert(&mut root, s);
                }
            }
        } else if let Some(code) = cmd.get("code").and_then(|c| c.as_str()) {
            insert(&mut root, code);
        }
    }
    fn to_json(n: &Node) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("terminal".into(), serde_json::Value::Bool(n.terminal));
        let mut kids = serde_json::Map::new();
        for (k, v) in &n.children {
            kids.insert(k.to_string(), to_json(v));
        }
        map.insert("children".into(), serde_json::Value::Object(kids));
        serde_json::Value::Object(map)
    }
    to_json(&root)
}
