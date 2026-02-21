//! ZPL spec compiler — reads ZPL command specification YAML/JSONC files and
//! compiles them into parser tables, documentation bundles, and coverage
//! reports. This is an internal build-time tool, not a runtime dependency.

pub mod pipeline;
pub mod source;

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;
pub use zpl_toolchain_jsonc_strip::strip_jsonc;

/// The current spec schema version that this compiler expects.
pub const SCHEMA_VERSION: &str = "1.1.1";

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
