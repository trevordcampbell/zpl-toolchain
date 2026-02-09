//! Spec-compiler build pipeline: load → validate → generate.
//!
//! Each function is pure (input → output), testable independently.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;

use anyhow::Result;

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
                }
                zpl_toolchain_spec_tables::ConstraintKind::EmptyData => {
                    // No expr needed
                }
                // Range, Note, and Custom: no expr grammar to validate.
                // Range expr is freeform, Note has optional expr, Custom is
                // escape-hatch.
                zpl_toolchain_spec_tables::ConstraintKind::Range
                | zpl_toolchain_spec_tables::ConstraintKind::Note
                | zpl_toolchain_spec_tables::ConstraintKind::Custom => {}
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
        }
    }
}

/// Validate composites linkage: composite names must appear in signature params,
/// and exposed args must exist in the command's args.
fn validate_composites_linkage(cmd: &SourceCommand, errors: &mut Vec<String>) {
    if let Some(comps) = &cmd.composites {
        let params = cmd.signature_params();
        let arg_keys = cmd.arg_keys();
        for comp in comps {
            if !comp.name.is_empty() {
                if !params.is_empty() && !params.iter().any(|p| p == &comp.name) {
                    errors.push(format!(
                        "composite '{}' not referenced in signature.params",
                        comp.name
                    ));
                }
                for k in &comp.exposes_args {
                    if !arg_keys.contains(k) {
                        errors.push(format!(
                            "composite '{}' exposes arg '{}' not present in args",
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

    let mut results = validate_duplicate_opcodes(commands);

    for cmd in commands {
        let code = cmd.canonical_code().unwrap_or_default();
        let mut errors = Vec::new();

        validate_command_arity(cmd, &mut errors);
        validate_signature_linkage(cmd, &mut errors);
        validate_arg_hygiene(cmd, &all_codes, &has_effects, &mut errors);
        validate_signature_overrides(cmd, &mut errors);
        validate_command_constraints_spec(cmd, &all_codes, &mut errors);
        validate_composites_linkage(cmd, &mut errors);
        validate_effects(cmd, &mut errors);
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
    // If multiple schema versions exist, `warn_schema_versions()` in main.rs
    // already warns about unexpected versions. We pick the latest here to
    // ensure deterministic, forward-compatible output.
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
            effects: cmd.effects.clone(),
            plane: cmd.plane,
            scope: cmd.scope,
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
        })
        .collect();

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
        present_code_set.insert(code.clone());
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

        // Stable anchor
        entry.insert("anchor".into(), serde_json::json!(anchor_from_code(&code)));

        // Format template
        if let Some(sig) = &cmd.signature {
            let joiner = &sig.joiner;
            let params_fmt: Vec<String> = params.iter().map(|k| format!("{{{}}}", k)).collect();
            let fmt = format!("{}{}", code, params_fmt.join(joiner));
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

        docs_by_code.insert(code, serde_json::Value::Object(entry));
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

// ─── Helpers ────────────────────────────────────────────────────────────────

fn anchor_from_code(code: &str) -> String {
    code.trim_start_matches('^')
        .trim_start_matches('~')
        .to_string()
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
    use zpl_toolchain_spec_tables::ConstraintKind;

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
}
