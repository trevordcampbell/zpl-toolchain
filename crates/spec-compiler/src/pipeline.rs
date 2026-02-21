//! Spec-compiler build pipeline: load → validate → generate.
//!
//! Each function is pure (input → output), testable independently.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::source::{SourceCommand, SourceSpecFile};
use crate::{build_opcode_trie, parse_jsonc};
use zpl_toolchain_spec_tables::TABLE_FORMAT_VERSION;

// ─── Load ───────────────────────────────────────────────────────────────────

/// Result of loading spec files.
pub struct LoadResult {
    /// The parsed command entries loaded from spec files.
    pub commands: Vec<SourceCommand>,
    /// The set of schema versions encountered across all spec files.
    pub schema_versions: BTreeSet<String>,
}

/// Load and parse all per-command JSONC files from `spec_dir/commands/`.
pub fn load_spec_files(spec_dir: &Path) -> Result<LoadResult> {
    let mut commands = Vec::new();
    let mut schema_versions = BTreeSet::new();
    let commands_dir = spec_dir.join("commands");

    if !commands_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "spec/commands directory not found; please provide per-command JSONC files"
        ));
    }

    for entry_result in walkdir::WalkDir::new(&commands_dir) {
        let entry = entry_result.map_err(|e| {
            let path_info = e.path().map(|p| p.display().to_string());
            anyhow::anyhow!(
                "error reading spec directory{}: {}",
                path_info
                    .as_deref()
                    .map_or(String::new(), |p| format!(" at '{}'", p)),
                e,
            )
        })?;
        if entry.file_type().is_file() && entry.path().extension() == Some(OsStr::new("jsonc")) {
            let text = std::fs::read_to_string(entry.path())?;
            let value = parse_jsonc(&text)?;

            // Extract schema version before typed deserialization
            if let Some(sv) = value.get("schemaVersion").and_then(|x| x.as_str()) {
                schema_versions.insert(sv.to_string());
            }

            // Deserialize into typed struct
            let spec_file: SourceSpecFile = serde_json::from_value(value)
                .map_err(|e| anyhow::anyhow!("parsing {:?}: {}", entry.path(), e))?;

            commands.extend(spec_file.commands);
        }
    }

    // Sort commands by canonical code for deterministic output regardless of
    // filesystem readdir ordering (WalkDir does not guarantee order).
    commands.sort_by(|a, b| {
        a.canonical_code()
            .unwrap_or_default()
            .cmp(&b.canonical_code().unwrap_or_default())
    });

    Ok(LoadResult {
        commands,
        schema_versions,
    })
}

// ─── Cross-field validation ─────────────────────────────────────────────────

/// A non-fatal validation error for a command.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The command opcode that triggered the validation errors.
    pub code: String,
    /// Human-readable descriptions of each validation issue found.
    pub errors: Vec<String>,
}

/// Finding emitted by `note-audit` for note-constraint quality checks.
#[derive(Debug, Clone, Serialize)]
pub struct NoteAuditFinding {
    /// Canonical command code.
    pub code: String,
    /// Severity level for this audit finding.
    pub level: String,
    /// Constraint location within the command spec.
    pub location: String,
    /// Human-readable finding message.
    pub message: String,
}

fn message_looks_conditional(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    [
        "only when",
        "only if",
        "supported only",
        "available only",
        "only on",
        "only for",
        "requires firmware",
        "requires profile",
        "if ",
        " when ",
    ]
    .iter()
    .any(|needle| m.contains(needle))
}

fn message_looks_explanatory(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    [
        "sets defaults for subsequent",
        "returns",
        "is processed",
        "remains active until",
        "can improve throughput",
        "extension of",
        "for backward-compatibility",
    ]
    .iter()
    .any(|needle| m.contains(needle))
}

/// Audit note constraints to identify obvious conditionalization/surface opportunities.
pub fn audit_notes(commands: &[SourceCommand]) -> Vec<NoteAuditFinding> {
    let mut findings = Vec::new();
    for command in commands {
        let code = command
            .canonical_code()
            .unwrap_or_else(|| "<unknown>".to_string());
        let Some(constraints) = command.constraints.as_ref() else {
            continue;
        };

        for (index, constraint) in constraints.iter().enumerate() {
            if constraint.kind != zpl_toolchain_spec_tables::ConstraintKind::Note {
                continue;
            }
            let location = format!("constraints[{index}]");
            if constraint.message.trim().is_empty() {
                findings.push(NoteAuditFinding {
                    code: code.clone(),
                    level: "error".to_string(),
                    location: location.clone(),
                    message: "note constraint message is empty".to_string(),
                });
                continue;
            }

            if constraint.expr.as_deref().is_none()
                && message_looks_conditional(&constraint.message)
            {
                findings.push(NoteAuditFinding {
                    code: code.clone(),
                    level: "warn".to_string(),
                    location: location.clone(),
                    message:
                        "note message looks conditional but has no expr (consider when:/before:/after:)"
                            .to_string(),
                });
            }

            if constraint.audience.is_none() && message_looks_explanatory(&constraint.message) {
                findings.push(NoteAuditFinding {
                    code: code.clone(),
                    level: "info".to_string(),
                    location,
                    message:
                        "note appears explanatory; consider audience=contextual to keep problem lists focused"
                            .to_string(),
                });
            }
        }
    }
    findings
}

/// Load profile schema and return the set of valid field paths.
///
/// Logs warnings to stderr if the schema file is missing, malformed, or
/// lacks the expected `fields` array — these conditions silently disable
/// profile constraint cross-validation and should be surfaced to spec authors.
fn load_profile_field_paths(spec_dir: &Path) -> HashSet<String> {
    let schema_path = spec_dir.join("schema/profile.schema.jsonc");
    let mut paths = HashSet::new();
    match std::fs::read_to_string(&schema_path) {
        Ok(raw) => {
            let stripped = crate::strip_jsonc(&raw);
            match serde_json::from_str::<serde_json::Value>(&stripped) {
                Ok(val) => {
                    if let Some(fields) = val.get("fields").and_then(|f| f.as_array()) {
                        for field in fields {
                            if let Some(p) = field.get("path").and_then(|p| p.as_str())
                                && !p.is_empty()
                            {
                                paths.insert(p.to_string());
                            }
                        }
                    } else {
                        eprintln!("warn: {} missing 'fields' array", schema_path.display());
                    }
                }
                Err(e) => eprintln!("warn: failed to parse {}: {}", schema_path.display(), e),
            }
        }
        Err(e) => eprintln!("warn: could not read {}: {}", schema_path.display(), e),
    }
    paths
}

/// Visit all concrete `Arg` values inside a slice of `ArgUnion`, calling `f(index, arg)` for each.
fn visit_args<F>(args: &[zpl_toolchain_spec_tables::ArgUnion], mut f: F)
where
    F: FnMut(usize, &zpl_toolchain_spec_tables::Arg),
{
    for (idx, item) in args.iter().enumerate() {
        match item {
            zpl_toolchain_spec_tables::ArgUnion::OneOf { one_of } => {
                for arg in one_of {
                    f(idx, arg);
                }
            }
            zpl_toolchain_spec_tables::ArgUnion::Single(arg) => {
                f(idx, arg);
            }
        }
    }
}

/// Detect duplicate opcodes across all commands.
fn validate_duplicate_opcodes(commands: &[SourceCommand]) -> Vec<ValidationError> {
    let mut results = Vec::new();
    let mut seen_codes: HashMap<String, String> = HashMap::new(); // code -> first canonical code
    for cmd in commands {
        let owner = cmd
            .canonical_code()
            .unwrap_or_else(|| "<unknown>".to_string());
        for code in cmd.all_codes() {
            if let Some(prev_owner) = seen_codes.get(&code) {
                results.push(ValidationError {
                    code: code.clone(),
                    errors: vec![format!(
                        "duplicate opcode '{}': already defined by '{}', also in '{}'",
                        code, prev_owner, owner
                    )],
                });
            } else {
                seen_codes.insert(code, owner.clone());
            }
        }
    }
    results
}

/// Validate arity consistency between signature params and args for a single command.
fn validate_command_arity(cmd: &SourceCommand, errors: &mut Vec<String>) {
    let arity = cmd.arity as usize;
    let params = cmd.signature_params();
    let has_sig = cmd.signature.is_some();
    let has_args = cmd.args.is_some();

    if has_sig && params.len() != arity {
        errors.push(format!(
            "signature.params length {} != arity {}",
            params.len(),
            arity
        ));
    }
    if has_args
        && let Some(args) = &cmd.args
        && args.len() != arity
    {
        errors.push(format!("args length {} != arity {}", args.len(), arity));
    }
}

