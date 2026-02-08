use serde::{Deserialize, Serialize};
use zpl_toolchain_diagnostics::Span;

/// A parsed ZPL abstract syntax tree, consisting of one or more labels.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Ast {
    /// Ordered list of labels found in the input.
    pub labels: Vec<Label>,
}

/// A single ZPL label, delimited by `^XA` and `^XZ`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Label {
    /// Ordered list of nodes within this label.
    pub nodes: Vec<Node>,
}

/// A node in the ZPL AST representing a command, field data, raw payload, or trivia.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum Node {
    /// A ZPL command (e.g., `^FO`, `^PW`, `~DG`).
    Command {
        /// Canonical command code including leader (e.g., `"^FO"`).
        code: String,
        /// Parsed arguments for this command.
        args: Vec<ArgSlot>,
        /// Source span of the entire command.
        span: Span,
    },
    /// Field data content (text between ^FD/^FV and ^FS).
    FieldData {
        /// The raw text content of the field (after ^FD/^FV, before ^FS).
        content: String,
        /// Whether ^FH hex escapes have been applied.
        hex_escaped: bool,
        /// Source span of the field data content.
        span: Span,
    },
    /// Raw binary/hex payload (e.g., graphic data after ^GF or ~DG header).
    RawData {
        /// The command code that initiated the raw payload (e.g., `"^GF"`).
        command: String,
        /// The raw payload data, if any was collected.
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        /// Source span of the raw data content.
        span: Span,
    },
    /// Preserved trivia: comments, whitespace, content outside labels.
    Trivia {
        /// The trivia text content.
        text: String,
        /// Source span of the trivia.
        span: Span,
    },
}

/// A single argument slot in a parsed ZPL command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArgSlot {
    /// Spec-defined parameter name, if known from the signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Whether this argument was provided, empty, or absent.
    pub presence: Presence,
    /// The raw string value of the argument, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Indicates whether a command argument was provided, left empty, or absent.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    /// Argument was not present in the source at all.
    #[default]
    Unset,
    /// Argument position existed but was empty (e.g., `^FO,100`).
    Empty,
    /// Argument was provided with a value.
    Value,
}
