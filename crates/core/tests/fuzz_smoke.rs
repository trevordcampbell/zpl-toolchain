//! Fuzz smoke tests for the ZPL lexer and parser.
//!
//! These tests feed random, adversarial, and edge-case inputs to the tokenizer
//! and parser to verify they never panic and that basic structural invariants
//! hold on every `ParseResult`.
//!
//! No external crate dependencies are used — a simple deterministic PRNG
//! provides reproducible randomness.

mod common;

use zpl_toolchain_core::grammar::ast::Node;
use zpl_toolchain_core::grammar::lexer::tokenize;
use zpl_toolchain_core::grammar::parser::{ParseResult, parse_str, parse_with_tables};
use zpl_toolchain_core::validate::validate;

// ─── Simple deterministic PRNG (LCG) ────────────────────────────────────────

struct SimpleRng(u64);

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn gen_range(&mut self, max: usize) -> usize {
        (self.next() as usize) % max
    }

    fn gen_bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }
}

// ─── Invariant checking ─────────────────────────────────────────────────────

/// Assert structural invariants on any `ParseResult`, regardless of input.
fn assert_invariants(result: &ParseResult, input: &str) {
    // labels is a valid Vec (may be empty)
    let _ = result.ast.labels.len();

    for label in &result.ast.labels {
        for node in &label.nodes {
            let s = match node {
                Node::Command { span, .. }
                | Node::FieldData { span, .. }
                | Node::RawData { span, .. }
                | Node::Trivia { span, .. } => span,
                _ => continue,
            };
            assert!(
                s.start <= s.end,
                "Span start ({}) > end ({}) in input {:?}",
                s.start,
                s.end,
                truncate(input, 120),
            );
            assert!(
                s.end <= input.len(),
                "Span end ({}) > input len ({}) in input {:?}",
                s.end,
                input.len(),
                truncate(input, 120),
            );
        }
    }

    // Verify diagnostic spans are valid
    for diag in &result.diagnostics {
        if let Some(span) = diag.span {
            assert!(
                span.start <= span.end,
                "Diagnostic span start ({}) > end ({}): {:?}",
                span.start,
                span.end,
                diag
            );
            assert!(
                span.end <= input.len(),
                "Diagnostic span end ({}) > input length ({}): {:?}",
                span.end,
                input.len(),
                diag
            );
        }
    }
}

/// Truncate a string for error messages (safe for multi-byte UTF-8).
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Find the largest char boundary <= max to avoid slicing mid-character.
        let safe_end = (0..=max)
            .rev()
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(0);
        format!("{}…({} bytes total)", &s[..safe_end], s.len())
    }
}