/// Validate arg keys ↔ signature params cross-references and detect duplicate arg keys.
fn validate_signature_linkage(cmd: &SourceCommand, errors: &mut Vec<String>) {
    let params = cmd.signature_params();
    let arg_keys = cmd.arg_keys();
    let has_sig = cmd.signature.is_some();
    let has_args = cmd.args.is_some();

    // Arg keys referenced in params
    if has_sig && has_args {
        let param_set: HashSet<String> = params.iter().cloned().collect();
        for k in &arg_keys {
            if !param_set.contains(k) {
                errors.push(format!(
                    "arg key '{}' not referenced in signature.params",
                    k
                ));
            }
        }
    }

    // Params reference args or composites (use all_arg_keys to include
    // every alternative in OneOf unions)
    if has_sig {
        let comp_names = composite_names(cmd);
        let all_keys = cmd.all_arg_keys();
        let all_key_set: HashSet<String> = all_keys.iter().cloned().collect();
        for p in &params {
            if !all_key_set.contains(p) && !comp_names.contains(p.as_str()) {
                errors.push(format!(
                    "signature param '{}' not found in args or composites",
                    p
                ));
            }
        }
    }

    // Duplicate arg keys
    {
        let mut seen = HashSet::new();
        let mut dupes = Vec::new();
        for k in &arg_keys {
            if !seen.insert(k.clone()) {
                dupes.push(k.clone());
            }
        }
        if !dupes.is_empty() {
            errors.push(format!("duplicate arg keys: {}", dupes.join(", ")));
        }
    }
}

/// Validate signatureOverrides: keys must be command opcodes, params must exist in args/composites.
fn validate_signature_overrides(cmd: &SourceCommand, errors: &mut Vec<String>) {
    if let Some(overrides) = &cmd.signature_overrides {
        // Use all_arg_keys to include every alternative in OneOf unions
        let all_keys = cmd.all_arg_keys();
        let arg_key_set: HashSet<String> = all_keys.iter().cloned().collect();
        let comp_names = composite_names(cmd);
        let cmd_codes: HashSet<String> = cmd.all_codes().into_iter().collect();
        for (opcode, sig) in overrides {
            // Override key must be one of this command's opcodes
            if !cmd_codes.contains(opcode) {
                errors.push(format!(
                    "signatureOverrides key '{}' is not one of this command's opcodes ({:?})",
                    opcode,
                    cmd.all_codes()
                ));
            }
            for s in &sig.params {
                if !arg_key_set.contains(s) && !comp_names.contains(s.as_str()) {
                    errors.push(format!(
                        "signatureOverrides[{}] param '{}' not found in args or composites",
                        opcode, s
                    ));
                }
            }
        }
    }
}

/// Validate arg-level hygiene, type checks, and defaultFrom references for a single command.
fn validate_arg_hygiene(
    cmd: &SourceCommand,
    all_codes: &HashSet<String>,
    has_effects: &HashMap<String, bool>,
    effects_sets: &HashMap<String, HashSet<String>>,
    errors: &mut Vec<String>,
) {
    if let Some(args) = &cmd.args {
        visit_args(args, |idx, arg| {
            // Empty name check
            if let Some(n) = &arg.name
                && n.trim().is_empty()
            {
                errors.push(format!("arg[{}] has empty name", idx));
            }

            // Enum type must have non-empty enum values
            if arg.r#type == "enum" {
                match &arg.r#enum {
                    None => errors.push(format!(
                        "arg[{}] has type 'enum' but no enum values defined",
                        idx
                    )),
                    Some(v) if v.is_empty() => {
                        errors.push(format!("arg[{}] has type 'enum' with empty enum list", idx))
                    }
                    _ => {}
                }
            }

            // Range validity: min <= max
            if let Some(range) = &arg.range
                && range[0] > range[1]
            {
                errors.push(format!(
                    "arg[{}] range [{}, {}] is invalid (min > max)",
                    idx, range[0], range[1]
                ));
            }

            // defaultFrom must reference a known command with effects.sets
            if let Some(df) = &arg.default_from {
                if !df.starts_with('^') && !df.starts_with('~') {
                    errors.push(format!(
                        "arg[{}] defaultFrom '{}' must start with ^ or ~",
                        idx, df
                    ));
                }
                if !all_codes.contains(df) {
                    errors.push(format!(
                        "arg[{}] defaultFrom '{}' references unknown command",
                        idx, df
                    ));
                } else if has_effects.get(df) == Some(&false) {
                    errors.push(format!(
                        "arg[{}] defaultFrom '{}' references command with no effects.sets",
                        idx, df
                    ));
                }

                if arg.default_from_state_key.is_none() {
                    let hint = effects_sets
                        .get(df)
                        .map(|keys| format!(" (choices: {:?})", keys))
                        .unwrap_or_default();
                    errors.push(format!(
                        "arg[{}] defaultFrom '{}' requires defaultFromStateKey{}",
                        idx, df, hint
                    ));
                }

                if let Some(state_key) = &arg.default_from_state_key {
                    if let Some(effect_keys) = effects_sets.get(df) {
                        if !effect_keys.contains(state_key) {
                            errors.push(format!(
                                "arg[{}] defaultFromStateKey '{}' is not in effects.sets of '{}': {:?}",
                                idx, state_key, df, effect_keys
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "arg[{}] defaultFromStateKey '{}' references '{}' which has no effects.sets",
                            idx, state_key, df
                        ));
                    }
                }
            }
        });
    }
}

/// Validate constraint expressions, target references, and messages for a single command.
///
/// Note: kind and severity are validated at deserialization time via enums.
/// The canonical list of constraint kinds is defined in
/// [`ConstraintKind::ALL`](zpl_toolchain_spec_tables::ConstraintKind::ALL)
/// (the single source of truth). The JSONC schema at
/// `spec/schema/zpl-spec.schema.jsonc` mirrors this list for spec authoring;
/// a test (`constraint_kinds_match_schema`) validates they stay in sync.
/// Adding a new kind requires updating: (1) `ConstraintKind` enum + `ALL`,
/// (2) JSONC schema, (3) this validation block, (4) the validator in
/// `crates/core/src/validate/`.
fn validate_command_constraints_spec(
    cmd: &SourceCommand,
    all_codes: &HashSet<String>,
    errors: &mut Vec<String>,
) {
    if let Some(constraints) = &cmd.constraints {
        for (ci, constraint) in constraints.iter().enumerate() {
            let expr = constraint.expr.as_deref().unwrap_or("");

            // Validate expr grammar per kind
            match constraint.kind {
                zpl_toolchain_spec_tables::ConstraintKind::Order => {
                    if expr.is_empty() {
                        errors.push(format!(
                            "constraints[{}] order constraint requires expr",
                            ci
                        ));
                    } else if let Some(targets) = expr.strip_prefix("before:") {
                        validate_target_expr(targets, ci, errors);
                    } else if let Some(targets) = expr.strip_prefix("after:") {
                        validate_target_expr(targets, ci, errors);
                    } else {
                        errors.push(format!(
                            "constraints[{}].expr '{}' must start with 'before:' or 'after:'",
                            ci, expr
                        ));
                    }
                    if constraint.scope.is_none() {
                        errors.push(format!(
                            "constraints[{}] order constraint requires explicit scope",
                            ci
                        ));
                    }
                }
                zpl_toolchain_spec_tables::ConstraintKind::Requires
                | zpl_toolchain_spec_tables::ConstraintKind::Incompatible => {
                    if expr.is_empty() {
                        errors.push(format!(
                            "constraints[{}] {:?} constraint requires expr",
                            ci, constraint.kind
                        ));
                    } else {
                        validate_target_expr(expr, ci, errors);
                    }
                    if constraint.scope.is_none() {
                        errors.push(format!(
                            "constraints[{}] {:?} constraint requires explicit scope",
                            ci, constraint.kind
                        ));
                    }
                }
                zpl_toolchain_spec_tables::ConstraintKind::EmptyData => {
                    // No expr needed
                }
                // Range and Custom: no expr grammar to validate.
                // Range expr is freeform; Custom is escape-hatch.
                zpl_toolchain_spec_tables::ConstraintKind::Range
                | zpl_toolchain_spec_tables::ConstraintKind::Custom => {}
                zpl_toolchain_spec_tables::ConstraintKind::Note => {
                    validate_note_expr(expr, ci, errors);
                }
            }

            // Validate constraint target opcodes exist in the command set
            let targets = extract_constraint_targets(constraint);
            for target in &targets {
                if !all_codes.contains(target) {
                    errors.push(format!(
                        "constraints[{}] references unknown command '{}'",
                        ci, target
                    ));
                }
            }

            // Validate message is not empty
            if constraint.message.is_empty() {
                errors.push(format!("constraints[{}] missing or empty message", ci));
            }

            if constraint.audience.is_some()
                && constraint.kind != zpl_toolchain_spec_tables::ConstraintKind::Note
            {
                errors.push(format!(
                    "constraints[{}] audience is only supported for kind=note",
                    ci
                ));
            }
        }
    }
}

