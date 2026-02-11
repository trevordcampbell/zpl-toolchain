//! Typed source representation for per-command JSONC spec files.
//!
//! These structs mirror the JSONC schema (camelCase) and are used for
//! early typed deserialization in the spec-compiler pipeline.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zpl_toolchain_spec_tables::{
    Arg, ArgUnion, CommandCategory, CommandScope, Composite, Constraint, Effects, Example,
    Placement, Plane, Signature, Stability,
};

fn default_scope_opt() -> Option<CommandScope> {
    Some(CommandScope::Field)
}

/// Top-level structure of a per-command JSONC file.
#[derive(Debug, Clone, Deserialize)]
pub struct SourceSpecFile {
    /// Optional file-level version identifier.
    #[serde(default)]
    pub version: Option<String>,
    /// The spec schema version this file conforms to (e.g. `"1.1.1"`).
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    /// The command definitions contained in this spec file.
    pub commands: Vec<SourceCommand>,
}

/// A single command entry as authored in JSONC.
///
/// Uses `rename_all = "camelCase"` for consistency. Fields that remain
/// snake_case in the JSONC spec files have explicit `#[serde(rename)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCommand {
    // Identity
    /// Multi-code opcodes (e.g. `["^FD", "~FD"]`). Preferred over `code`+`aliases`.
    #[serde(default)]
    pub codes: Option<Vec<String>>,
    /// Single opcode (legacy, e.g. `"^FD"`). Use `codes` for multi-code commands.
    #[serde(default)]
    pub code: Option<String>,
    /// Legacy alias opcodes (e.g. `["~FD"]`). Use `codes` for new specs.
    #[serde(default)]
    pub aliases: Option<Vec<String>>,

    // Metadata
    /// Human-readable command name (e.g. `"Field Data"`).
    #[serde(default)]
    pub name: Option<String>,
    /// Functional category (e.g. printing, barcode, graphics).
    #[serde(default)]
    pub category: Option<CommandCategory>,
    /// Print plane this command targets (e.g. ZPL II, EPL).
    #[serde(default)]
    pub plane: Option<Plane>,
    /// Scope in which this command is valid (e.g. label, format, global).
    #[serde(default = "default_scope_opt")]
    pub scope: Option<CommandScope>,
    /// Explicit placement permissions (inside/outside ^XA/^XZ).
    #[serde(default)]
    pub placement: Option<Placement>,
    /// Free-text documentation / description of the command.
    #[serde(default)]
    pub docs: Option<String>,

    // Arguments
    /// Number of positional parameters this command accepts.
    pub arity: u32,
    /// Signature template describing parameter order and joining convention.
    #[serde(default)]
    pub signature: Option<Signature>,
    /// Per-opcode signature overrides (keyed by opcode string).
    #[serde(default)]
    pub signature_overrides: Option<HashMap<String, Signature>>,
    /// Composite argument groups that bundle multiple args under one name.
    #[serde(default)]
    pub composites: Option<Vec<Composite>>,
    /// Positional argument definitions (may include `OneOf` union variants).
    #[serde(default)]
    pub args: Option<Vec<ArgUnion>>,
    /// Freeform default-value overrides. Stays as `serde_json::Value` because the
    /// schema defines no specific properties (`additionalProperties: true`) and no
    /// pipeline code inspects its contents — it is only passed through to the output.
    #[serde(default)]
    pub defaults: Option<serde_json::Value>,
    /// Unit of measurement for the command's arguments (e.g. `"dots"`).
    #[serde(default)]
    pub units: Option<String>,

    // Flags (snake_case in JSONC — explicit rename overrides camelCase default)
    /// If `true`, the command carries a raw (non-parsed) payload.
    #[serde(default, rename = "raw_payload")]
    pub raw_payload: bool,
    /// If `true`, this command consumes field data from the data stream.
    #[serde(default, rename = "field_data")]
    pub field_data: bool,
    /// If `true`, this command opens a new field context.
    #[serde(default, rename = "opens_field")]
    pub opens_field: bool,
    /// If `true`, this command closes the current field context.
    #[serde(default, rename = "closes_field")]
    pub closes_field: bool,
    /// If `true`, hex escape sequences are interpreted as a modifier.
    #[serde(default, rename = "hex_escape_modifier")]
    pub hex_escape_modifier: bool,
    /// If `true`, this command takes a field number parameter.
    #[serde(default, rename = "field_number")]
    pub field_number: bool,
    /// If `true`, this command relates to serialization control.
    #[serde(default)]
    pub serialization: bool,
    /// If `true`, this command must appear within an open field context.
    #[serde(default, rename = "requires_field")]
    pub requires_field: bool,

    // Validation
    /// Validation constraints for this command (ordering, compatibility, etc.).
    #[serde(default)]
    pub constraints: Option<Vec<Constraint>>,
    /// Printer model gates that restrict which printers support this command.
    #[serde(default)]
    pub printer_gates: Option<Vec<String>>,

    // Effects & versioning
    /// Side effects this command produces (e.g. setting state variables).
    #[serde(default)]
    pub effects: Option<Effects>,
    /// Rules governing how this command interacts with field data.
    #[serde(default)]
    pub field_data_rules: Option<zpl_toolchain_spec_tables::FieldDataRules>,
    /// Firmware version when this command was introduced.
    #[serde(default)]
    pub since: Option<String>,
    /// Whether this command is deprecated.
    #[serde(default)]
    pub deprecated: Option<bool>,
    /// Firmware version when this command was deprecated.
    #[serde(default)]
    pub deprecated_since: Option<String>,
    /// Stability level of this command (stable, experimental, etc.).
    #[serde(default)]
    pub stability: Option<Stability>,

    // Examples
    /// Usage examples for this command.
    #[serde(default)]
    pub examples: Option<Vec<Example>>,
    /// Freeform extension/vendor data. Stays as `serde_json::Value` because the
    /// schema defines no specific properties (`additionalProperties: true`) and this
    /// field is a catch-all for vendor/experimental metadata.
    #[serde(default)]
    pub extras: Option<serde_json::Value>,
}

