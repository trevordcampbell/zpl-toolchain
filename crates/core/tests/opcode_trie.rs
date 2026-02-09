//! Tests for opcode trie recognition and lookup.

mod common;

#[test]
fn trie_opcode_recognition_mixed_set() {
    let tables = &*common::TABLES;
    assert!(
        tables.opcode_trie.is_some(),
        "opcode_trie missing in tables"
    );

    // Input with mixed opcodes that share prefixes under ^B and ^F
    let input = "^XA^BY3,2,50^BCN,142,N,N,N^FO10,10^A0N,22,26^FS^XZ";
    let res = zpl_toolchain_core::grammar::parser::parse_with_tables(input, Some(tables));

    // Extract parsed codes in order
    let mut codes = Vec::new();
    for label in &res.ast.labels {
        for node in &label.nodes {
            if let zpl_toolchain_core::grammar::ast::Node::Command { code, .. } = node {
                codes.push(code.clone());
            }
        }
    }

    // Expect longest-match codes recognized correctly, including ^XA/^XZ markers now stored as nodes
    let expected = vec!["^XA", "^BY", "^BC", "^FO", "^A", "^FS", "^XZ"];
    assert_eq!(codes, expected);
}