/// Extract `{key}` placeholders from a composite template string.
/// Returns placeholders in order of first occurrence.
pub fn extract_template_placeholders(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut i = 0;
    let bytes = template.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i + 1;
            i += 1;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            if i < bytes.len() {
                let key = String::from_utf8_lossy(&bytes[start..i]).to_string();
                if !key.is_empty() && seen.insert(key.clone()) {
                    out.push(key);
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Validate composites linkage: composite names must appear in signature params,
/// exposed args must exist in the command's args, and template placeholders
/// must match exposesArgs bidirectionally.
fn validate_composites_linkage(cmd: &SourceCommand, errors: &mut Vec<String>) {
    if let Some(comps) = &cmd.composites {
        let params = cmd.signature_params();
        // Use all_arg_keys so OneOf alternatives are valid composite targets.
        let all_arg_keys = cmd.all_arg_keys();
        for comp in comps {
            if !comp.name.is_empty() {
                if !params.is_empty() && !params.iter().any(|p| p == &comp.name) {
                    errors.push(format!(
                        "composite '{}' not referenced in signature.params",
                        comp.name
                    ));
                }
                for k in &comp.exposes_args {
                    if !all_arg_keys.contains(k) {
                        errors.push(format!(
                            "composite '{}' exposes arg '{}' not present in args",
                            comp.name, k
                        ));
                    }
                }
                // Template ↔ exposesArgs linkage
                let placeholders = extract_template_placeholders(&comp.template);
                let exposes_set: HashSet<&str> =
                    comp.exposes_args.iter().map(|s| s.as_str()).collect();
                for placeholder in &placeholders {
                    if !exposes_set.contains(placeholder.as_str()) {
                        errors.push(format!(
                            "composite '{}' template placeholder '{{{}}}' not in exposesArgs",
                            comp.name, placeholder
                        ));
                    }
                }
                for k in &comp.exposes_args {
                    if !placeholders.contains(k) {
                        errors.push(format!(
                            "composite '{}' exposes arg '{}' not used in template",
                            comp.name, k
                        ));
                    }
                }
            }
        }
    }
}

/// Validate effects: effects must have non-empty sets with no empty strings.
fn validate_effects(cmd: &SourceCommand, errors: &mut Vec<String>) {
    if let Some(effects) = &cmd.effects {
        if effects.sets.is_empty() {
            errors.push("effects declared but sets is empty".to_string());
        }
        for (si, s) in effects.sets.iter().enumerate() {
            if s.trim().is_empty() {
                errors.push(format!("effects.sets[{}] is empty string", si));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum StructuralBindingKey {
    Kind(zpl_toolchain_spec_tables::StructuralRuleKind),
    PositionAction(zpl_toolchain_spec_tables::PositionBoundsAction),
    FontAction(zpl_toolchain_spec_tables::FontReferenceAction),
    MediaTarget(zpl_toolchain_spec_tables::MediaModesTarget),
}

fn required_structural_bindings_for_code(code: &str) -> Option<&'static [StructuralBindingKey]> {
    use zpl_toolchain_spec_tables::FontReferenceAction as FA;
    use zpl_toolchain_spec_tables::MediaModesTarget as MT;
    use zpl_toolchain_spec_tables::PositionBoundsAction as PA;
    use zpl_toolchain_spec_tables::StructuralRuleKind as K;
    match code {
        "^FN" => Some(&[StructuralBindingKey::Kind(K::DuplicateFieldNumber)]),
        "^PW" => Some(&[StructuralBindingKey::PositionAction(PA::TrackWidth)]),
        "^LL" => Some(&[StructuralBindingKey::PositionAction(PA::TrackHeight)]),
        "^FO" | "^FT" => Some(&[
            StructuralBindingKey::PositionAction(PA::TrackFieldOrigin),
            StructuralBindingKey::PositionAction(PA::ValidateFieldOrigin),
        ]),
        "^A" => Some(&[StructuralBindingKey::FontAction(FA::Validate)]),
        "^CW" => Some(&[StructuralBindingKey::FontAction(FA::Register)]),
        "^MM" => Some(&[StructuralBindingKey::MediaTarget(MT::SupportedModes)]),
        "^MN" => Some(&[StructuralBindingKey::MediaTarget(MT::SupportedTracking)]),
        "^MT" => Some(&[StructuralBindingKey::MediaTarget(MT::PrintMethod)]),
        "^GF" => Some(&[
            StructuralBindingKey::Kind(K::GfDataLength),
            StructuralBindingKey::Kind(K::GfPreflightTracking),
        ]),
        _ => None,
    }
}

/// Validate schema-selected structural rules for commands that require structural checks.
fn validate_structural_rules_binding(cmd: &SourceCommand, errors: &mut Vec<String>) {
    let Some(code) = cmd.canonical_code() else {
        return;
    };
    let Some(required_rules) = required_structural_bindings_for_code(&code) else {
        return;
    };

    let configured_rules = cmd.structural_rules.as_deref().unwrap_or_default();
    let arity = cmd.arity as usize;

    for rule in configured_rules {
        match rule {
            zpl_toolchain_spec_tables::StructuralRule::DuplicateFieldNumber { arg_index }
            | zpl_toolchain_spec_tables::StructuralRule::FontReference { arg_index, .. }
            | zpl_toolchain_spec_tables::StructuralRule::MediaModes { arg_index, .. } => {
                if *arg_index >= arity {
                    errors.push(format!(
                        "structuralRules argIndex {} is out of range for command '{}' (arity {})",
                        arg_index, code, arity
                    ));
                }
            }
            zpl_toolchain_spec_tables::StructuralRule::GfDataLength {
                compression_arg_index,
                declared_byte_count_arg_index,
                data_arg_index,
            } => {
                for idx in [
                    compression_arg_index,
                    declared_byte_count_arg_index,
                    data_arg_index,
                ] {
                    if *idx >= arity {
                        errors.push(format!(
                            "structuralRules gfDataLength arg index {} is out of range for command '{}' (arity {})",
                            idx, code, arity
                        ));
                    }
                }
            }
            zpl_toolchain_spec_tables::StructuralRule::GfPreflightTracking {
                graphic_field_count_arg_index,
                bytes_per_row_arg_index,
            } => {
                for idx in [graphic_field_count_arg_index, bytes_per_row_arg_index] {
                    if *idx >= arity {
                        errors.push(format!(
                            "structuralRules gfPreflightTracking arg index {} is out of range for command '{}' (arity {})",
                            idx, code, arity
                        ));
                    }
                }
            }
            zpl_toolchain_spec_tables::StructuralRule::PositionBounds { .. } => {}
        }
    }

    let configured = configured_rules
        .iter()
        .map(|rule| match rule {
            zpl_toolchain_spec_tables::StructuralRule::DuplicateFieldNumber { .. } => {
                StructuralBindingKey::Kind(
                    zpl_toolchain_spec_tables::StructuralRuleKind::DuplicateFieldNumber,
                )
            }
            zpl_toolchain_spec_tables::StructuralRule::PositionBounds { action } => {
                StructuralBindingKey::PositionAction(*action)
            }
            zpl_toolchain_spec_tables::StructuralRule::FontReference { action, .. } => {
                StructuralBindingKey::FontAction(*action)
            }
            zpl_toolchain_spec_tables::StructuralRule::MediaModes { target, .. } => {
                StructuralBindingKey::MediaTarget(*target)
            }
            zpl_toolchain_spec_tables::StructuralRule::GfDataLength { .. } => {
                StructuralBindingKey::Kind(
                    zpl_toolchain_spec_tables::StructuralRuleKind::GfDataLength,
                )
            }
            zpl_toolchain_spec_tables::StructuralRule::GfPreflightTracking { .. } => {
                StructuralBindingKey::Kind(
                    zpl_toolchain_spec_tables::StructuralRuleKind::GfPreflightTracking,
                )
            }
        })
        .collect::<Vec<_>>();
    let configured_set: HashSet<StructuralBindingKey> = configured.into_iter().collect();

    for required in required_rules {
        if !configured_set.contains(required) {
            errors.push(format!(
                "structuralRules missing required rule '{:?}' for command '{}'",
                required, code
            ));
        }
    }

    for configured_rule in &configured_set {
        if !required_rules.contains(configured_rule) {
            errors.push(format!(
                "structuralRules includes unsupported structural rule '{:?}' for command '{}'",
                configured_rule, code
            ));
        }
    }
}

/// Validate profileConstraint.field references against the profile schema for a single command.
fn validate_profile_constraints_spec(
    cmd: &SourceCommand,
    profile_fields: &HashSet<String>,
    errors: &mut Vec<String>,
) {
    if !profile_fields.is_empty()
        && let Some(args) = &cmd.args
    {
        visit_args(args, |idx, arg| {
            if let Some(pc) = &arg.profile_constraint
                && !profile_fields.contains(&pc.field)
            {
                errors.push(format!(
                    "arg[{}] profileConstraint references unknown profile field '{}' \
                     (valid: {:?})",
                    idx, pc.field, profile_fields
                ));
            }
        });
    }
}

/// Validate cross-field consistency of all commands.
pub fn validate_cross_field(commands: &[SourceCommand], spec_dir: &Path) -> Vec<ValidationError> {
    let profile_fields = load_profile_field_paths(spec_dir);

    let all_codes: HashSet<String> = commands.iter().flat_map(|cmd| cmd.all_codes()).collect();

    let has_effects: HashMap<String, bool> = commands
        .iter()
        .flat_map(|cmd| {
            let has = cmd.effects.as_ref().is_some_and(|e| !e.sets.is_empty());
            cmd.all_codes().into_iter().map(move |c| (c, has))
        })
        .collect();
    let effects_sets: HashMap<String, HashSet<String>> = commands
        .iter()
        .flat_map(|cmd| {
            let keys: HashSet<String> = cmd
                .effects
                .as_ref()
                .map(|e| e.sets.iter().cloned().collect())
                .unwrap_or_default();
            cmd.all_codes().into_iter().map(move |c| (c, keys.clone()))
        })
        .collect();

    let mut results = validate_duplicate_opcodes(commands);

    for cmd in commands {
        let code = cmd.canonical_code().unwrap_or_default();
        let mut errors = Vec::new();

        validate_command_arity(cmd, &mut errors);
        validate_signature_linkage(cmd, &mut errors);
        validate_arg_hygiene(cmd, &all_codes, &has_effects, &effects_sets, &mut errors);
        validate_signature_overrides(cmd, &mut errors);
        validate_command_constraints_spec(cmd, &all_codes, &mut errors);
        validate_composites_linkage(cmd, &mut errors);
        validate_effects(cmd, &mut errors);
        validate_structural_rules_binding(cmd, &mut errors);
        validate_profile_constraints_spec(cmd, &profile_fields, &mut errors);

        if !errors.is_empty() {
            results.push(ValidationError { code, errors });
        }
    }

    results
}

/// Validate a pipe-separated target expression (e.g., "^FD|^FV").
/// Each target must start with ^ or ~.
fn validate_target_expr(targets: &str, constraint_idx: usize, errors: &mut Vec<String>) {
    if targets.is_empty() {
        errors.push(format!(
            "constraints[{}].expr has empty target list",
            constraint_idx
        ));
        return;
    }
    for target in targets.split('|') {
        let t = target.trim();
        if t.is_empty() {
            errors.push(format!(
                "constraints[{}].expr has empty target in pipe list",
                constraint_idx
            ));
        } else if !t.starts_with('^') && !t.starts_with('~') {
            errors.push(format!(
                "constraints[{}].expr target '{}' must start with ^ or ~",
                constraint_idx, t
            ));
        }
    }
}

/// Known when: predicate prefixes (must have non-empty suffix where applicable).
const WHEN_PREDICATE_PREFIXES: &[&str] = &[
    "arg:",
    "label:has:",
    "label:missing:",
    "profile:id:",
    "profile:dpi:",
    "profile:feature:",
    "profile:featureMissing:",
    "profile:firmware:",
    "profile:firmwareGte:",
    "profile:model:",
];

fn is_valid_when_predicate(token: &str) -> bool {
    let predicate = token.trim().strip_prefix('!').unwrap_or(token).trim();
    if predicate.is_empty() {
        return false;
    }
    WHEN_PREDICATE_PREFIXES
        .iter()
        .any(|p| predicate.starts_with(p) && predicate.len() > p.len())
        || (predicate.starts_with("arg:") && {
            // arg:keyPresent, arg:keyEmpty, arg:keyIsValue:V
            predicate.ends_with("Present")
                || predicate.ends_with("Empty")
                || predicate.contains("IsValue:")
        })
}

fn validate_when_predicate_terms(condition: &str, constraint_idx: usize, errors: &mut Vec<String>) {
    for disjunction in condition.split("||") {
        for term in disjunction.split("&&") {
            let token = term.trim();
            if token.is_empty() {
                errors.push(format!(
                    "constraints[{}].expr when: has empty term (surrounded by &&/||)",
                    constraint_idx
                ));
            } else if !is_valid_when_predicate(token) {
                errors.push(format!(
                    "constraints[{}].expr when: term '{}' is not a recognized predicate (use arg:, label:has:, label:missing:, or profile:* )",
                    constraint_idx, token
                ));
            }
        }
    }
}

fn validate_note_expr(expr: &str, constraint_idx: usize, errors: &mut Vec<String>) {
    if expr.is_empty() {
        return;
    }
    if let Some(targets) = expr.strip_prefix("after:first:") {
        validate_target_expr(targets, constraint_idx, errors);
        return;
    }
    if let Some(targets) = expr.strip_prefix("before:first:") {
        validate_target_expr(targets, constraint_idx, errors);
        return;
    }
    if let Some(targets) = expr.strip_prefix("after:") {
        validate_target_expr(targets, constraint_idx, errors);
        return;
    }
    if let Some(targets) = expr.strip_prefix("before:") {
        validate_target_expr(targets, constraint_idx, errors);
        return;
    }
    if let Some(condition) = expr.strip_prefix("when:") {
        let condition = condition.trim();
        if condition.is_empty() {
            errors.push(format!(
                "constraints[{}].expr has empty when: predicate",
                constraint_idx
            ));
        } else {
            validate_when_predicate_terms(condition, constraint_idx, errors);
        }
        return;
    }
    errors.push(format!(
        "constraints[{}].expr '{}' is not a recognized note expression prefix",
        constraint_idx, expr
    ));
}

/// Extract target opcodes from a constraint's expr field.
fn extract_constraint_targets(constraint: &zpl_toolchain_spec_tables::Constraint) -> Vec<String> {
    let expr = constraint.expr.as_deref().unwrap_or("");
    let targets_str = match constraint.kind {
        zpl_toolchain_spec_tables::ConstraintKind::Order => expr
            .strip_prefix("before:")
            .or_else(|| expr.strip_prefix("after:")),
        zpl_toolchain_spec_tables::ConstraintKind::Requires
        | zpl_toolchain_spec_tables::ConstraintKind::Incompatible => {
            if expr.is_empty() {
                None
            } else {
                Some(expr)
            }
        }
        zpl_toolchain_spec_tables::ConstraintKind::Note => expr
            .strip_prefix("after:first:")
            .or_else(|| expr.strip_prefix("before:first:"))
            .or_else(|| expr.strip_prefix("after:"))
            .or_else(|| expr.strip_prefix("before:")),
        _ => None,
    };
    match targets_str {
        Some(s) if !s.is_empty() => s
            .split('|')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn composite_names(cmd: &SourceCommand) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(comps) = &cmd.composites {
        for c in comps {
            if !c.name.is_empty() {
                names.insert(c.name.clone());
            }
        }
    }
    names
}

// ─── Generate parser tables ─────────────────────────────────────────────────

/// Generate parser_tables.json (includes the opcode trie inline).
pub fn generate_tables(
    commands: &[SourceCommand],
    schema_versions: &BTreeSet<String>,
) -> Result<serde_json::Value> {
    // BTreeSet is sorted ascending — use the highest (latest) version.
    // The CLI enforces a single schemaVersion invariant before calling into
    // generation, so this value is deterministic and represents the only
    // allowed schema version in the loaded tree.
    let schema_version = schema_versions
        .iter()
        .next_back()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    // Build typed command entries — signature, args, constraints, effects are
    // already typed on SourceCommand, so no serde_json::from_value() needed.
    let out_cmds: Vec<zpl_toolchain_spec_tables::CommandEntry> = commands
        .iter()
        .map(|cmd| zpl_toolchain_spec_tables::CommandEntry {
            codes: cmd.all_codes(),
            arity: cmd.arity,
            raw_payload: cmd.raw_payload,
            field_data: cmd.field_data,
            opens_field: cmd.opens_field,
            closes_field: cmd.closes_field,
            hex_escape_modifier: cmd.hex_escape_modifier,
            field_number: cmd.field_number,
            serialization: cmd.serialization,
            requires_field: cmd.requires_field,
            signature: cmd.signature.clone(),
            args: cmd.args.clone(),
            constraints: cmd.constraints.clone(),
            constraint_defaults: cmd.constraint_defaults.clone(),
            effects: cmd.effects.clone(),
            structural_rules: cmd.structural_rules.clone(),
            plane: cmd.plane,
            scope: cmd.scope,
            placement: cmd.placement.clone(),
            name: cmd.name.clone(),
            category: cmd.category,
            since: cmd.since.clone(),
            deprecated: cmd.deprecated,
            deprecated_since: cmd.deprecated_since.clone(),
            stability: cmd.stability,
            composites: cmd.composites.clone(),
            defaults: cmd.defaults.clone(),
            units: cmd.units.clone(),
            printer_gates: cmd.printer_gates.clone(),
            signature_overrides: cmd.signature_overrides.clone(),
            field_data_rules: cmd.field_data_rules.clone(),
            examples: cmd.examples.clone(),
        })
        .collect();

    let mut by_kind: BTreeMap<zpl_toolchain_spec_tables::StructuralRuleKind, BTreeSet<String>> =
        BTreeMap::new();
    let mut by_trigger: BTreeMap<zpl_toolchain_spec_tables::StructuralTrigger, BTreeSet<String>> =
        BTreeMap::new();
    let mut by_effect: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for cmd in &out_cmds {
        let Some(code) = cmd.codes.first().cloned() else {
            continue;
        };
        if let Some(rules) = cmd.structural_rules.as_ref() {
            for rule in rules {
                by_kind.entry(rule.kind()).or_default().insert(code.clone());
            }
        }
        if cmd.opens_field {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::OpensField)
                .or_default()
                .insert(code.clone());
        }
        if cmd.closes_field {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::ClosesField)
                .or_default()
                .insert(code.clone());
        }
        if cmd.field_data {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::FieldData)
                .or_default()
                .insert(code.clone());
        }
        if cmd.raw_payload {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::RawPayload)
                .or_default()
                .insert(code.clone());
        }
        if cmd.field_number {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::FieldNumber)
                .or_default()
                .insert(code.clone());
        }
        if cmd.serialization {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::Serialization)
                .or_default()
                .insert(code.clone());
        }
        if cmd.requires_field {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::RequiresField)
                .or_default()
                .insert(code.clone());
        }
        if cmd.hex_escape_modifier {
            by_trigger
                .entry(zpl_toolchain_spec_tables::StructuralTrigger::HexEscapeModifier)
                .or_default()
                .insert(code.clone());
        }

        if let Some(effects) = cmd.effects.as_ref() {
            for effect_key in &effects.sets {
                by_effect
                    .entry(effect_key.clone())
                    .or_default()
                    .insert(code.clone());
            }
        }
    }

    let structural_rule_index = zpl_toolchain_spec_tables::StructuralRuleIndex {
        by_kind: by_kind
            .into_iter()
            .map(|(k, set)| (k, set.into_iter().collect()))
            .collect(),
        by_trigger: by_trigger
            .into_iter()
            .map(|(k, set)| (k, set.into_iter().collect()))
            .collect(),
        by_effect: by_effect
            .into_iter()
            .map(|(k, set)| (k, set.into_iter().collect()))
            .collect(),
    };

    // Build opcode trie from raw JSON values (reuse existing function)
    let raw_cmds: Vec<serde_json::Value> = commands
        .iter()
        .map(|cmd| serde_json::json!({"codes": cmd.all_codes()}))
        .collect();
    let trie_json = build_opcode_trie(&raw_cmds);
    // Validate the trie deserializes correctly
    let _trie_root: zpl_toolchain_spec_tables::OpcodeTrieNode =
        serde_json::from_value(trie_json.clone())?;

    // Build the ParserTables through serde round-trip so that private
    // OnceLock cache fields are default-initialized correctly.
    let tables_value = serde_json::json!({
        "schemaVersion": schema_version,
        "formatVersion": TABLE_FORMAT_VERSION,
        "commands": serde_json::to_value(&out_cmds)?,
        "opcodeTrie": trie_json,
        "structuralRuleIndex": serde_json::to_value(&structural_rule_index)?,
    });

    // Verify the value deserializes into a valid ParserTables
    let _tables: zpl_toolchain_spec_tables::ParserTables =
        serde_json::from_value(tables_value.clone())?;

    Ok(tables_value)
}