impl SourceCommand {
    /// Returns the canonical (first) opcode for this command.
    pub fn canonical_code(&self) -> Option<String> {
        if let Some(codes) = &self.codes {
            codes.first().cloned()
        } else {
            self.code.clone()
        }
    }

    /// Returns all opcodes (codes + legacy code/aliases).
    pub fn all_codes(&self) -> Vec<String> {
        if let Some(codes) = &self.codes {
            codes.clone()
        } else {
            let mut v = Vec::new();
            if let Some(c) = &self.code {
                v.push(c.clone());
            }
            if let Some(a) = &self.aliases {
                v.extend(a.iter().cloned());
            }
            v
        }
    }

    /// Whether this is a structural command (arity 0 or field_data).
    pub fn is_structural(&self) -> bool {
        self.arity == 0 || self.field_data
    }

    /// Extract signature params as a Vec<String>.
    pub fn signature_params(&self) -> Vec<String> {
        self.signature
            .as_ref()
            .map(|s| s.params.clone())
            .unwrap_or_default()
    }

    /// Extract arg keys from args (handles oneOf unions).
    ///
    /// For `OneOf` unions, returns one representative key per position
    /// (the first non-empty key among the alternatives) to avoid
    /// false-positive duplicate-key errors.
    pub fn arg_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let Some(args) = &self.args {
            for item in args {
                match item {
                    ArgUnion::Single(arg) => {
                        if let Some(k) = &arg.key
                            && !k.is_empty()
                        {
                            keys.push(k.clone());
                        }
                    }
                    ArgUnion::OneOf { one_of } => {
                        // Take the first non-empty key as representative for this position
                        if let Some(k) = one_of
                            .iter()
                            .find_map(|a| a.key.as_ref().filter(|k| !k.is_empty()))
                        {
                            keys.push(k.clone());
                        }
                    }
                }
            }
        }
        keys
    }

    /// Extract ALL arg keys from args (including all alternatives in OneOf unions).
    /// Used for signature param validation where we need to know every possible key.
    pub fn all_arg_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let Some(args) = &self.args {
            for item in args {
                let args_to_check: Vec<&Arg> = match item {
                    ArgUnion::OneOf { one_of } => one_of.iter().collect(),
                    ArgUnion::Single(arg) => vec![arg.as_ref()],
                };
                for arg in args_to_check {
                    if let Some(k) = &arg.key
                        && !k.is_empty()
                    {
                        keys.push(k.clone());
                    }
                }
            }
        }
        keys
    }
}
