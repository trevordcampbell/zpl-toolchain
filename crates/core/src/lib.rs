//! ZPL toolchain core library.
//!
//! Provides parsing, validation, and emission of ZPL (Zebra Programming
//! Language) label code.  The main entry points are [`parse_str`] for parsing,
//! [`validate_with_profile`] for validation, and [`emit_zpl`] for formatted
//! output.

#![warn(missing_docs)]

/// ZPL grammar: lexer, parser, AST, emitter, and related utilities.
pub mod grammar;
/// Hex escape processing for `^FH` field data.
pub mod hex_escape;
/// AST validation against spec tables and printer profiles.
pub mod validate;

// ── Convenience re-exports ──────────────────────────────────────────────────
// Flat imports for the most common entry points. The full module paths
// remain available for less common types.

// Parser
pub use grammar::parser::{ParseResult, parse_str, parse_with_tables};

// AST
pub use grammar::ast::{ArgSlot, Ast, Label, Node, Presence};

// Emitter
pub use grammar::emit::{EmitConfig, Indent, emit_zpl, strip_spans};

// Diagnostics (re-exported from the diagnostics crate)
pub use grammar::diag::{Diagnostic, Severity, Span, codes};

// Validator
pub use validate::{ValidationResult, validate_with_profile};

// Tables
pub use grammar::tables::ParserTables;

// Serialization helpers
pub use grammar::dump::to_pretty_json;