// ─── Generate docs bundle ───────────────────────────────────────────────────

/// Generate `docs_bundle.json` — a per-code documentation view with signatures,
/// args, enum values, composites, format templates, and missing-field analysis.
///
/// **Not consumed at runtime** by the parser, validator, or CLI. This artifact
/// is generated for external tooling such as IDE plugins, documentation
/// generators, and the web playground.
pub fn generate_docs_bundle(
    commands: &[SourceCommand],
    schema_versions: &BTreeSet<String>,
    master_codes: &BTreeSet<String>,
) -> Result<serde_json::Value> {
    let mut docs_by_code = serde_json::Map::new();
    let mut present_code_set = BTreeSet::new();

    for cmd in commands {
        let code = match cmd.canonical_code() {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };
        let all_codes = cmd.all_codes();
        for c in &all_codes {
            present_code_set.insert(c.clone());
        }
        let params = cmd.signature_params();

        let mut entry = serde_json::Map::new();
        if let Some(sig) = &cmd.signature {
            entry.insert("signature".into(), serde_json::to_value(sig)?);
        }
        if let Some(args) = &cmd.args {
            entry.insert("args".into(), serde_json::to_value(args)?);
        }
        if let Some(docs) = &cmd.docs {
            entry.insert("docs".into(), serde_json::json!(docs));
        }
        if let Some(name) = &cmd.name {
            entry.insert("name".into(), serde_json::json!(name));
        }
        let effective_category = effective_command_category(cmd, &code);
        entry.insert("category".into(), serde_json::to_value(effective_category)?);
        entry.insert("scope".into(), serde_json::to_value(cmd.scope)?);

        // Stable anchor
        entry.insert("anchor".into(), serde_json::json!(anchor_from_code(&code)));

        // Format template
        if let Some(sig) = &cmd.signature {
            let fmt = format_template_from_signature(&code, sig, &params);
            entry.insert("formatTemplate".into(), serde_json::json!(fmt));
        }

        // Composites
        if let Some(comps) = &cmd.composites {
            let notes: Vec<serde_json::Value> = comps
                .iter()
                .map(serde_json::to_value)
                .collect::<std::result::Result<_, _>>()?;
            if !notes.is_empty() {
                entry.insert("hasComposites".into(), serde_json::json!(true));
                entry.insert("composites".into(), serde_json::json!(notes));
            }
        }

        // Enum value docs
        if let Some(args) = &cmd.args {
            let mut enum_docs: Vec<serde_json::Value> = Vec::new();
            let visit_arg = |arg: &zpl_toolchain_spec_tables::Arg,
                             enum_docs: &mut Vec<serde_json::Value>| {
                if arg.r#type == "enum"
                    && let Some(ev) = &arg.r#enum
                {
                    let key = arg.key.as_deref().unwrap_or("").to_string();
                    let values: Vec<serde_json::Value> = ev
                        .iter()
                        .map(|v| match v {
                            zpl_toolchain_spec_tables::EnumValue::Simple(s) => {
                                serde_json::json!({"value": s})
                            }
                            zpl_toolchain_spec_tables::EnumValue::Object {
                                value,
                                extras,
                                printer_gates,
                            } => {
                                serde_json::json!({
                                    "value": value,
                                    "extras": extras,
                                    "printerGates": printer_gates,
                                })
                            }
                        })
                        .collect();
                    enum_docs.push(serde_json::json!({"argKey": key, "values": values}));
                }
            };
            for it in args {
                match it {
                    zpl_toolchain_spec_tables::ArgUnion::OneOf { one_of } => {
                        for arg in one_of {
                            visit_arg(arg, &mut enum_docs);
                        }
                    }
                    zpl_toolchain_spec_tables::ArgUnion::Single(arg) => {
                        visit_arg(arg, &mut enum_docs);
                    }
                }
            }
            if !enum_docs.is_empty() {
                entry.insert("enumValues".into(), serde_json::json!(enum_docs));
            }
        }

        // Missing fields
        let miss = missing_fields(cmd);
        if !miss.is_empty() {
            entry.insert("missingFields".into(), serde_json::json!(miss));
            entry.insert("missingFieldsTotal".into(), serde_json::json!(miss.len()));
        }

        let canonical_entry = serde_json::Value::Object(entry);
        docs_by_code.insert(code.clone(), canonical_entry.clone());
        for alias in all_codes {
            if alias == code {
                continue;
            }
            let mut alias_entry = serde_json::Map::new();
            alias_entry.insert("anchor".into(), serde_json::json!(anchor_from_code(&alias)));
            alias_entry.insert("aliasOf".into(), serde_json::json!(code));
            alias_entry.insert("hasSpec".into(), serde_json::json!(false));
            if let Some(name) = &cmd.name {
                alias_entry.insert("name".into(), serde_json::json!(name));
            }
            alias_entry.insert("category".into(), serde_json::to_value(effective_category)?);
            alias_entry.insert("scope".into(), serde_json::to_value(cmd.scope)?);
            docs_by_code.insert(alias, serde_json::Value::Object(alias_entry));
        }
    }

    // Add placeholders for master list codes not in spec
    let missing_codes: Vec<String> = master_codes
        .iter()
        .filter(|c| !present_code_set.contains(*c))
        .cloned()
        .collect();
    for code in &missing_codes {
        let mut entry = serde_json::Map::new();
        entry.insert("anchor".into(), serde_json::json!(anchor_from_code(code)));
        entry.insert("hasSpec".into(), serde_json::json!(false));
        docs_by_code
            .entry(code.clone())
            .or_insert(serde_json::Value::Object(entry));
    }

    Ok(serde_json::json!({
        "missing_codes": missing_codes,
        "all_codes": master_codes.iter().cloned().collect::<Vec<_>>(),
        "by_code": serde_json::Value::Object(docs_by_code),
        "schema_versions": schema_versions.iter().cloned().collect::<Vec<_>>(),
        "format_version": TABLE_FORMAT_VERSION,
    }))
}

