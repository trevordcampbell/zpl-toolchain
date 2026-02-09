//! Diagnostics for the ZPL toolchain.
//!
//! Provides [`Diagnostic`], [`Severity`], [`Span`], and [`LineIndex`] types
//! used to report errors, warnings, and informational messages from the parser
//! and validator. Diagnostic codes are defined in the [`codes`] module.

#![warn(missing_docs)]

/// Diagnostic ID constants auto-generated from the spec.
pub mod codes;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

// ── LineIndex ────────────────────────────────────────────────────────────

/// Maps byte offsets in a source string to line and column positions.
///
/// Lines and columns are **0-indexed** internally. Use [`LineIndex::line_col`]
/// to get a `(line, col)` pair and add 1 when displaying to users.
///
/// The index is built in O(n) time and each lookup is O(log n) via binary
/// search. This struct is intentionally dependency-free so it can be reused
/// by WASM bindings, an LSP server, or any other consumer.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line.
    /// `line_starts[0]` is always 0.
    line_starts: Vec<usize>,
}

impl LineIndex {
    /// Build a `LineIndex` from source text.
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to a 0-indexed `(line, column)` pair.
    ///
    /// If `offset` is past the end of the source, the last line is returned
    /// with the column clamped to the line length.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line]);
        (line, col)
    }

    /// Byte offset of the start of the given 0-indexed line.
    ///
    /// Returns `None` if `line` is out of bounds.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.line_starts.get(line).copied()
    }

    /// Total number of lines (at least 1 for non-empty or even empty input).
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Severity level for a diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    /// Hard error — the input is invalid.
    Error,
    /// Warning — the input may produce unexpected results.
    Warn,
    /// Informational note.
    Info,
}

/// Byte span in the source input.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the first character (0-based).
    pub start: usize,
    /// Byte offset one past the last character.
    pub end: usize,
}

impl Span {
    /// Create a span covering `[start, end)`.
    ///
    /// Panics if `end < start`.
    pub fn new(start: usize, end: usize) -> Self {
        assert!(end >= start, "Span end ({end}) < start ({start})");
        Self { start, end }
    }

    /// Create a zero-width span at the given position.
    pub fn empty(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }
}

/// A diagnostic message produced by the parser or validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Unique diagnostic code (e.g., `"ZPL1101"`).
    pub id: Cow<'static, str>,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional byte span in the source input that this diagnostic relates to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    /// Machine-readable context for tooling. Keys and values are free-form strings.
    /// Absent when no context is applicable. Serialized only when present.
    ///
    /// Uses `BTreeMap` for deterministic key ordering in serialized output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<BTreeMap<String, String>>,
}

impl Diagnostic {
    /// Create a diagnostic with the given fields.
    pub fn new(
        id: impl Into<Cow<'static, str>>,
        severity: Severity,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            message: message.into(),
            span,
            context: None,
        }
    }

    /// Shorthand for an `Error` diagnostic.
    pub fn error(
        id: impl Into<Cow<'static, str>>,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self::new(id, Severity::Error, message, span)
    }

    /// Shorthand for a `Warn` diagnostic.
    pub fn warn(
        id: impl Into<Cow<'static, str>>,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self::new(id, Severity::Warn, message, span)
    }

    /// Shorthand for an `Info` diagnostic.
    pub fn info(
        id: impl Into<Cow<'static, str>>,
        message: impl Into<String>,
        span: Option<Span>,
    ) -> Self {
        Self::new(id, Severity::Info, message, span)
    }

    /// Attach machine-readable context metadata (builder pattern).
    ///
    /// Context is a set of key-value string pairs providing structured details
    /// about the diagnostic for tooling, filtering, and programmatic consumption.
    /// Keys are short descriptors like `"command"`, `"field"`, `"value"`, etc.
    pub fn with_context(mut self, ctx: BTreeMap<String, String>) -> Self {
        self.context = Some(ctx);
        self
    }

    /// Returns the human-readable explanation for this diagnostic's code, if available.
    pub fn explain(&self) -> Option<&'static str> {
        explain(&self.id)
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warn => write!(f, "warn"),
            Severity::Info => write!(f, "info"),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]: {}", self.severity, self.id, self.message)
    }
}