/// Parse an input (with and without tables) and check invariants + validate.
fn fuzz_parse(input: &str, tables: &zpl_toolchain_spec_tables::ParserTables) {
    // Without tables
    let result = parse_str(input);
    assert_invariants(&result, input);

    // With tables
    let result = parse_with_tables(input, Some(tables));
    assert_invariants(&result, input);

    // Validate should also never panic
    let _validation = validate(&result.ast, tables);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Category A: Random byte strings
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tokenizer_no_panic_random_bytes() {
    let mut rng = SimpleRng::new(0xDEAD_BEEF);
    for len in [0, 1, 2, 5, 10, 50, 100, 500, 1000, 5000] {
        for _ in 0..20 {
            let bytes = rng.gen_bytes(len);
            let input = String::from_utf8_lossy(&bytes);
            let _tokens = tokenize(&input);
        }
    }
}

#[test]
fn parser_no_panic_random_bytes() {
    let tables = &*common::TABLES;
    let mut rng = SimpleRng::new(0xCAFE_BABE);
    for len in [0, 1, 2, 5, 10, 50, 100, 500, 1000, 5000] {
        for _ in 0..20 {
            let bytes = rng.gen_bytes(len);
            let input = String::from_utf8_lossy(&bytes);
            fuzz_parse(&input, tables);
        }
    }
}

#[test]
fn parser_no_panic_random_ascii() {
    let tables = &*common::TABLES;
    let mut rng = SimpleRng::new(0x1234_5678);
    let ascii_chars: Vec<u8> = (0x20..=0x7E).collect();
    for len in [0, 1, 5, 20, 100, 500, 2000] {
        for _ in 0..20 {
            let s: String = (0..len)
                .map(|_| ascii_chars[rng.gen_range(ascii_chars.len())] as char)
                .collect();
            fuzz_parse(&s, tables);
        }
    }
}

#[test]
fn parser_no_panic_random_zpl_like() {
    let tables = &*common::TABLES;
    let mut rng = SimpleRng::new(0xBAAD_F00D);
    let alphabet: &[u8] = b"^~XAZFOFDFS,01234567890ABCDEFabcdef \n";
    for len in [1, 5, 20, 100, 500] {
        for _ in 0..30 {
            let s: String = (0..len)
                .map(|_| alphabet[rng.gen_range(alphabet.len())] as char)
                .collect();
            fuzz_parse(&s, tables);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Category B: Adversarial leader sequences
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parser_no_panic_adversarial_leaders() {
    let tables = &*common::TABLES;
    let cases = [
        "^",
        "~",
        "^^",
        "~~",
        "^^^^",
        "~~~~",
        "^~^~^~",
        "~^~^~^",
        "^~",
        "~^",
        "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^",
        "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        "^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~^~",
    ];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

#[test]
fn parser_no_panic_leaders_with_non_ascii() {
    let tables = &*common::TABLES;
    let cases = [
        "^é",
        "~日本語",
        "^🎉",
        "~λ",
        "^XAé^XZ",
        "^FDñoño^FS",
        "^XA^FD中文^FS^XZ",
        "^±²³",
        "~µ¶·",
        "^XA^FD\u{FEFF}^FS^XZ", // BOM
        "^XA^FD\u{200B}^FS^XZ", // zero-width space
    ];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

#[test]
fn parser_no_panic_leaders_at_eof() {
    let tables = &*common::TABLES;
    let cases = [
        "^XA^",
        "^XA~",
        "^XA^FO0,0^",
        "^XA^FD",
        "^XA^FDhello",
        "^XA^GF",
    ];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Category C: Pathological nesting / repetition
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parser_no_panic_repeated_xa() {
    let tables = &*common::TABLES;
    let input = "^XA".repeat(10_000);
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_repeated_labels() {
    let tables = &*common::TABLES;
    let input = "^XA^XZ".repeat(1_000);
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_repeated_fo_no_label() {
    let tables = &*common::TABLES;
    let input = "^FO0,0".repeat(1_000);
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_very_long_arg_string() {
    let tables = &*common::TABLES;
    let input = format!("^XA^FO{}^XZ", ",0".repeat(5_000));
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_deeply_nested_field_data() {
    let tables = &*common::TABLES;
    // Lots of ^FD^FS pairs without closing the label
    let mut input = String::from("^XA");
    for _ in 0..1_000 {
        input.push_str("^FDtest^FS");
    }
    input.push_str("^XZ");
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_alternating_open_close() {
    let tables = &*common::TABLES;
    // Interleaved ^XA and ^XZ in weird patterns
    let mut input = String::new();
    for _ in 0..500 {
        input.push_str("^XA^XZ^XA");
    }
    for _ in 0..500 {
        input.push_str("^XZ");
    }
    fuzz_parse(&input, tables);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Category D: Edge-case strings
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parser_no_panic_empty_input() {
    let tables = &*common::TABLES;
    fuzz_parse("", tables);
}

#[test]
fn parser_no_panic_single_chars() {
    let tables = &*common::TABLES;
    let cases = ["\n", "\r", "\t", " ", "\0", ",", ";", "X", "A", "Z"];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

#[test]
fn parser_no_panic_whitespace_variants() {
    let tables = &*common::TABLES;
    let cases: &[&str] = &["   ", "\n\n\n", "\r\n\r\n", "\t\t\t", " \n \t \r "];
    for input in cases {
        fuzz_parse(input, tables);
    }
    let long_spaces = " ".repeat(10_000);
    fuzz_parse(&long_spaces, tables);
}

#[test]
fn parser_no_panic_null_bytes() {
    let tables = &*common::TABLES;
    let cases = [
        "\0",
        "\0\0\0",
        "\0\0\0\0\0\0\0\0\0\0",
        "^XA\0^XZ",
        "^XA^FD\0\0\0^FS^XZ",
        "\0^XA\0^FO0,0\0^XZ\0",
    ];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

#[test]
fn parser_no_panic_very_long_field_data() {
    let tables = &*common::TABLES;
    let content = "A".repeat(100_000);
    let input = format!("^XA^FD{}^FS^XZ", content);
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_unicode_variety() {
    let tables = &*common::TABLES;
    let cases = [
        "^XA^FD\u{0000}^FS^XZ",                 // null
        "^XA^FD\u{FFFF}^FS^XZ",                 // max BMP
        "^XA^FD\u{10FFFF}^FS^XZ",               // max unicode
        "^XA^FDé à ü ñ ö^FS^XZ",                // latin extended
        "^XA^FD日本語テスト^FS^XZ",             // CJK
        "^XA^FD🎉🚀💻🔥^FS^XZ",                 // emoji
        "^XA^FDمرحبا^FS^XZ",                    // Arabic (RTL)
        "^XA^FDשלום^FS^XZ",                     // Hebrew (RTL)
        "^XA^FDα β γ δ ε^FS^XZ",                // Greek
        "^XA^FDПривет^FS^XZ",                   // Cyrillic
        "^XA^FD\u{200E}\u{200F}\u{200B}^FS^XZ", // direction marks / zero-width
        "^XA^FD\u{FEFF}^FS^XZ",                 // BOM
        "^XA^FD\u{202A}\u{202B}\u{202C}^FS^XZ", // embedding marks
    ];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

#[test]
fn parser_no_panic_mixed_control_chars() {
    let tables = &*common::TABLES;
    let mut input = String::from("^XA^FD");
    for c in 0u8..32 {
        input.push(c as char);
    }
    input.push_str("^FS^XZ");
    fuzz_parse(&input, tables);
}

#[test]
fn parser_no_panic_comments() {
    let tables = &*common::TABLES;
    let cases = [
        "; just a comment",
        ";;;;;;",
        "; ^XA ^XZ ^FD",
        "^XA ; comment\n^FO0,0^XZ",
        "; \n; \n; \n",
    ];
    for input in &cases {
        fuzz_parse(input, tables);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Category E: Invariant checks on structured inputs
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn invariants_valid_labels() {
    let tables = &*common::TABLES;
    let inputs = [
        "^XA^XZ",
        "^XA^FO10,10^FDHello^FS^XZ",
        "^XA^CF0,30^FO50,50^FDWorld^FS^XZ",
        "^XA^XZ^XA^XZ^XA^XZ",
        "^XA^FO0,0^A0N,30,30^FDTest^FS^XZ",
    ];
    for input in &inputs {
        let result = parse_with_tables(input, Some(tables));
        assert_invariants(&result, input);
        let _v = validate(&result.ast, tables);
    }
}

#[test]
fn invariants_all_spans_valid_random() {
    let tables = &*common::TABLES;
    let mut rng = SimpleRng::new(0xFACE_FEED);
    let alphabet: &[u8] = b"^~XAZFOFDFS,01234567890 \n";

    for _ in 0..200 {
        let len = rng.gen_range(300);
        let s: String = (0..len)
            .map(|_| alphabet[rng.gen_range(alphabet.len())] as char)
            .collect();

        let result = parse_with_tables(&s, Some(tables));
        assert_invariants(&result, &s);
        let _v = validate(&result.ast, tables);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Category F: Tokenizer-specific edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tokenizer_no_panic_all_single_bytes() {
    // Every possible single-byte value (as lossy UTF-8)
    for b in 0u8..=255 {
        let bytes = [b];
        let input = String::from_utf8_lossy(&bytes);
        let _tokens = tokenize(&input);
    }
}

#[test]
fn tokenizer_no_panic_all_two_byte_pairs() {
    // Sample two-byte pairs with interesting boundaries
    let interesting: &[u8] = &[
        0, 1, 9, 10, 13, 32, 44, 59, 64, 65, 88, 90, 94, 126, 127, 128, 200, 255,
    ];
    for &a in interesting {
        for &b in interesting {
            let bytes = [a, b];
            let input = String::from_utf8_lossy(&bytes);
            let _tokens = tokenize(&input);
        }
    }
}

#[test]
fn tokenizer_invariants() {
    let mut rng = SimpleRng::new(0xBEEF_CAFE);
    for _ in 0..200 {
        let len = rng.gen_range(500);
        let bytes = rng.gen_bytes(len);
        let input = String::from_utf8_lossy(&bytes);
        let tokens = tokenize(&input);

        // Tokens should cover the input without gaps or overlaps
        let mut expected_start = 0usize;
        for tok in &tokens {
            assert!(
                tok.start == expected_start,
                "Token gap/overlap at byte {}: expected start {}, got {}",
                tok.start,
                expected_start,
                tok.start,
            );
            assert!(
                tok.end > tok.start,
                "Zero-length token at byte {}",
                tok.start,
            );
            assert!(
                tok.end <= input.len(),
                "Token end ({}) exceeds input length ({})",
                tok.end,
                input.len(),
            );
            expected_start = tok.end;
        }
        assert_eq!(
            expected_start,
            input.len(),
            "Tokens did not cover full input (covered {} of {} bytes)",
            expected_start,
            input.len(),
        );
    }
}