fn format_template_from_signature(
    code: &str,
    sig: &zpl_toolchain_spec_tables::Signature,
    params: &[String],
) -> String {
    let mut out = String::from(code);
    let mut i = 0usize;
    while i < params.len() {
        if let Some(rule) = &sig.split_rule
            && i == rule.param_index
        {
            let split_len = rule.char_counts.len().max(1);
            let end = (i + split_len).min(params.len());
            for key in &params[i..end] {
                out.push_str(&format!("{{{}}}", key));
            }
            i = end;
        } else {
            out.push_str(&format!("{{{}}}", params[i]));
            i += 1;
        }
        if i < params.len() {
            out.push_str(&sig.joiner);
        }
    }
    out
}

// ─── Generate constraints bundle ────────────────────────────────────────────

/// Generate `constraints_bundle.json` — extracted constraint data per command
/// code, including kind, expression, message, and severity.
///
/// **Not consumed at runtime** by the parser, validator, or CLI (constraints
/// are already embedded in `parser_tables.json` via `CommandEntry.constraints`).
/// This artifact is generated for external tooling such as IDE plugins,
/// documentation generators, and constraint analysis tools.
pub fn generate_constraints_bundle(
    commands: &[SourceCommand],
    schema_versions: &BTreeSet<String>,
) -> Result<serde_json::Value> {
    let mut by_code = serde_json::Map::new();
    for cmd in commands {
        let constraints = cmd
            .constraints
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or_else(|| serde_json::json!([]));
        for code in cmd.all_codes() {
            by_code.insert(code, constraints.clone());
        }
    }
    Ok(serde_json::json!({
        "by_code": serde_json::Value::Object(by_code),
        "schema_versions": schema_versions.iter().cloned().collect::<Vec<_>>(),
        "format_version": TABLE_FORMAT_VERSION,
    }))
}

// ─── Generate coverage ──────────────────────────────────────────────────────

