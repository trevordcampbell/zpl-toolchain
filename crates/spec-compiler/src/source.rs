//! Typed source representation for per-command JSONC spec files.
//!
//! These structs mirror the JSONC schema (camelCase) and are used for
//! early typed deserialization in the spec-compiler pipeline.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zpl_toolchain_spec_tables::{
    Arg, ArgUnion, CommandCategory, CommandScope, Composite, Constraint, Effects, Example, Plane,
    Signature, Stability,
};

/// Top-level structure of a per-command JSONC file.
#[derive(Debug, Clone, Deserialize)]
pub struct SourceSpecFile {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
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
    #[serde(default)]
    pub codes: Option<Vec<String>>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,

    // Metadata
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<CommandCategory>,
    #[serde(default)]
    pub plane: Option<Plane>,
    #[serde(default)]
    pub scope: Option<CommandScope>,
    #[serde(default)]
    pub docs: Option<String>,

    // Arguments
    pub arity: u32,
    #[serde(default)]
    pub signature: Option<Signature>,
    #[serde(default)]
    pub signature_overrides: Option<HashMap<String, Signature>>,
    #[serde(default)]
    pub composites: Option<Vec<Composite>>,
    #[serde(default)]
    pub args: Option<Vec<ArgUnion>>,
    /// Freeform default-value overrides. Stays as `serde_json::Value` because the
    /// schema defines no specific properties (`additionalProperties: true`) and no
    /// pipeline code inspects its contents — it is only passed through to the output.
    #[serde(default)]
    pub defaults: Option<serde_json::Value>,
    #[serde(default)]
    pub units: Option<String>,

    // Flags (snake_case in JSONC — explicit rename overrides camelCase default)
    #[serde(default, rename = "raw_payload")]
    pub raw_payload: bool,
    #[serde(default, rename = "field_data")]
    pub field_data: bool,
    #[serde(default, rename = "opens_field")]
    pub opens_field: bool,
    #[serde(default, rename = "closes_field")]
    pub closes_field: bool,
    #[serde(default, rename = "hex_escape_modifier")]
    pub hex_escape_modifier: bool,
    #[serde(default, rename = "field_number")]
    pub field_number: bool,
    #[serde(default)]
    pub serialization: bool,
    #[serde(default, rename = "requires_field")]
    pub requires_field: bool,

    // Validation
    #[serde(default)]
    pub constraints: Option<Vec<Constraint>>,
    #[serde(default)]
    pub printer_gates: Option<Vec<String>>,

    // Effects & versioning
    #[serde(default)]
    pub effects: Option<Effects>,
    #[serde(default)]
    pub field_data_rules: Option<zpl_toolchain_spec_tables::FieldDataRules>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub deprecated: Option<bool>,
    #[serde(default)]
    pub deprecated_since: Option<String>,
    #[serde(default)]
    pub stability: Option<Stability>,

    // Examples
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