/// Returns the human-readable explanation for a diagnostic code, if known.
///
/// Auto-generated from `spec/diagnostics.jsonc` at build time.
pub fn explain(id: &str) -> Option<&'static str> {
    include!(concat!(env!("OUT_DIR"), "/generated_explain.rs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LineIndex ────────────────────────────────────────────────────────

    #[test]
    fn line_index_single_line() {
        let idx = LineIndex::new("hello");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_col(0), (0, 0));
        assert_eq!(idx.line_col(4), (0, 4));
    }

    #[test]
    fn line_index_two_lines() {
        let idx = LineIndex::new("ab\ncd");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_col(0), (0, 0)); // 'a'
        assert_eq!(idx.line_col(1), (0, 1)); // 'b'
        assert_eq!(idx.line_col(2), (0, 2)); // '\n'
        assert_eq!(idx.line_col(3), (1, 0)); // 'c'
        assert_eq!(idx.line_col(4), (1, 1)); // 'd'
    }

    #[test]
    fn line_index_trailing_newline() {
        let idx = LineIndex::new("a\n");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_col(0), (0, 0));
        assert_eq!(idx.line_col(2), (1, 0)); // start of (empty) second line
    }

    #[test]
    fn line_index_empty_input() {
        let idx = LineIndex::new("");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_col(0), (0, 0));
    }

    #[test]
    fn line_index_multiple_newlines() {
        let idx = LineIndex::new("a\n\nb\n");
        assert_eq!(idx.line_count(), 4);
        assert_eq!(idx.line_col(0), (0, 0)); // 'a'
        assert_eq!(idx.line_col(2), (1, 0)); // empty line
        assert_eq!(idx.line_col(3), (2, 0)); // 'b'
        assert_eq!(idx.line_col(5), (3, 0)); // empty trailing line
    }

    #[test]
    fn line_index_multibyte_utf8() {
        // '€' is 3 bytes in UTF-8
        let idx = LineIndex::new("€\na");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_col(0), (0, 0)); // start of '€'
        assert_eq!(idx.line_col(3), (0, 3)); // '\n' (byte offset 3)
        assert_eq!(idx.line_col(4), (1, 0)); // 'a'
    }

    #[test]
    fn line_index_line_start() {
        let idx = LineIndex::new("ab\ncd\nef");
        assert_eq!(idx.line_start(0), Some(0));
        assert_eq!(idx.line_start(1), Some(3));
        assert_eq!(idx.line_start(2), Some(6));
        assert_eq!(idx.line_start(3), None);
    }

    #[test]
    fn line_index_offset_past_end() {
        let idx = LineIndex::new("hi");
        // offset past the end should clamp to last line
        let (line, col) = idx.line_col(100);
        assert_eq!(line, 0);
        assert_eq!(col, 100);
    }

    // ── Span ────────────────────────────────────────────────────────────

    #[test]
    fn span_new_valid() {
        let s = Span::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn span_empty() {
        let s = Span::empty(7);
        assert_eq!(s.start, 7);
        assert_eq!(s.end, 7);
    }

    #[test]
    #[should_panic(expected = "Span end (3) < start (5)")]
    fn span_new_inverted_panics() {
        Span::new(5, 3);
    }

    // ── Severity Display ────────────────────────────────────────────────

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warn), "warn");
        assert_eq!(format!("{}", Severity::Info), "info");
    }

    // ── Diagnostic constructors ─────────────────────────────────────────

    #[test]
    fn diagnostic_error_constructor() {
        let d = Diagnostic::error(codes::ARITY, "too many args", None);
        assert_eq!(d.id, "ZPL1101");
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "too many args");
        assert!(d.span.is_none());
    }

    #[test]
    fn diagnostic_warn_constructor() {
        let d = Diagnostic::warn(codes::NOTE, "info note", Some(Span::new(0, 5)));
        assert_eq!(d.severity, Severity::Warn);
        assert_eq!(d.span, Some(Span::new(0, 5)));
    }

    #[test]
    fn diagnostic_info_constructor() {
        let d = Diagnostic::info("CUSTOM", "custom message", None);
        assert_eq!(d.severity, Severity::Info);
        assert_eq!(d.id, "CUSTOM");
    }

    // ── Diagnostic Display ──────────────────────────────────────────────

    #[test]
    fn diagnostic_display() {
        let d = Diagnostic::error(codes::ARITY, "too many arguments", None);
        assert_eq!(format!("{}", d), "error[ZPL1101]: too many arguments");
    }

    // ── Diagnostic explain ──────────────────────────────────────────────

    #[test]
    fn diagnostic_explain_known() {
        let d = Diagnostic::error(codes::ARITY, "test", None);
        assert!(d.explain().is_some());
        assert!(d.explain().unwrap().contains("arity"));
    }

    #[test]
    fn diagnostic_explain_unknown() {
        let d = Diagnostic::error("UNKNOWN_CODE", "test", None);
        assert!(d.explain().is_none());
    }

    // ── explain() exhaustiveness ────────────────────────────────────────

    #[test]
    fn all_codes_have_explanations() {
        let all = [
            codes::ARITY,
            codes::INVALID_ENUM,
            codes::EMPTY_FIELD_DATA,
            codes::STRING_TOO_SHORT,
            codes::STRING_TOO_LONG,
            codes::EXPECTED_INTEGER,
            codes::EXPECTED_NUMERIC,
            codes::EXPECTED_CHAR,
            codes::OUT_OF_RANGE,
            codes::ROUNDING_VIOLATION,
            codes::PROFILE_CONSTRAINT,
            codes::PRINTER_GATE,
            codes::MEDIA_MODE_UNSUPPORTED,
            codes::REQUIRED_MISSING,
            codes::REQUIRED_EMPTY,
            codes::REQUIRED_COMMAND,
            codes::INCOMPATIBLE_COMMAND,
            codes::ORDER_BEFORE,
            codes::ORDER_AFTER,
            codes::FIELD_DATA_WITHOUT_ORIGIN,
            codes::EMPTY_LABEL,
            codes::FIELD_NOT_CLOSED,
            codes::ORPHANED_FIELD_SEPARATOR,
            codes::HOST_COMMAND_IN_LABEL,
            codes::DUPLICATE_FIELD_NUMBER,
            codes::POSITION_OUT_OF_BOUNDS,
            codes::UNKNOWN_FONT,
            codes::INVALID_HEX_ESCAPE,
            codes::REDUNDANT_STATE,
            codes::SERIALIZATION_WITHOUT_FIELD_NUMBER,
            codes::GF_DATA_LENGTH_MISMATCH,
            codes::GF_BOUNDS_OVERFLOW,
            codes::GF_MEMORY_EXCEEDED,
            codes::MISSING_EXPLICIT_DIMENSIONS,
            codes::BARCODE_INVALID_CHAR,
            codes::BARCODE_DATA_LENGTH,
            codes::NOTE,
            codes::PARSER_NO_LABELS,
            codes::PARSER_INVALID_COMMAND,
            codes::PARSER_UNKNOWN_COMMAND,
            codes::PARSER_MISSING_TERMINATOR,
            codes::PARSER_MISSING_FIELD_SEPARATOR,
            codes::PARSER_FIELD_DATA_INTERRUPTED,
            codes::PARSER_STRAY_CONTENT,
        ];
        for code in &all {
            assert!(
                explain(code).is_some(),
                "diagnostic code {code} has no explain() entry"
            );
        }
    }

    // ── Eq / PartialEq ─────────────────────────────────────────────────

    #[test]
    fn diagnostic_eq() {
        let a = Diagnostic::error(codes::ARITY, "msg", Some(Span::new(0, 5)));
        let b = Diagnostic::error(codes::ARITY, "msg", Some(Span::new(0, 5)));
        assert_eq!(a, b);
    }

    #[test]
    fn diagnostic_ne_different_id() {
        let a = Diagnostic::error(codes::ARITY, "msg", None);
        let b = Diagnostic::error(codes::NOTE, "msg", None);
        assert_ne!(a, b);
    }

    // ── Serde round-trip ────────────────────────────────────────────────

    #[test]
    fn diagnostic_serde_roundtrip() {
        let d = Diagnostic::error(codes::ARITY, "test message", Some(Span::new(10, 20)));
        let json = serde_json::to_string(&d).unwrap();
        let d2: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn diagnostic_serde_omits_none_span() {
        let d = Diagnostic::error(codes::ARITY, "test", None);
        let json = serde_json::to_string(&d).unwrap();
        assert!(
            !json.contains("span"),
            "None span should be omitted: {json}"
        );
        assert!(
            !json.contains("context"),
            "None context should be omitted: {json}"
        );
    }

    // ── Context ───────────────────────────────────────────────────────────

    #[test]
    fn diagnostic_with_context() {
        let d = Diagnostic::error(codes::ARITY, "too many", None).with_context(BTreeMap::from([
            ("command".into(), "^PW".into()),
            ("arity".into(), "1".into()),
            ("actual".into(), "3".into()),
        ]));
        assert!(d.context.is_some());
        let ctx = d.context.as_ref().unwrap();
        assert_eq!(ctx.get("command").unwrap(), "^PW");
        assert_eq!(ctx.get("arity").unwrap(), "1");
        assert_eq!(ctx.get("actual").unwrap(), "3");
    }

    #[test]
    fn diagnostic_context_serde_roundtrip() {
        let d = Diagnostic::error(codes::OUT_OF_RANGE, "out of range", Some(Span::new(0, 5)))
            .with_context(BTreeMap::from([
                ("command".into(), "^PW".into()),
                ("min".into(), "1".into()),
                ("max".into(), "32000".into()),
            ]));
        let json = serde_json::to_string(&d).unwrap();
        assert!(
            json.contains("context"),
            "context should be serialized: {json}"
        );
        let d2: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn diagnostic_context_deterministic_order() {
        let d = Diagnostic::error(codes::ARITY, "test", None).with_context(BTreeMap::from([
            ("z_last".into(), "1".into()),
            ("a_first".into(), "2".into()),
            ("m_middle".into(), "3".into()),
        ]));
        let json = serde_json::to_string(&d).unwrap();
        let a_pos = json.find("a_first").unwrap();
        let m_pos = json.find("m_middle").unwrap();
        let z_pos = json.find("z_last").unwrap();
        assert!(
            a_pos < m_pos && m_pos < z_pos,
            "BTreeMap should serialize in alphabetical key order: {json}"
        );
    }
}