/// Generate `coverage.json` — per-command completeness stats, missing fields,
/// and validation error summaries for the spec authoring dashboard.
pub fn generate_coverage(
    commands: &[SourceCommand],
    schema_versions: &BTreeSet<String>,
    master_codes: &BTreeSet<String>,
    validation_errors: &[ValidationError],
) -> serde_json::Value {
    let total = commands.len();
    let mut with_sig = 0usize;
    let mut with_args = 0usize;
    let mut with_constraints = 0usize;
    let mut with_docs = 0usize;
    let mut with_composites = 0usize;
    let mut constraint_kind_counts: HashMap<String, usize> = HashMap::new();
    let mut missing_by_code = serde_json::Map::new();
    let mut per_code = serde_json::Map::new();
    let mut present_code_set = BTreeSet::new();

    // Index validation errors by code
    let val_err_map: HashMap<String, &[String]> = validation_errors
        .iter()
        .map(|ve| (ve.code.clone(), ve.errors.as_slice()))
        .collect();

    for cmd in commands {
        let code = cmd.canonical_code().unwrap_or_default();
        let miss = missing_fields(cmd);

        if cmd.signature.is_some() {
            with_sig += 1;
        }
        if cmd.args.is_some() {
            with_args += 1;
        }
        if cmd.constraints.is_some() {
            with_constraints += 1;
        }
        if cmd.docs.is_some() {
            with_docs += 1;
        }
        if cmd.composites.is_some() {
            with_composites += 1;
        }

        // Count union positions
        let union_positions = cmd.args.as_ref().map_or(0, |args| {
            args.iter()
                .filter(|a| matches!(a, zpl_toolchain_spec_tables::ArgUnion::OneOf { .. }))
                .count()
        });

        // Count constraint kinds
        if let Some(constraints) = &cmd.constraints {
            for c in constraints {
                let kind_str = Some(c.kind.to_string());
                if let Some(kind) = kind_str {
                    *constraint_kind_counts.entry(kind).or_insert(0) += 1;
                }
            }
        }

        // Per-code stats
        let mut per = serde_json::Map::new();
        per.insert(
            "arg_count".into(),
            serde_json::json!(cmd.args.as_ref().map(|a| a.len()).unwrap_or(0)),
        );
        per.insert("union_positions".into(), serde_json::json!(union_positions));
        per.insert(
            "has_composites".into(),
            serde_json::json!(cmd.composites.is_some()),
        );
        per.insert("has_docs".into(), serde_json::json!(cmd.docs.is_some()));
        per.insert(
            "constraints_count".into(),
            serde_json::json!(cmd.constraints.as_ref().map(|c| c.len()).unwrap_or(0)),
        );
        if !miss.is_empty() {
            per.insert("missing_fields_total".into(), serde_json::json!(miss.len()));
            per.insert("missing_fields".into(), serde_json::json!(miss));
        }
        if let Some(errs) = val_err_map.get(&code) {
            per.insert("validation_errors".into(), serde_json::json!(errs));
        }

        if !code.is_empty() {
            // Register ALL codes (including aliases like ~CC for ^CC)
            for c in cmd.all_codes() {
                present_code_set.insert(c.clone());
            }
            per_code.insert(code.clone(), serde_json::Value::Object(per));
        }
        if !miss.is_empty() && !code.is_empty() {
            missing_by_code.insert(code, serde_json::json!(miss));
        }
    }

    // Missing codes from master list
    let missing_in_spec: Vec<String> = master_codes
        .iter()
        .filter(|c| !present_code_set.contains(*c))
        .cloned()
        .collect();

    // Add per_code markers for missing codes
    for code in &missing_in_spec {
        let mut per = serde_json::Map::new();
        per.insert("has_spec".into(), serde_json::json!(false));
        per_code.insert(code.clone(), serde_json::Value::Object(per));
    }

    let present_count = present_code_set.len();
    let missing_count = master_codes.len().saturating_sub(present_count);

    serde_json::json!({
        "master_total": master_codes.len(),
        "present_in_spec_count": present_count,
        "missing_in_spec_count": missing_count,
        "missing_in_spec": missing_in_spec,
        "constraint_kind_counts": constraint_kind_counts,
        "total": total,
        "with_signature": with_sig,
        "with_args": with_args,
        "with_constraints": with_constraints,
        "with_docs": with_docs,
        "with_composites": with_composites,
        "missing_by_code": serde_json::Value::Object(missing_by_code),
        "per_code": serde_json::Value::Object(per_code),
        "schema_versions": schema_versions.iter().cloned().collect::<Vec<_>>(),
        "format_version": TABLE_FORMAT_VERSION,
    })
}

/// Generate canonical state-keys artifact from declared producer effects.
pub fn generate_state_keys(
    commands: &[SourceCommand],
    schema_versions: &BTreeSet<String>,
) -> serde_json::Value {
    let mut keys = BTreeSet::new();
    let mut by_producer = serde_json::Map::new();

    for cmd in commands {
        let Some(effects) = cmd.effects.as_ref() else {
            continue;
        };
        let producer_keys: Vec<String> = effects.sets.clone();
        for k in &producer_keys {
            keys.insert(k.clone());
        }
        let producer_json = serde_json::Value::Array(
            producer_keys
                .iter()
                .map(|k| serde_json::Value::String(k.clone()))
                .collect(),
        );
        for code in cmd.all_codes() {
            by_producer.insert(code, producer_json.clone());
        }
    }

    serde_json::json!({
        "schema_versions": schema_versions.iter().cloned().collect::<Vec<_>>(),
        "format_version": TABLE_FORMAT_VERSION,
        "state_keys": keys.into_iter().collect::<Vec<_>>(),
        "by_producer": by_producer,
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn anchor_from_code(code: &str) -> String {
    code.trim_start_matches('^')
        .trim_start_matches('~')
        .to_string()
}

fn effective_command_category(
    cmd: &SourceCommand,
    code: &str,
) -> Option<zpl_toolchain_spec_tables::CommandCategory> {
    use zpl_toolchain_spec_tables::CommandCategory as C;

    if let Some(category) = cmd.category {
        return Some(category);
    }

    let code_upper = code.to_ascii_uppercase();
    if code_upper.starts_with("^B") || code_upper.starts_with("~B") {
        return Some(C::Barcode);
    }

    let mut hint_text = String::new();
    if let Some(name) = &cmd.name {
        hint_text.push_str(name);
        hint_text.push(' ');
    }
    if let Some(docs) = &cmd.docs {
        hint_text.push_str(docs);
    }
    let hint = hint_text.to_ascii_lowercase();

    if hint.contains("rfid") {
        return Some(C::Rfid);
    }
    if hint.contains("wireless") || hint.contains("wlan") {
        return Some(C::Wireless);
    }
    if hint.contains("network") {
        return Some(C::Network);
    }
    if hint.contains("barcode") {
        return Some(C::Barcode);
    }
    if hint.contains("graphic") || hint.contains("image") {
        return Some(C::Graphics);
    }
    if hint.contains("font") || hint.contains("text") || hint.contains("field data") {
        return Some(C::Text);
    }
    if hint.contains("host") || hint.contains("diagnostic") || hint.contains("status") {
        return Some(C::Host);
    }
    if hint.contains("memory")
        || hint.contains("object")
        || hint.contains("download")
        || hint.contains("storage")
    {
        return Some(C::Storage);
    }
    if hint.contains("media") || hint.contains("print mode") || hint.contains("cutter") {
        return Some(C::Media);
    }
    if hint.contains("keyboard") || hint.contains("kiosk") {
        return Some(C::Kdu);
    }
    if hint.contains("config")
        || hint.contains("setting")
        || hint.contains("calibration")
        || hint.contains("default")
    {
        return Some(C::Config);
    }

    match cmd
        .scope
        .unwrap_or(zpl_toolchain_spec_tables::CommandScope::Field)
    {
        zpl_toolchain_spec_tables::CommandScope::Field
        | zpl_toolchain_spec_tables::CommandScope::Label
        | zpl_toolchain_spec_tables::CommandScope::Document => Some(C::Format),
        zpl_toolchain_spec_tables::CommandScope::Session
        | zpl_toolchain_spec_tables::CommandScope::Job => Some(C::Config),
    }
}

/// Standard fields that every non-structural command is expected to have.
/// Adding a new required field only requires adding one entry here.
const REQUIRED_FIELDS: &[RequiredField] = &[
    RequiredField {
        name: "signature",
        check: |cmd| cmd.signature.is_some(),
        structural_exempt: true,
    },
    RequiredField {
        name: "args",
        check: |cmd| cmd.args.is_some(),
        structural_exempt: true,
    },
    RequiredField {
        name: "constraints",
        check: |cmd| cmd.constraints.is_some(),
        structural_exempt: true,
    },
    RequiredField {
        name: "docs",
        check: |cmd| cmd.docs.is_some(),
        structural_exempt: false,
    },
];

struct RequiredField {
    name: &'static str,
    check: fn(&SourceCommand) -> bool,
    /// If true, structural commands (arity 0 or field_data) are exempt.
    structural_exempt: bool,
}

/// Determine which standard fields are missing for coverage purposes.
fn missing_fields(cmd: &SourceCommand) -> Vec<&'static str> {
    let is_structural = cmd.is_structural();
    REQUIRED_FIELDS
        .iter()
        .filter(|f| {
            let exempt = f.structural_exempt && is_structural;
            !exempt && !(f.check)(cmd)
        })
        .map(|f| f.name)
        .collect()
}

/// Load the master ZPL command list from docs/public/zpl-commands.jsonc.
///
/// Prints a warning to stderr if the file cannot be read or parsed.
pub fn load_master_codes(path: &str) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("warn: could not read master codes file '{}': {}", path, e);
            return codes;
        }
    };
    let v = match parse_jsonc(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("warn: could not parse master codes file '{}': {}", path, e);
            return codes;
        }
    };
    if let Some(arr) = v.get("codes").and_then(|c| c.as_array()) {
        for it in arr {
            if let Some(s) = it.as_str() {
                codes.insert(s.to_string());
            }
        }
    } else {
        eprintln!("warn: master codes file '{}' missing 'codes' array", path);
    }
    codes
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use zpl_toolchain_spec_tables::{ConstraintKind, ParserTables, StructuralTrigger};

    /// Verify that `ConstraintKind::ALL` and the JSONC schema's `kind` enum
    /// list exactly the same set of values (via serde serialized names).
    ///
    /// If this test fails, either the Rust enum or the JSONC schema was updated
    /// without updating the other. See the doc comment on
    /// `validate_command_constraints_spec` for the full checklist.
    #[test]
    fn constraint_kinds_match_schema() {
        // 1. Collect the Rust-side canonical names from ConstraintKind::ALL.
        let rust_kinds: BTreeSet<String> = ConstraintKind::ALL
            .iter()
            .map(|k| serde_json::to_value(k).unwrap())
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        // 2. Read and parse the JSONC schema.
        let schema_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../spec/schema/zpl-spec.schema.jsonc"
        );
        let schema_text =
            std::fs::read_to_string(schema_path).expect("failed to read JSONC schema");
        let schema_json: serde_json::Value =
            crate::parse_jsonc(&schema_text).expect("failed to parse JSONC schema");

        // 3. Navigate to $defs → constraint → properties → kind → enum.
        let schema_kinds: BTreeSet<String> = schema_json
            .pointer("/$defs/constraint/properties/kind/enum")
            .expect("could not find $defs.constraint.properties.kind.enum in schema")
            .as_array()
            .expect("kind.enum is not an array")
            .iter()
            .map(|v| {
                v.as_str()
                    .expect("kind.enum element is not a string")
                    .to_string()
            })
            .collect();

        assert_eq!(
            rust_kinds, schema_kinds,
            "ConstraintKind::ALL and JSONC schema kind enum are out of sync.\n\
             Rust: {:?}\nSchema: {:?}",
            rust_kinds, schema_kinds
        );
    }

    #[test]
    fn structural_rule_kinds_match_schema() {
        let expected: BTreeSet<String> = zpl_toolchain_spec_tables::StructuralRuleKind::ALL
            .iter()
            .map(|kind| serde_json::to_value(kind).unwrap())
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        let schema_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../spec/schema/zpl-spec.schema.jsonc"
        );
        let schema_text =
            std::fs::read_to_string(schema_path).expect("failed to read JSONC schema");
        let schema_json: serde_json::Value =
            crate::parse_jsonc(&schema_text).expect("failed to parse JSONC schema");

        let schema_kinds: BTreeSet<String> = schema_json
            .pointer("/$defs/structuralRule/oneOf")
            .expect("could not find $defs.structuralRule.oneOf in schema")
            .as_array()
            .expect("structuralRule.oneOf is not an array")
            .iter()
            .map(|variant| {
                variant
                    .pointer("/properties/kind/const")
                    .and_then(serde_json::Value::as_str)
                    .expect("structuralRule variant kind.const is not a string")
                    .to_string()
            })
            .collect();

        assert_eq!(
            expected, schema_kinds,
            "structuralRule.kind enum in schema is out of sync"
        );
    }

    #[test]
    fn extract_template_placeholders_empty() {
        assert!(super::extract_template_placeholders("").is_empty());
        assert!(super::extract_template_placeholders("no braces").is_empty());
    }

    #[test]
    fn extract_template_placeholders_single() {
        assert_eq!(
            super::extract_template_placeholders("{d}"),
            vec!["d".to_string()]
        );
    }

    #[test]
    fn extract_template_placeholders_multiple() {
        assert_eq!(
            super::extract_template_placeholders("{d}:{o}.{x}"),
            vec!["d".to_string(), "o".to_string(), "x".to_string()]
        );
    }

    #[test]
    fn extract_template_placeholders_dedupe() {
        assert_eq!(
            super::extract_template_placeholders("{a}_{a}"),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn validate_composites_linkage_valid_template_exposes_args() {
        use super::validate_composites_linkage;
        use crate::source::SourceSpecFile;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^XG"],"arity":1,"signature":{"params":["path"],"joiner":","},"composites":[{"name":"path","template":"{d}:{o}.{x}","exposesArgs":["d","o","x"]}],"args":[{"key":"d","type":"string","name":"d"},{"key":"o","type":"string","name":"o"},{"key":"x","type":"string","name":"x"}]}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let cmd = spec.commands.into_iter().next().unwrap();
        let mut errors = Vec::new();
        validate_composites_linkage(&cmd, &mut errors);
        assert!(errors.is_empty(), "expected no errors: {:?}", errors);
    }

    #[test]
    fn validate_composites_linkage_placeholder_not_in_exposes_args() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^XG"],"arity":2,"signature":{"params":["path","x"],"joiner":","},"composites":[{"name":"path","template":"{d}:{o}.{x}","exposesArgs":["d","o"]}],"args":[{"key":"d","type":"string","name":"d"},{"key":"o","type":"string","name":"o"},{"key":"x","type":"string","name":"x"}]}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let cmd = spec.commands.into_iter().next().unwrap();
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&[cmd], &spec_dir);
        assert!(!errs.is_empty(), "expected validation errors");
        let err = errs.first().unwrap();
        assert!(
            err.errors.iter().any(|e| e.contains("not in exposesArgs")),
            "expected 'not in exposesArgs' error: {:?}",
            err.errors
        );
    }

    #[test]
    fn validate_composites_linkage_exposes_arg_missing_from_template() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^XG"],"arity":3,"signature":{"params":["path","mx","my"],"joiner":","},"composites":[{"name":"path","template":"{d}.{x}","exposesArgs":["d","o","x"]}],"args":[{"key":"d","type":"string","name":"d"},{"key":"o","type":"string","name":"o"},{"key":"x","type":"string","name":"x"}]}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let cmd = spec.commands.into_iter().next().unwrap();
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&[cmd], &spec_dir);
        assert!(!errs.is_empty(), "expected validation errors");
        let err = errs.first().unwrap();
        assert!(
            err.errors
                .iter()
                .any(|e| e.contains("not used in template")),
            "expected 'not used in template' error: {:?}",
            err.errors
        );
    }

    #[test]
    fn format_template_respects_split_rule_without_extra_joiners() {
        let sig = zpl_toolchain_spec_tables::Signature {
            params: vec!["f".into(), "o".into(), "h".into(), "w".into()],
            joiner: ",".into(),
            spacing_policy: zpl_toolchain_spec_tables::SpacingPolicy::Forbid,
            allow_empty_trailing: true,
            split_rule: Some(zpl_toolchain_spec_tables::SplitRule {
                param_index: 0,
                char_counts: vec![1, 1],
            }),
        };
        let params = vec![
            "f".to_string(),
            "o".to_string(),
            "h".to_string(),
            "w".to_string(),
        ];
        let fmt = super::format_template_from_signature("^A", &sig, &params);
        assert_eq!(fmt, "^A{f}{o},{h},{w}");
    }

    #[test]
    fn validate_default_from_always_requires_default_from_state_key() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^P"],"arity":0,"effects":{"sets":["font.height","font.width"]}},{"codes":["^C"],"arity":1,"signature":{"params":["x"],"joiner":","},"args":[{"name":"x","key":"x","type":"int","defaultFrom":"^P"}]}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(!errs.is_empty(), "expected validation errors");
        assert!(
            errs.iter()
                .flat_map(|e| e.errors.iter())
                .any(|e| e.contains("requires defaultFromStateKey")),
            "expected required defaultFromStateKey error: {:?}",
            errs
        );
    }

    #[test]
    fn validate_default_from_requires_state_key_even_for_single_effect_value() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^P"],"arity":0,"effects":{"sets":["font.height"]}},{"codes":["^C"],"arity":1,"signature":{"params":["x"],"joiner":","},"args":[{"name":"x","key":"x","type":"int","defaultFrom":"^P"}]}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(!errs.is_empty(), "expected validation errors");
        assert!(
            errs.iter()
                .flat_map(|e| e.errors.iter())
                .any(|e| e.contains("requires defaultFromStateKey")),
            "expected required defaultFromStateKey error: {:?}",
            errs
        );
    }

    #[test]
    fn validate_default_from_with_state_key_passes() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^P"],"arity":0,"effects":{"sets":["font.height"]}},{"codes":["^C"],"arity":1,"signature":{"params":["x"],"joiner":","},"args":[{"name":"x","key":"x","type":"int","defaultFrom":"^P","defaultFromStateKey":"font.height"}]}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(errs.is_empty(), "expected no validation errors: {:?}", errs);
    }

    #[test]
    fn generate_state_keys_collects_effect_sets() {
        use super::generate_state_keys;
        use crate::source::SourceSpecFile;
        use std::collections::BTreeSet;

        let json = r#"{"schemaVersion":"1.1.1","commands":[{"codes":["^P","~P"],"arity":0,"effects":{"sets":["font.height","font.width"]}},{"codes":["^Q"],"arity":0}]}"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let mut versions = BTreeSet::new();
        versions.insert("1.1.1".to_string());
        let out = generate_state_keys(&spec.commands, &versions);

        let keys = out
            .get("state_keys")
            .and_then(|v| v.as_array())
            .expect("state_keys array");
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k == "font.height"));
        assert!(keys.iter().any(|k| k == "font.width"));

        let by = out
            .get("by_producer")
            .and_then(|v| v.as_object())
            .expect("by_producer object");
        assert!(by.contains_key("^P"));
        assert!(by.contains_key("~P"));
    }

    #[test]
    fn note_audit_flags_conditional_note_without_expr() {
        use super::audit_notes;
        use crate::source::SourceSpecFile;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T1"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "message":"Supported only on KR403 printers." }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let findings = audit_notes(&spec.commands);
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("looks conditional but has no expr")),
            "expected conditional note finding: {:?}",
            findings
        );
    }

    #[test]
    fn note_audit_flags_explanatory_note_without_audience() {
        use super::audit_notes;
        use crate::source::SourceSpecFile;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T2"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "message":"Sets defaults for subsequent barcode commands." }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let findings = audit_notes(&spec.commands);
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("consider audience=contextual")),
            "expected explanatory note finding: {:?}",
            findings
        );
    }

    #[test]
    fn note_audit_flags_empty_message() {
        use super::audit_notes;
        use crate::source::SourceSpecFile;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T3"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "message":"   " }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let findings = audit_notes(&spec.commands);
        let empty_finding = findings
            .iter()
            .find(|f| f.message.contains("empty") && f.level == "error");
        assert!(
            empty_finding.is_some(),
            "expected empty message error finding: {:?}",
            findings
        );
    }

    #[test]
    fn validate_constraints_require_explicit_scope_for_requires() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T2"],
                "arity":0,
                "constraints":[
                  { "kind":"requires", "expr":"^XA", "message":"requires scope" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("constraint requires explicit scope")),
            "expected explicit scope validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_constraints_require_explicit_scope_for_order() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^FD","^FV"],
                "arity":0
              },
              {
                "codes":["^T2"],
                "arity":0,
                "constraints":[
                  { "kind":"order", "expr":"before:^FD|^FV", "message":"order without scope" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("order constraint requires explicit scope")),
            "expected order constraint explicit scope validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_constraints_order_with_scope_passes() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^FD","^FV"],
                "arity":0
              },
              {
                "codes":["^T2"],
                "arity":0,
                "constraints":[
                  { "kind":"order", "expr":"before:^FD|^FV", "scope":"field", "message":"order with scope" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(errs.is_empty(), "order with scope should pass: {:?}", errs);
    }

    #[test]
    fn validate_constraints_reject_audience_on_non_note() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T2"],
                "arity":0,
                "constraints":[
                  { "kind":"requires", "expr":"^XA", "audience":"contextual", "message":"bad audience placement" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("audience is only supported for kind=note")),
            "expected audience-on-non-note validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_constraints_reject_unknown_note_expr_prefix() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T3"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "expr":"during:^XA", "message":"bad note expr prefix" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("recognized note expression prefix")),
            "expected note expr validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_note_expr_rejects_unknown_target_command_codes() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^TN"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "expr":"after:first:^FAKE", "message":"bad target" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("references unknown command '^FAKE'")),
            "expected unknown target command validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_note_expr_accepts_profile_predicates() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^TP"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "expr":"when:profile:dpi:203", "message":"203 DPI hint" },
                  { "kind":"note", "expr":"when:profile:id:zebra-xi4-203||profile:feature:cutter", "message":"Xi4 or cutter hint" },
                  { "kind":"note", "expr":"when:arg:xPresent&&profile:firmwareGte:V60.14", "message":"Firmware hint" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.is_empty(),
            "profile predicates should be accepted: {:?}",
            errs
        );
    }

    #[test]
    fn validate_note_expr_rejects_unknown_when_predicate() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^T4"],
                "arity":0,
                "constraints":[
                  { "kind":"note", "expr":"when:unknown:foo", "message":"bad predicate" }
                ]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("not a recognized predicate")),
            "expected unknown predicate validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_structural_rules_requires_mapping_for_semantic_commands() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              { "codes":["^FN"], "arity":1, "signature":{"params":["n"],"joiner":","}, "args":[{"name":"n","key":"n","type":"int"}] }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("structuralRules missing required rule")),
            "expected missing structuralRules validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn validate_structural_rules_rejects_unsupported_rule_for_command() {
        use super::validate_cross_field;
        use crate::source::SourceSpecFile;
        use std::path::Path;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              { "codes":["^FN"], "arity":1, "structuralRules":[{"kind":"duplicateFieldNumber"},{"kind":"mediaModes","target":"supportedModes"}], "signature":{"params":["n"],"joiner":","}, "args":[{"name":"n","key":"n","type":"int"}] }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
        let errs = validate_cross_field(&spec.commands, &spec_dir);
        assert!(
            errs.iter()
                .flat_map(|entry| entry.errors.iter())
                .any(|msg| msg.contains("unsupported structural rule")),
            "expected unsupported structuralRules validation failure: {:?}",
            errs
        );
    }

    #[test]
    fn generate_tables_emits_structural_rule_index() {
        use crate::source::SourceSpecFile;
        use std::collections::BTreeSet;

        let json = r#"{
            "schemaVersion":"1.1.1",
            "commands":[
              {
                "codes":["^PW"],
                "arity":1,
                "effects":{"sets":["label.width"]},
                "structuralRules":[{"kind":"positionBounds","action":"trackWidth"}],
                "signature":{"params":["w"],"joiner":","},
                "args":[{"name":"w","key":"w","type":"int"}]
              },
              {
                "codes":["^FO"],
                "arity":2,
                "opens_field":true,
                "structuralRules":[
                  {"kind":"positionBounds","action":"trackFieldOrigin"},
                  {"kind":"positionBounds","action":"validateFieldOrigin"}
                ],
                "signature":{"params":["x","y"],"joiner":","},
                "args":[{"name":"x","key":"x","type":"int"},{"name":"y","key":"y","type":"int"}]
              }
            ]
        }"#;
        let val = crate::parse_jsonc(json).expect("parse");
        let spec: SourceSpecFile = serde_json::from_value(val).expect("deserialize");
        let schema_versions = BTreeSet::from([String::from("1.1.1")]);
        let tables = super::generate_tables(&spec.commands, &schema_versions).expect("tables");

        let by_kind = tables
            .pointer("/structuralRuleIndex/byKind/positionBounds")
            .and_then(serde_json::Value::as_array)
            .expect("missing structuralRuleIndex.byKind.positionBounds");
        assert!(
            by_kind.iter().any(|v| v == "^PW") && by_kind.iter().any(|v| v == "^FO"),
            "expected ^PW and ^FO in byKind.positionBounds: {:?}",
            by_kind
        );

        let by_trigger = tables
            .pointer("/structuralRuleIndex/byTrigger/opensField")
            .and_then(serde_json::Value::as_array)
            .expect("missing structuralRuleIndex.byTrigger.opensField");
        assert!(
            by_trigger.iter().any(|v| v == "^FO"),
            "expected ^FO in byTrigger.opensField: {:?}",
            by_trigger
        );

        let by_effect = tables
            .pointer("/structuralRuleIndex/byEffect/label.width")
            .and_then(serde_json::Value::as_array)
            .expect("missing structuralRuleIndex.byEffect.label.width");
        assert!(
            by_effect.iter().any(|v| v == "^PW"),
            "expected ^PW in byEffect.label.width: {:?}",
            by_effect
        );
    }

    #[test]
    fn structural_trigger_index_matches_command_entry_flags() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../generated/parser_tables.json");
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let tables: ParserTables = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to parse {}: {}", path.display(), e));
        let index = tables
            .structural_rule_index
            .as_ref()
            .expect("expected structural_rule_index in parser tables");

        for cmd in &tables.commands {
            let Some(code) = cmd.codes.first() else {
                continue;
            };

            let checks = [
                (
                    "opens_field",
                    cmd.opens_field,
                    StructuralTrigger::OpensField,
                ),
                (
                    "closes_field",
                    cmd.closes_field,
                    StructuralTrigger::ClosesField,
                ),
                ("field_data", cmd.field_data, StructuralTrigger::FieldData),
                (
                    "raw_payload",
                    cmd.raw_payload,
                    StructuralTrigger::RawPayload,
                ),
                (
                    "field_number",
                    cmd.field_number,
                    StructuralTrigger::FieldNumber,
                ),
                (
                    "serialization",
                    cmd.serialization,
                    StructuralTrigger::Serialization,
                ),
                (
                    "requires_field",
                    cmd.requires_field,
                    StructuralTrigger::RequiresField,
                ),
                (
                    "hex_escape_modifier",
                    cmd.hex_escape_modifier,
                    StructuralTrigger::HexEscapeModifier,
                ),
            ];

            for (flag_name, expected, trigger) in checks {
                let present = index
                    .by_trigger
                    .get(&trigger)
                    .is_some_and(|codes| codes.contains(code));
                assert_eq!(
                    present, expected,
                    "command {} mismatch for {} against by_trigger.{:?}",
                    code, flag_name, trigger
                );
            }
        }
    }
}
