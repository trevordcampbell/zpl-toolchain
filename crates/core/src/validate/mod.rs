pub use crate::grammar::diag::Diagnostic;
use crate::grammar::{ast::Ast, diag::Severity, diag::Span, diag::codes, tables::ParserTables};
use crate::state::{DeviceState, LabelValueState, ResolvedLabelState, Units, convert_to_dots};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::{
    CommandScope, ComparisonOp, ConstraintKind, ConstraintScope, Plane, RoundingMode,
};

/// Shorthand for building a `BTreeMap<String, String>` context from key-value pairs.
///
/// ```ignore
/// ctx!("command" => code, "arg" => name, "value" => val)
/// ```
macro_rules! ctx {
    ($($k:expr => $v:expr),+ $(,)?) => {
        BTreeMap::from([$(($k.into(), $v.into())),+])
    };
}

/// Result of validating a ZPL AST against spec tables and an optional printer profile.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    /// `true` if no errors were found (warnings and info are allowed).
    pub ok: bool,
    /// All diagnostics produced during validation.
    pub issues: Vec<Diagnostic>,
    /// Renderer-ready resolved state for each label.
    pub resolved_labels: Vec<ResolvedLabelState>,
}

fn map_sev(sev: Option<&zpl_toolchain_spec_tables::ConstraintSeverity>) -> Severity {
    match sev {
        Some(zpl_toolchain_spec_tables::ConstraintSeverity::Error) => Severity::Error,
        Some(zpl_toolchain_spec_tables::ConstraintSeverity::Info) => Severity::Info,
        _ => Severity::Warn,
    }
}

fn trim_f64(n: f64) -> String {
    let s = format!("{:.6}", n);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".to_string() } else { s }
}

// Very small predicate support for conditionalRange / roundingPolicyWhen
// MVP: support keys like "arg:keyIsValue:X" or "arg:keyPresent" or "arg:keyEmpty"
fn predicate_matches(when: &str, args: &[crate::grammar::ast::ArgSlot]) -> bool {
    if let Some(rest) = when.strip_prefix("arg:") {
        if let Some((k, rhs)) = rest.split_once("IsValue:") {
            return args
                .iter()
                .any(|a| a.key.as_deref() == Some(k) && a.value.as_deref() == Some(rhs));
        }
        if let Some(k) = rest.strip_suffix("Present") {
            return args.iter().any(|a| {
                a.key.as_deref() == Some(k) && a.presence == crate::grammar::ast::Presence::Value
            });
        }
        if let Some(k) = rest.strip_suffix("Empty") {
            return args.iter().any(|a| {
                a.key.as_deref() == Some(k) && a.presence == crate::grammar::ast::Presence::Empty
            });
        }
    }
    false
}

/// Check if an enum value list contains a given value.
fn enum_contains(values: &[zpl_toolchain_spec_tables::EnumValue], target: &str) -> bool {
    values.iter().any(|e| match e {
        zpl_toolchain_spec_tables::EnumValue::Simple(s) => s == target,
        zpl_toolchain_spec_tables::EnumValue::Object { value, .. } => value == target,
    })
}

// Select the effective Arg from an ArgUnion using a simple heuristic based on the slot value.
fn select_effective_arg<'a>(
    u: &'a zpl_toolchain_spec_tables::ArgUnion,
    slot: Option<&crate::grammar::ast::ArgSlot>,
) -> Option<&'a zpl_toolchain_spec_tables::Arg> {
    match u {
        zpl_toolchain_spec_tables::ArgUnion::Single(a) => Some(a),
        zpl_toolchain_spec_tables::ArgUnion::OneOf { one_of } => {
            if let Some(s) = slot
                && let Some(v) = s.value.as_ref()
            {
                // Prefer enum arm that contains v
                if let Some(arg) = one_of.iter().find(|a| {
                    a.r#type == "enum" && a.r#enum.as_ref().is_some_and(|ev| enum_contains(ev, v))
                }) {
                    return Some(arg);
                }
                // Numeric arm if value parses
                if let Some(arg) = one_of.iter().find(|a| {
                    (a.r#type == "int" || a.r#type == "float") && v.parse::<f64>().is_ok()
                }) {
                    return Some(arg);
                }
            }
            // Fallback to first
            one_of.first()
        }
    }
}

/// Type alias for profile field accessor functions.
type ProfileFieldFn = fn(&Profile) -> Option<f64>;

/// Declarative registry of all numeric profile fields.
///
/// Adding a new numeric profile field requires adding one entry here.
/// The `all_profile_constraint_fields_are_resolvable` test ensures
/// coverage of all fields referenced by `profileConstraint` in specs.
const PROFILE_FIELD_REGISTRY: &[(&str, ProfileFieldFn)] = &[
    ("dpi", |p| Some(p.dpi as f64)),
    ("page.width_dots", |p| {
        p.page
            .as_ref()
            .and_then(|pg| pg.width_dots.map(|v| v as f64))
    }),
    ("page.height_dots", |p| {
        p.page
            .as_ref()
            .and_then(|pg| pg.height_dots.map(|v| v as f64))
    }),
    ("speed_range.min", |p| {
        p.speed_range.as_ref().map(|r| r.min as f64)
    }),
    ("speed_range.max", |p| {
        p.speed_range.as_ref().map(|r| r.max as f64)
    }),
    ("darkness_range.min", |p| {
        p.darkness_range.as_ref().map(|r| r.min as f64)
    }),
    ("darkness_range.max", |p| {
        p.darkness_range.as_ref().map(|r| r.max as f64)
    }),
    ("memory.ram_kb", |p| {
        p.memory.as_ref().and_then(|m| m.ram_kb.map(|v| v as f64))
    }),
    ("memory.flash_kb", |p| {
        p.memory.as_ref().and_then(|m| m.flash_kb.map(|v| v as f64))
    }),
];

/// Cached lookup map from field path to accessor function.
static PROFILE_FIELD_MAP: OnceLock<HashMap<&'static str, ProfileFieldFn>> = OnceLock::new();

fn profile_field_map() -> &'static HashMap<&'static str, ProfileFieldFn> {
    PROFILE_FIELD_MAP.get_or_init(|| PROFILE_FIELD_REGISTRY.iter().copied().collect())
}

/// Resolve a profile field by dotted path (e.g., "page.width_dots").
///
/// Returns the numeric value of the named profile field, or `None` if the
/// field path is unrecognized or the corresponding value is not set in the
/// profile. Used by the validator for `profileConstraint` checks and
/// exposed publicly so that tests can verify coverage of all constraint
/// field paths referenced in command specs.
pub fn resolve_profile_field(profile: &Profile, field: &str) -> Option<f64> {
    profile_field_map().get(field).and_then(|f| f(profile))
}

/// Check a profile constraint operator.
///
/// Returns `false` (constraint violated) for non-finite values (NaN, infinity)
/// to prevent them from silently passing validation.
///
/// The `Eq` tolerance of 0.5 is intentional: all profile fields (DPI, dots,
/// speed, darkness, KB) are integer values cast to `f64`, so two values
/// represent the same integer exactly when they round to the same whole
/// number — i.e., when their difference is less than 0.5.  This is far more
/// robust than `f64::EPSILON` (~2.2e-16), which is the unit-of-least-precision
/// near 1.0 and is neither a correct nor a general-purpose equality tolerance.
fn check_profile_op(value: f64, op: &ComparisonOp, limit: f64) -> bool {
    if !value.is_finite() || !limit.is_finite() {
        return false;
    }
    match op {
        ComparisonOp::Lte => value <= limit,
        ComparisonOp::Gte => value >= limit,
        ComparisonOp::Lt => value < limit,
        ComparisonOp::Gt => value > limit,
        ComparisonOp::Eq => (value - limit).abs() < 0.5,
    }
}

/// Check if any of the pipe-separated targets are present in a pre-built set (O(1) per target).
fn any_target_in_set(targets: &str, seen: &HashSet<&str>) -> bool {
    targets.split('|').any(|target| seen.contains(target))
}

// ─── Cross-command state tracking ───────────────────────────────────────────

/// Tracks which state-setting commands have been seen within a label.
#[derive(Debug, Default)]
struct LabelState {
    /// Set of producer commands that have been seen in this label.
    producers_seen: HashSet<String>,
    /// Track field numbers seen (for duplicate ^FN detection)
    field_numbers: HashMap<String, usize>, // value -> first node_idx
    /// Track ^CW font registrations (font letter -> node_idx)
    loaded_fonts: HashSet<char>,
    /// Track last producer position for redundant state detection
    last_producer_idx: HashMap<String, usize>,
    /// Track whether any consumer has used a producer's state since it was set
    producer_consumed: HashMap<String, bool>,
    /// Track effective print width (from ^PW) and label length (from ^LL)
    effective_width: Option<f64>,
    effective_height: Option<f64>,
    /// Whether ^PW was explicitly set in this label (vs inherited from profile).
    has_explicit_pw: bool,
    /// Whether ^LL was explicitly set in this label (vs inherited from profile).
    has_explicit_ll: bool,
    /// Last ^FO x position (for graphic bounds checking).
    last_fo_x: Option<f64>,
    /// Last ^FO y position (for graphic bounds checking).
    last_fo_y: Option<f64>,
    /// Accumulated total graphic bytes from ^GF commands (for memory estimation).
    gf_total_bytes: u32,
    /// Typed producer values for renderer/validator default resolution.
    value_state: LabelValueState,
}

impl LabelState {
    /// Record that a state-producing command was seen.
    fn record_producer(&mut self, code: &str, node_idx: usize) {
        let key = code.to_string();
        self.producers_seen.insert(key.clone());
        self.last_producer_idx.insert(key.clone(), node_idx);
        self.producer_consumed.insert(key, false);
    }

    /// Check if a given producer command has been seen.
    fn has_producer(&self, code: &str) -> bool {
        self.producers_seen.contains(code)
    }

    /// Mark a producer as consumed (its state was used by a consumer command).
    fn mark_consumed(&mut self, producer_code: &str) {
        if let Some(consumed) = self.producer_consumed.get_mut(producer_code) {
            *consumed = true;
        }
    }
}

// ─── Field-level structural tracking ────────────────────────────────────────

/// Tracks field-level structural state within a label.
/// Reset when a field-opening command is encountered.
struct FieldTracker {
    /// Whether a field is currently open (between ^FO/^FT and ^FS).
    open: bool,
    /// Whether ^FH was seen in the current field.
    has_fh: bool,
    /// The hex escape indicator character (default `_`, configurable via ^FH arg).
    fh_indicator: u8,
    /// Whether ^FN was seen in the current field.
    has_fn: bool,
    /// Whether ^SN/^SF was seen in the current field.
    has_serial: bool,
    /// Node index of the field-opening command.
    start_idx: usize,
    /// Barcode commands seen in this field, in order, with their node index.
    /// Used to attribute ^FD/^FV segments to the correct barcode when multiple
    /// barcode commands appear in a single field.
    active_barcodes: Vec<(usize, String, zpl_toolchain_spec_tables::FieldDataRules)>,
}

impl Default for FieldTracker {
    fn default() -> Self {
        Self {
            open: false,
            has_fh: false,
            fh_indicator: b'_',
            has_fn: false,
            has_serial: false,
            start_idx: 0,
            active_barcodes: Vec::new(),
        }
    }
}

impl FieldTracker {
    /// Reset field state for a new field.
    fn reset(&mut self) {
        self.has_fh = false;
        self.fh_indicator = b'_';
        self.has_fn = false;
        self.has_serial = false;
        self.active_barcodes.clear();
    }

    /// Process a command's structural flags and emit diagnostics.
    fn process_command(
        &mut self,
        cmd_ctx: &CommandCtx,
        vctx: &ValidationContext,
        label_state: &LabelState,
        issues: &mut Vec<Diagnostic>,
    ) {
        if cmd_ctx.cmd.opens_field {
            if self.open {
                issues.push(
                    Diagnostic::warn(
                        codes::FIELD_NOT_CLOSED,
                        format!(
                            "{} opens a new field before previous field was closed with ^FS",
                            cmd_ctx.code
                        ),
                        cmd_ctx.span,
                    )
                    .with_context(ctx!("command" => cmd_ctx.code)),
                );
            }
            self.open = true;
            self.reset();
            self.start_idx = cmd_ctx.node_idx;
        }

        if cmd_ctx.cmd.closes_field {
            self.validate_field_close(cmd_ctx, vctx, label_state, issues);
        }

        if (cmd_ctx.cmd.field_data || cmd_ctx.cmd.requires_field) && !self.open {
            issues.push(
                Diagnostic::warn(
                    codes::FIELD_DATA_WITHOUT_ORIGIN,
                    format!(
                        "{} without preceding field origin (no field origin)",
                        cmd_ctx.code
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!("command" => cmd_ctx.code)),
            );
        }

        if cmd_ctx.cmd.hex_escape_modifier {
            self.has_fh = true;
            // Capture the indicator character from the ^FH argument (default '_').
            // ^FH's first arg is the indicator char — if provided, use it.
            if let Some(slot) = cmd_ctx.args.first()
                && let Some(val) = slot.value.as_deref()
                && let Some(ch) = val.bytes().next()
            {
                self.fh_indicator = ch;
            }
        }
        if cmd_ctx.cmd.field_number {
            self.has_fn = true;
        }
        if cmd_ctx.cmd.serialization {
            self.has_serial = true;
        }

        // Track barcode commands for field data validation
        if let Some(rules) = &cmd_ctx.cmd.field_data_rules
            && (rules.character_set.is_some()
                || rules.exact_length.is_some()
                || rules.allowed_lengths.is_some()
                || rules.min_length.is_some()
                || rules.max_length.is_some()
                || rules.length_parity.is_some())
        {
            self.active_barcodes
                .push((cmd_ctx.node_idx, cmd_ctx.code.to_string(), rules.clone()));
        }
    }

    /// Validate when a field-closing command (^FS) is encountered.
    fn validate_field_close(
        &mut self,
        cmd_ctx: &CommandCtx,
        vctx: &ValidationContext,
        label_state: &LabelState,
        issues: &mut Vec<Diagnostic>,
    ) {
        // ZPL2204: Orphaned field separator — check first and skip field
        // content validation since there's no valid field to validate.
        if !self.open {
            issues.push(
                Diagnostic::warn(
                    codes::ORPHANED_FIELD_SEPARATOR,
                    format!(
                        "{} without a preceding field origin (orphaned field separator)",
                        cmd_ctx.code
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!("command" => cmd_ctx.code)),
            );
            return;
        }

        // ZPL2304: Validate hex escapes in field data if ^FH was active
        if self.has_fh {
            let indicator = self.fh_indicator;
            for field_node in &vctx.label_nodes[self.start_idx..cmd_ctx.node_idx] {
                let content_and_span = match field_node {
                    crate::grammar::ast::Node::FieldData { content, span, .. } => {
                        Some((content.as_str(), Some(*span)))
                    }
                    crate::grammar::ast::Node::Command {
                        code, args, span, ..
                    } if code == "^FD" || code == "^FV" => args
                        .first()
                        .and_then(|slot| slot.value.as_deref())
                        .map(|val| (val, Some(*span))),
                    _ => None,
                };
                if let Some((content, dspan)) = content_and_span {
                    for err in crate::hex_escape::validate_hex_escapes(content, indicator) {
                        issues.push(
                            Diagnostic::error(codes::INVALID_HEX_ESCAPE, err.message, dspan)
                                .with_context(ctx!(
                                    "command" => "^FH",
                                    "indicator" => String::from(indicator as char)
                                )),
                        );
                    }
                }
            }
        }

        // ZPL2306: Serialization without field number
        if self.has_serial && !self.has_fn {
            issues.push(
                Diagnostic::warn(
                    codes::SERIALIZATION_WITHOUT_FIELD_NUMBER,
                    "Serialization (^SN/^SF) in field without ^FN field number",
                    cmd_ctx.span,
                )
                .with_context(ctx!("command" => "^SN/^SF")),
            );
        }

        // ZPL2401/ZPL2402: Barcode field data validation
        // Skip when ^FH (hex escape) is active — raw content contains escape
        // sequences that alter the actual byte values, making character-set
        // validation against the raw text incorrect.
        //
        // Field data can live in two places:
        // 1. Inline: the first arg (key="data") of a ^FD/^FV command node
        // 2. Multi-line: a FieldData node following the ^FD/^FV command
        //
        // Validate the combined payload for the field once, so split data like
        // "^FD123<newline>456^FS" is evaluated as "123456" instead of two
        // separate fragments.
        if !self.has_fh && !self.active_barcodes.is_empty() {
            for (i, (barcode_idx, barcode_code, rules)) in self.active_barcodes.iter().enumerate() {
                let seg_start = *barcode_idx;
                let seg_end = self
                    .active_barcodes
                    .get(i + 1)
                    .map(|(next_idx, _, _)| *next_idx)
                    .unwrap_or(cmd_ctx.node_idx);

                let mut combined_fd = String::new();
                let mut has_any_fd = false;
                let mut first_fd_span: Option<Span> = None;
                for field_node in &vctx.label_nodes[seg_start..seg_end] {
                    match field_node {
                        crate::grammar::ast::Node::Command {
                            code, args, span, ..
                        } if code == "^FD" || code == "^FV" => {
                            if let Some(slot) = args.first()
                                && let Some(val) = slot.value.as_deref()
                            {
                                has_any_fd = true;
                                combined_fd.push_str(val);
                                if first_fd_span.is_none() {
                                    first_fd_span = Some(*span);
                                }
                            }
                        }
                        crate::grammar::ast::Node::FieldData { content, span, .. } => {
                            has_any_fd = true;
                            combined_fd.push_str(content);
                            if first_fd_span.is_none() {
                                first_fd_span = Some(*span);
                            }
                        }
                        _ => {}
                    }
                }
                if has_any_fd {
                    validate_barcode_field_data(
                        barcode_code,
                        &combined_fd,
                        rules,
                        first_fd_span.or(cmd_ctx.span),
                        issues,
                    );
                }
            }
        }

        // ZPL2311: Object bounds check (text/barcode overflow)
        validate_object_bounds(self, cmd_ctx, vctx, label_state, issues);

        self.open = false;
        self.reset();
    }
}

/// ZPL2311: Check if text or barcode content extends beyond label bounds.
///
/// Uses conservative estimates: text width = chars × char_width (height if
/// width unset); barcode dimensions from ^BY + data-length heuristics.
fn validate_object_bounds(
    field: &FieldTracker,
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    let Some(fo_x) = label_state.last_fo_x else {
        return;
    };
    let Some(fo_y) = label_state.last_fo_y else {
        return;
    };
    let max_x = label_state.effective_width.or_else(|| {
        vctx.profile
            .and_then(|p| resolve_profile_field(p, "page.width_dots"))
    });
    let max_y = label_state.effective_height.or_else(|| {
        vctx.profile
            .and_then(|p| resolve_profile_field(p, "page.height_dots"))
    });
    let (Some(max_x), Some(max_y)) = (max_x, max_y) else {
        return;
    };

    // Gather full field content (all ^FD/^FV + FieldData)
    let mut combined_fd = String::new();
    for node in &vctx.label_nodes[field.start_idx..cmd_ctx.node_idx] {
        match node {
            crate::grammar::ast::Node::Command { code, args, .. }
                if code == "^FD" || code == "^FV" =>
            {
                if let Some(slot) = args.first().and_then(|a| a.value.as_deref()) {
                    combined_fd.push_str(slot);
                }
            }
            crate::grammar::ast::Node::FieldData { content, .. } => combined_fd.push_str(content),
            _ => {}
        }
    }
    let char_count = combined_fd.chars().count();
    if char_count == 0 {
        return;
    }

    let is_barcode = !field.active_barcodes.is_empty();
    let (est_width, est_height, object_type) = if is_barcode {
        // Barcode: height from ^BY, width from modules (Code 128 ~11 mod/char + overhead)
        let height = label_state.value_state.barcode.height.unwrap_or(50) as f64;
        let mw = label_state.value_state.barcode.module_width.unwrap_or(2) as f64;
        let modules_per_char = 11.0_f64;
        let modules = (modules_per_char * char_count as f64 + 22.0).ceil();
        let width = (modules * mw).ceil();
        (width, height, "barcode")
    } else {
        // Text: font height/width from ^CF or ^A defaults
        let fh = label_state.value_state.font.height.unwrap_or(20) as f64;
        let fw = label_state
            .value_state
            .font
            .width
            .unwrap_or_else(|| label_state.value_state.font.height.unwrap_or(20))
            as f64;
        let width = (char_count as f64 * fw).ceil();
        let height = fh;
        (width, height, "text")
    };

    let overflows_x = fo_x + est_width > max_x;
    let overflows_y = fo_y + est_height > max_y;
    if overflows_x || overflows_y {
        issues.push(
            Diagnostic::warn(
                codes::OBJECT_BOUNDS_OVERFLOW,
                format!(
                    "{} at ({}, {}) extends beyond label bounds ({}×{} dots)",
                    object_type,
                    trim_f64(fo_x),
                    trim_f64(fo_y),
                    trim_f64(max_x),
                    trim_f64(max_y)
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "object_type" => object_type,
                "x" => trim_f64(fo_x),
                "y" => trim_f64(fo_y),
                "estimated_width" => trim_f64(est_width),
                "estimated_height" => trim_f64(est_height),
                "label_width" => trim_f64(max_x),
                "label_height" => trim_f64(max_y),
            )),
        );
    }
}

/// Validate field data content against the active barcode's `fieldDataRules`.
///
/// Called from `validate_field_close()` when a barcode command was seen in the
/// current field and field data is present.
fn validate_barcode_field_data(
    barcode_code: &str,
    fd_content: &str,
    rules: &zpl_toolchain_spec_tables::FieldDataRules,
    dspan: Option<Span>,
    issues: &mut Vec<Diagnostic>,
) {
    // Character set validation
    if let Some(charset) = &rules.character_set {
        for (i, ch) in fd_content.chars().enumerate() {
            if !char_in_set(ch, charset) {
                issues.push(Diagnostic::error(
                    codes::BARCODE_INVALID_CHAR,
                    format!(
                        "invalid character '{}' at position {} in {} field data (allowed: [{}])",
                        ch, i, barcode_code, charset
                    ),
                    dspan,
                ).with_context(ctx!(
                    "command" => barcode_code,
                    "character" => ch.to_string(),
                    "position" => i.to_string(),
                    "allowedSet" => charset.clone(),
                )));
                // Only report the first invalid character to avoid flooding
                break;
            }
        }
    }

    // Length validation
    let len = fd_content.chars().count();

    // allowedLengths takes precedence over exact/min/max.
    if let Some(allowed) = &rules.allowed_lengths {
        if !allowed.contains(&len) {
            let expected = allowed
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            issues.push(
                Diagnostic::warn(
                    codes::BARCODE_DATA_LENGTH,
                    format!(
                        "{} field data length {} (expected one of [{}])",
                        barcode_code, len, expected
                    ),
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => len.to_string(),
                    "expected" => expected,
                )),
            );
        }
    // exactLength takes precedence over min/max.
    } else if let Some(exact) = rules.exact_length {
        if len != exact {
            issues.push(
                Diagnostic::warn(
                    codes::BARCODE_DATA_LENGTH,
                    format!(
                        "{} field data length {} (expected exactly {})",
                        barcode_code, len, exact
                    ),
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => len.to_string(),
                    "expected" => exact.to_string(),
                )),
            );
        }
    } else {
        if let Some(min) = rules.min_length
            && len < min
        {
            issues.push(
                Diagnostic::warn(
                    codes::BARCODE_DATA_LENGTH,
                    format!(
                        "{} field data too short: {} chars (minimum {})",
                        barcode_code, len, min
                    ),
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => len.to_string(),
                    "min" => min.to_string(),
                )),
            );
        }
        if let Some(max) = rules.max_length
            && len > max
        {
            issues.push(
                Diagnostic::warn(
                    codes::BARCODE_DATA_LENGTH,
                    format!(
                        "{} field data too long: {} chars (maximum {})",
                        barcode_code, len, max
                    ),
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => len.to_string(),
                    "max" => max.to_string(),
                )),
            );
        }
    }

    // Length parity
    if let Some(parity) = &rules.length_parity {
        let even = len.is_multiple_of(2);
        let valid = match parity.as_str() {
            "even" => even,
            "odd" => !even,
            _ => true,
        };
        if !valid {
            issues.push(
                Diagnostic::warn(
                    codes::BARCODE_DATA_LENGTH,
                    format!(
                        "{} field data length {} should be {} (got {})",
                        barcode_code,
                        len,
                        parity,
                        if even { "even" } else { "odd" }
                    ),
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => len.to_string(),
                    "parity" => parity.clone(),
                )),
            );
        }
    }
}

/// Check if a character is in a compact character set notation.
///
/// Supports:
/// - Ranges: `A-Z`, `0-9`, `a-z`
/// - Literal characters (including space)
/// - Backslash-escaped characters: `\\-` for literal dash, `\\.` for literal dot
///
/// # Limitations
///
/// This function operates on ASCII bytes only. All charset strings in the
/// barcode spec files use ASCII characters exclusively. Multi-byte UTF-8
/// characters in the charset string would be misinterpreted. If non-ASCII
/// charset support is ever needed, this should be rewritten to iterate
/// over `char` values instead of raw bytes.
fn char_in_set(ch: char, charset: &str) -> bool {
    let bytes = charset.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            // Escaped literal character
            if ch == bytes[i + 1] as char {
                return true;
            }
            i += 2;
            continue;
        }
        // Check for range pattern: X-Y
        if i + 2 < bytes.len() && bytes[i + 1] == b'-' && bytes[i + 2] != b'\\' {
            let lo = bytes[i] as char;
            let hi = bytes[i + 2] as char;
            // Handle both normal (lo <= hi) and reversed (lo > hi) ranges
            let (actual_lo, actual_hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
            if ch >= actual_lo && ch <= actual_hi {
                return true;
            }
            i += 3;
            continue;
        }
        // Literal character (including space)
        if ch == bytes[i] as char {
            return true;
        }
        i += 1;
    }
    false
}

// ─── Validation context structs ─────────────────────────────────────────────

/// Shared validation context that replaces repeated function parameters.
///
/// Groups the immutable references that are threaded through all validation
/// functions, reducing function signatures from 8-9 parameters to 2-3.
struct ValidationContext<'a> {
    profile: Option<&'a Profile>,
    label_nodes: &'a [crate::grammar::ast::Node],
    label_codes: &'a HashSet<&'a str>,
    device_state: &'a DeviceState,
}

/// Per-command context for validation functions.
struct CommandCtx<'a> {
    code: &'a str,
    args: &'a [crate::grammar::ast::ArgSlot],
    cmd: &'a zpl_toolchain_spec_tables::CommandEntry,
    span: Option<Span>,
    node_idx: usize,
}

// ─── Extracted sub-functions ────────────────────────────────────────────────

/// Check numeric range constraints on an argument value.
fn validate_arg_range(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    lookup_key: &str,
    val: &str,
    spec_arg: &zpl_toolchain_spec_tables::Arg,
    issues: &mut Vec<Diagnostic>,
) {
    let mut active_range: Option<[f64; 2]> = spec_arg.range;
    if let Some(conds) = spec_arg.range_when.as_ref() {
        for cr in conds {
            if predicate_matches(&cr.when, cmd_ctx.args) {
                active_range = Some(cr.range);
            }
        }
    }
    if let Some([lo, hi]) = active_range
        && let Ok(n) = val.parse::<f64>()
    {
        // Convert user value to dots if the arg is dot-based and units are non-dot
        let effective_n =
            if spec_arg.unit.as_deref() == Some("dots") && vctx.device_state.units != Units::Dots {
                if let Some(dpi) = vctx.device_state.dpi {
                    convert_to_dots(n, vctx.device_state.units, dpi)
                } else {
                    // Without DPI, we can't convert — skip range check
                    return;
                }
            } else {
                n
            };

        if effective_n < lo || effective_n > hi {
            issues.push(
                Diagnostic::error(
                    codes::OUT_OF_RANGE,
                    format!(
                        "{}.{} out of range [{},{}]",
                        cmd_ctx.code,
                        lookup_key,
                        trim_f64(lo),
                        trim_f64(hi)
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!(
                    "command" => cmd_ctx.code,
                    "arg" => lookup_key,
                    "value" => val,
                    "min" => trim_f64(lo),
                    "max" => trim_f64(hi),
                )),
            );
        }
    }
}

/// Check string length constraints on an argument value.
fn validate_arg_length(
    cmd_ctx: &CommandCtx,
    lookup_key: &str,
    val: &str,
    spec_arg: &zpl_toolchain_spec_tables::Arg,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(minl) = spec_arg.min_length
        && (val.len() as u32) < minl
    {
        issues.push(
            Diagnostic::error(
                codes::STRING_TOO_SHORT,
                format!(
                    "{}.{} shorter than minLength {}",
                    cmd_ctx.code, lookup_key, minl
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "arg" => lookup_key,
                "value" => val,
                "min_length" => minl.to_string(),
                "actual_length" => val.len().to_string(),
            )),
        );
    }
    if let Some(maxl) = spec_arg.max_length
        && (val.len() as u32) > maxl
    {
        issues.push(
            Diagnostic::error(
                codes::STRING_TOO_LONG,
                format!("{}.{} exceeds maxLength {}", cmd_ctx.code, lookup_key, maxl),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "arg" => lookup_key,
                "value" => val,
                "max_length" => maxl.to_string(),
                "actual_length" => val.len().to_string(),
            )),
        );
    }
}

/// Check rounding policy constraints on an argument value.
fn validate_arg_rounding(
    cmd_ctx: &CommandCtx,
    lookup_key: &str,
    val: &str,
    spec_arg: &zpl_toolchain_spec_tables::Arg,
    issues: &mut Vec<Diagnostic>,
) {
    let mut rp: Option<zpl_toolchain_spec_tables::RoundingPolicy> =
        spec_arg.rounding_policy.clone();
    if let Some(rpw) = spec_arg.rounding_policy_when.as_ref() {
        for c in rpw {
            if predicate_matches(&c.when, cmd_ctx.args) {
                rp = Some(zpl_toolchain_spec_tables::RoundingPolicy {
                    unit: None,
                    mode: c.mode,
                    multiple: c.multiple,
                });
            }
        }
    }
    if let Some(pol) = rp
        && pol.mode == RoundingMode::ToMultiple
        && let (Ok(n), Some(m)) = (val.parse::<f64>(), pol.multiple)
        && m > 0.0
    {
        let rem = (n / m).fract();
        // Both conditions needed: rem > ε catches non-multiples,
        // (1.0 - rem) > ε handles floating-point imprecision where
        // an exact multiple produces fract() ≈ 0.999999... instead of 0.0
        if rem > 1e-9 && (1.0 - rem) > 1e-9 {
            issues.push(
                Diagnostic::warn(
                    codes::ROUNDING_VIOLATION,
                    format!(
                        "{}.{}={} not a multiple of {}",
                        cmd_ctx.code,
                        lookup_key,
                        trim_f64(n),
                        trim_f64(m)
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!(
                    "command" => cmd_ctx.code,
                    "arg" => lookup_key,
                    "value" => val,
                    "multiple" => trim_f64(m),
                )),
            );
        }
    }
}

/// Check profile constraint on an argument value.
fn validate_arg_profile_constraint(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    lookup_key: &str,
    val: &str,
    spec_arg: &zpl_toolchain_spec_tables::Arg,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(pc) = &spec_arg.profile_constraint
        && let Some(p) = vctx.profile
        && let Ok(n) = val.parse::<f64>()
        && let Some(limit) = resolve_profile_field(p, &pc.field)
    {
        let effective_n =
            if spec_arg.unit.as_deref() == Some("dots") && vctx.device_state.units != Units::Dots {
                if let Some(dpi) = vctx.device_state.dpi {
                    convert_to_dots(n, vctx.device_state.units, dpi)
                } else {
                    // Without DPI we cannot reliably compare against dot-based profile limits.
                    return;
                }
            } else {
                n
            };

        if check_profile_op(effective_n, &pc.op, limit) {
            return;
        }

        let op_desc = match pc.op {
            ComparisonOp::Lte => "exceeds",
            ComparisonOp::Gte => "below",
            ComparisonOp::Lt => "exceeds or equals",
            ComparisonOp::Gt => "below or equals",
            ComparisonOp::Eq => "violates",
        };
        issues.push(
            Diagnostic::error(
                codes::PROFILE_CONSTRAINT,
                format!(
                    "{}.{} {} profile {} ({})",
                    cmd_ctx.code,
                    lookup_key,
                    op_desc,
                    pc.field,
                    trim_f64(limit),
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "arg" => lookup_key,
                "field" => pc.field.clone(),
                "op" => format!("{:?}", pc.op),
                "limit" => trim_f64(limit),
                "actual" => trim_f64(effective_n),
            )),
        );
    }
}

/// Check enum value printer gate constraints on an argument value.
fn validate_arg_enum_gates(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    lookup_key: &str,
    val: &str,
    spec_arg: &zpl_toolchain_spec_tables::Arg,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(ref enum_values) = spec_arg.r#enum
        && let Some(p) = vctx.profile
        && let Some(ref features) = p.features
    {
        for ev in enum_values {
            if let zpl_toolchain_spec_tables::EnumValue::Object {
                value: ev_val,
                printer_gates: Some(gates),
                ..
            } = ev
                && ev_val == val
            {
                for gate in gates {
                    if let Some(false) = zpl_toolchain_profile::resolve_gate(features, gate) {
                        issues.push(Diagnostic::warn(
                            codes::PRINTER_GATE,
                            format!(
                                "{}.{}={} requires '{}' capability not available in profile '{}'",
                                cmd_ctx.code,
                                lookup_key,
                                val,
                                gate,
                                &p.id
                            ),
                            cmd_ctx.span,
                        ).with_context(ctx!(
                            "command" => cmd_ctx.code,
                            "arg" => lookup_key,
                            "value" => val,
                            "gate" => gate.clone(),
                            "level" => "enum",
                            "profile" => &p.id,
                        )));
                    }
                }
            }
        }
    }
}

/// Validate a single argument's value: type, range, length, rounding, profile, gates.
fn validate_arg_value(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    lookup_key: &str,
    val: &str,
    spec_arg: &zpl_toolchain_spec_tables::Arg,
    issues: &mut Vec<Diagnostic>,
) {
    // Type validation — determines if value-based checks should proceed
    //
    // type_valid stays true even for invalid enums — this is intentional.
    // Enum validation is reported independently; range/length checks still
    // run to surface all issues at once rather than requiring fix-and-retry.
    let type_valid = match spec_arg.r#type.as_str() {
        "enum" => {
            if let Some(ev) = spec_arg.r#enum.as_ref() {
                let ok = enum_contains(ev, val);
                if !ok {
                    issues.push(
                        Diagnostic::error(
                            codes::INVALID_ENUM,
                            format!("{}.{} invalid enum", cmd_ctx.code, lookup_key),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!(
                            "command" => cmd_ctx.code,
                            "arg" => lookup_key,
                            "value" => val,
                        )),
                    );
                }
            }
            true
        }
        "int" => {
            val.parse::<i64>().is_ok() || {
                issues.push(
                    Diagnostic::error(
                        codes::EXPECTED_INTEGER,
                        format!(
                            "{}.{} expected integer, got \"{}\"",
                            cmd_ctx.code, lookup_key, val
                        ),
                        cmd_ctx.span,
                    )
                    .with_context(ctx!(
                        "command" => cmd_ctx.code,
                        "arg" => lookup_key,
                        "value" => val,
                    )),
                );
                false
            }
        }
        "float" => {
            val.parse::<f64>().is_ok() || {
                issues.push(
                    Diagnostic::error(
                        codes::EXPECTED_NUMERIC,
                        format!(
                            "{}.{} expected number, got \"{}\"",
                            cmd_ctx.code, lookup_key, val
                        ),
                        cmd_ctx.span,
                    )
                    .with_context(ctx!(
                        "command" => cmd_ctx.code,
                        "arg" => lookup_key,
                        "value" => val,
                    )),
                );
                false
            }
        }
        "char" => {
            val.chars().count() == 1 || {
                issues.push(
                    Diagnostic::error(
                        codes::EXPECTED_CHAR,
                        format!(
                            "{}.{} expected single character, got \"{}\"",
                            cmd_ctx.code, lookup_key, val
                        ),
                        cmd_ctx.span,
                    )
                    .with_context(ctx!(
                        "command" => cmd_ctx.code,
                        "arg" => lookup_key,
                        "value" => val,
                    )),
                );
                false
            }
        }
        _ => true,
    };

    if !type_valid {
        return;
    }

    // Numeric range
    validate_arg_range(cmd_ctx, vctx, lookup_key, val, spec_arg, issues);

    // String length constraints
    validate_arg_length(cmd_ctx, lookup_key, val, spec_arg, issues);

    // Rounding policy
    validate_arg_rounding(cmd_ctx, lookup_key, val, spec_arg, issues);

    // Profile constraint
    validate_arg_profile_constraint(cmd_ctx, vctx, lookup_key, val, spec_arg, issues);

    // Enum value printer gates
    validate_arg_enum_gates(cmd_ctx, vctx, lookup_key, val, spec_arg, issues);
}

fn value_to_arg_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(if *b { "Y".to_string() } else { "N".to_string() }),
        _ => None,
    }
}

fn resolve_effective_default_value(
    arg: &zpl_toolchain_spec_tables::Arg,
    vctx: &ValidationContext,
    label_state: &LabelState,
) -> Option<String> {
    if let Some(df) = arg.default_from.as_deref()
        && label_state.has_producer(df)
        && let Some(key) = arg.default_from_state_key.as_deref()
        && let Some(v) = label_state.value_state.state_value_by_key(key)
    {
        return Some(v);
    }

    if let Some(map) = arg.default_by_dpi.as_ref()
        && let Some(dpi) = vctx.profile.map(|p| p.dpi)
        && let Some(v) = map.get(&dpi.to_string()).and_then(value_to_arg_string)
    {
        return Some(v);
    }

    arg.default.as_ref().and_then(value_to_arg_string)
}

/// Validate command arguments: presence, enum, type, range, length, rounding, profile.
fn validate_command_args(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    let Some(spec_args) = cmd_ctx.cmd.args.as_ref() else {
        return;
    };

    let mut key_to_slot: HashMap<String, &crate::grammar::ast::ArgSlot> = HashMap::new();
    for (idx, slot) in cmd_ctx.args.iter().enumerate() {
        key_to_slot.insert(idx.to_string(), slot);
        if let Some(k) = slot.key.as_ref() {
            key_to_slot.insert(k.clone(), slot);
        }
    }

    for (idx, spec_arg) in spec_args.iter().enumerate() {
        let lookup_key = idx.to_string();
        let slot_opt = key_to_slot.get(&lookup_key).copied();
        let eff = select_effective_arg(spec_arg, slot_opt);
        let resolved_default =
            eff.and_then(|arg| resolve_effective_default_value(arg, vctx, label_state));

        // Presence checks (resolved defaults count as present).
        if let Some(arg) = eff
            && !arg.optional
        {
            let has_static_default = arg.default.is_some()
                || arg.default_by_dpi.as_ref().is_some_and(|m| {
                    vctx.profile
                        .map(|p| p.dpi)
                        .is_some_and(|d| m.contains_key(&d.to_string()))
                });
            // Presence should only be satisfied by defaults we can actually resolve.
            let has_any_default = has_static_default || resolved_default.is_some();

            match slot_opt {
                None if !has_any_default => {
                    issues.push(
                        Diagnostic::error(
                            codes::REQUIRED_MISSING,
                            format!("{}.{} is required but missing", cmd_ctx.code, lookup_key),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!(
                            "command" => cmd_ctx.code,
                            "arg" => lookup_key.clone(),
                        )),
                    );
                }
                Some(slot) => {
                    if slot.presence == crate::grammar::ast::Presence::Unset && !has_any_default {
                        issues.push(
                            Diagnostic::error(
                                codes::REQUIRED_MISSING,
                                format!("{}.{} is required but unset", cmd_ctx.code, lookup_key),
                                cmd_ctx.span,
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "arg" => lookup_key.clone(),
                            )),
                        );
                    } else if slot.presence == crate::grammar::ast::Presence::Empty
                        && !has_any_default
                    {
                        issues.push(
                            Diagnostic::warn(
                                codes::REQUIRED_EMPTY,
                                format!("{}.{} is empty but required", cmd_ctx.code, lookup_key),
                                cmd_ctx.span,
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "arg" => lookup_key.clone(),
                            )),
                        );
                    }
                }
                _ => {}
            }
        }

        // Value-based checks
        if let (Some(slot), Some(spec_arg)) = (slot_opt, eff)
            && let Some(val) = slot.value.as_ref()
        {
            validate_arg_value(cmd_ctx, vctx, &lookup_key, val, spec_arg, issues);
        } else if let (Some(spec_arg), Some(default_val)) = (eff, resolved_default.as_ref()) {
            // Validate resolved defaults too, so producer-provided values obey
            // the same type/range/profile rules as explicit args.
            validate_arg_value(cmd_ctx, vctx, &lookup_key, default_val, spec_arg, issues);
        }
    }
}

/// Validate command constraints: order, requires, incompatible, empty data, notes.
fn validate_command_constraints(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    seen_label_codes: &HashSet<&str>,
    seen_field_codes: &HashSet<&str>,
    current_field_codes: Option<&HashSet<&str>>,
    issues: &mut Vec<Diagnostic>,
) {
    let Some(constraints) = cmd_ctx.cmd.constraints.as_ref() else {
        return;
    };

    for c in constraints {
        match c.kind {
            ConstraintKind::Order => {
                if let Some(expr) = c.expr.as_ref() {
                    // Constraint scope precedence:
                    // 1) explicit constraint scope
                    // 2) command scope fallback (field commands default to field-local ordering)
                    // 3) label-wide default
                    let eval_scope = c.scope.unwrap_or_else(|| {
                        if cmd_ctx.cmd.scope == Some(CommandScope::Field) {
                            ConstraintScope::Field
                        } else {
                            ConstraintScope::Label
                        }
                    });
                    let seen_codes = if eval_scope == ConstraintScope::Field {
                        seen_field_codes
                    } else {
                        seen_label_codes
                    };
                    if let Some(targets) = expr.strip_prefix("before:") {
                        if any_target_in_set(targets, seen_codes) {
                            issues.push(
                                Diagnostic::new(
                                    codes::ORDER_BEFORE,
                                    map_sev(c.severity.as_ref()),
                                    c.message.clone(),
                                    cmd_ctx.span,
                                )
                                .with_context(ctx!(
                                    "command" => cmd_ctx.code,
                                    "target" => targets,
                                    "kind" => "order",
                                    "scope" => if eval_scope == ConstraintScope::Field { "field" } else { "label" },
                                )),
                            );
                        }
                    } else if let Some(targets) = expr.strip_prefix("after:")
                        && !any_target_in_set(targets, seen_codes)
                    {
                        issues.push(
                            Diagnostic::new(
                                codes::ORDER_AFTER,
                                map_sev(c.severity.as_ref()),
                                c.message.clone(),
                                cmd_ctx.span,
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "target" => targets,
                                "kind" => "order",
                                "scope" => if eval_scope == ConstraintScope::Field { "field" } else { "label" },
                            )),
                        );
                    }
                }
            }
            ConstraintKind::Requires => {
                if let Some(expr) = c.expr.as_ref() {
                    // Keep requires label-scoped by default for backward compatibility.
                    // Authors can opt into field-local semantics via constraint.scope.
                    let eval_scope = c.scope.unwrap_or(ConstraintScope::Label);
                    let empty_field_codes: HashSet<&str> = HashSet::new();
                    let target_codes = if eval_scope == ConstraintScope::Field {
                        current_field_codes.unwrap_or(&empty_field_codes)
                    } else {
                        vctx.label_codes
                    };
                    if !any_target_in_set(expr, target_codes) {
                        issues.push(
                            Diagnostic::new(
                                codes::REQUIRED_COMMAND,
                                map_sev(c.severity.as_ref()),
                                c.message.clone(),
                                cmd_ctx.span,
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "target" => expr.clone(),
                                "kind" => "requires",
                                "scope" => if eval_scope == ConstraintScope::Field { "field" } else { "label" },
                            )),
                        );
                    }
                }
            }
            ConstraintKind::Incompatible => {
                if let Some(expr) = c.expr.as_ref() {
                    // Keep incompatible label-scoped by default for backward compatibility.
                    // Authors can opt into field-local semantics via constraint.scope.
                    let eval_scope = c.scope.unwrap_or(ConstraintScope::Label);
                    let empty_field_codes: HashSet<&str> = HashSet::new();
                    let target_codes = if eval_scope == ConstraintScope::Field {
                        current_field_codes.unwrap_or(&empty_field_codes)
                    } else {
                        vctx.label_codes
                    };
                    if any_target_in_set(expr, target_codes) {
                        issues.push(
                            Diagnostic::new(
                                codes::INCOMPATIBLE_COMMAND,
                                map_sev(c.severity.as_ref()),
                                c.message.clone(),
                                cmd_ctx.span,
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "target" => expr.clone(),
                                "kind" => "incompatible",
                                "scope" => if eval_scope == ConstraintScope::Field { "field" } else { "label" },
                            )),
                        );
                    }
                }
            }
            ConstraintKind::EmptyData => {
                // Check both the command's args and any non-empty following
                // FieldData content up to the next command boundary.
                let fd_has_content = cmd_ctx
                    .args
                    .first()
                    .and_then(|a| a.value.as_ref())
                    .is_some_and(|s| !s.is_empty());
                let mut trailing_fd_has_content = false;
                for n in &vctx.label_nodes[(cmd_ctx.node_idx + 1).min(vctx.label_nodes.len())..] {
                    match n {
                        crate::grammar::ast::Node::FieldData { content, .. } => {
                            if !content.is_empty() {
                                trailing_fd_has_content = true;
                                break;
                            }
                        }
                        crate::grammar::ast::Node::Command { .. } => break,
                        _ => {}
                    }
                }
                if !fd_has_content && !trailing_fd_has_content {
                    issues.push(
                        Diagnostic::new(
                            codes::EMPTY_FIELD_DATA,
                            map_sev(c.severity.as_ref()),
                            c.message.clone(),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!("command" => cmd_ctx.code)),
                    );
                }
            }
            ConstraintKind::Note => {
                // Optional note predicates (spec-driven):
                // - after:first:<codes>
                // - before:first:<codes>
                // - after:<codes>
                // - before:<codes>
                // where <codes> can be a single command or comma-separated list.
                let should_emit = if let Some(expr) = c.expr.as_deref() {
                    let eval_scope = c.scope.unwrap_or_else(|| {
                        if cmd_ctx.cmd.scope == Some(CommandScope::Field) {
                            ConstraintScope::Field
                        } else {
                            ConstraintScope::Label
                        }
                    });
                    let seen_codes = if eval_scope == ConstraintScope::Field {
                        seen_field_codes
                    } else {
                        seen_label_codes
                    };

                    if let Some(targets) = expr.strip_prefix("after:first:") {
                        any_target_in_set(targets, seen_codes)
                    } else if let Some(targets) = expr.strip_prefix("before:first:") {
                        !any_target_in_set(targets, seen_codes)
                    } else if let Some(targets) = expr.strip_prefix("after:") {
                        any_target_in_set(targets, seen_codes)
                    } else if let Some(targets) = expr.strip_prefix("before:") {
                        !any_target_in_set(targets, seen_codes)
                    } else {
                        true
                    }
                } else {
                    true
                };
                if !should_emit {
                    continue;
                }
                issues.push(
                    Diagnostic::new(
                        codes::NOTE,
                        map_sev(c.severity.as_ref()),
                        c.message.clone(),
                        cmd_ctx.span,
                    )
                    .with_context(ctx!("command" => cmd_ctx.code)),
                );
            }
            // Range constraints are a future extension point — currently, range
            // validation is handled through `args[].range` on each Arg definition.
            // When activated, the constraint's `expr` would specify the range
            // and `message` would provide context.
            ConstraintKind::Range | ConstraintKind::Custom => {}
        }
    }
}

// ─── Semantic sub-functions (B3) ────────────────────────────────────────────

/// ZPL2301: Duplicate ^FN field number detection.
fn validate_field_number(
    cmd_ctx: &CommandCtx,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    if cmd_ctx.code == "^FN"
        && let Some(slot) = cmd_ctx.args.first()
        && let Some(n) = slot.value.as_ref()
    {
        if let Some(&first_idx) = label_state.field_numbers.get(n) {
            issues.push(
                Diagnostic::warn(
                    codes::DUPLICATE_FIELD_NUMBER,
                    format!(
                        "Duplicate field number {} (first used at node {})",
                        n, first_idx
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!("command" => cmd_ctx.code, "field_number" => n.clone())),
            );
        } else {
            label_state
                .field_numbers
                .insert(n.clone(), cmd_ctx.node_idx);
        }
    }
}

/// ZPL2302: ^PW/^LL tracking + position bounds checking for ^FO/^FT.
fn validate_position_bounds(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    // Track ^PW and ^LL values for position bounds checking from typed state.
    if cmd_ctx.code == "^PW" {
        if let Some(w) = label_state.value_state.layout.print_width {
            label_state.effective_width = Some(w);
        }
        label_state.has_explicit_pw = true;
    }
    if cmd_ctx.code == "^LL" {
        if let Some(h) = label_state.value_state.layout.label_length {
            label_state.effective_height = Some(h);
        }
        label_state.has_explicit_ll = true;
        // Profile height check is handled by the generic profileConstraint
        // mechanism in validate_command_args() — no hardcoded check here.
    }

    // Track ^FO position for graphic bounds checking (ZPL2308)
    if cmd_ctx.code == "^FO" || cmd_ctx.code == "^FT" {
        // Reset to defaults before parsing — ZPL defaults to (0,0)
        label_state.last_fo_x = Some(label_state.value_state.label_home.x);
        label_state.last_fo_y = Some(label_state.value_state.label_home.y);
        if let Some(x_slot) = cmd_ctx.args.first()
            && let Some(x_val) = x_slot.value.as_ref()
            && let Ok(x) = x_val.parse::<f64>()
        {
            // Normalize to dots for consistent bounds comparison (ZPL2308).
            // When ^MU sets inches/mm, ^FO values are in those units, but
            // ^GF dimensions and profile bounds are always in dots.
            label_state.last_fo_x = Some(
                if let Some(dpi) = vctx.device_state.dpi {
                    convert_to_dots(x, vctx.device_state.units, dpi)
                } else {
                    x
                } + label_state.value_state.label_home.x,
            );
        }
        if let Some(y_slot) = cmd_ctx.args.get(1)
            && let Some(y_val) = y_slot.value.as_ref()
            && let Ok(y) = y_val.parse::<f64>()
        {
            label_state.last_fo_y = Some(
                if let Some(dpi) = vctx.device_state.dpi {
                    convert_to_dots(y, vctx.device_state.units, dpi)
                } else {
                    y
                } + label_state.value_state.label_home.y,
            );
        }
    }

    if cmd_ctx.code != "^FO" && cmd_ctx.code != "^FT" {
        return;
    }

    // Determine effective bounds: label ^PW/^LL > profile > none
    let max_x = label_state.effective_width.or_else(|| {
        vctx.profile
            .and_then(|p| resolve_profile_field(p, "page.width_dots"))
    });
    let max_y = label_state.effective_height.or_else(|| {
        vctx.profile
            .and_then(|p| resolve_profile_field(p, "page.height_dots"))
    });

    if let (Some(fo_x), Some(w)) = (label_state.last_fo_x, max_x)
        && fo_x > w
    {
        issues.push(
            Diagnostic::warn(
                codes::POSITION_OUT_OF_BOUNDS,
                format!(
                    "{} x position {} exceeds label width {}",
                    cmd_ctx.code,
                    trim_f64(fo_x),
                    trim_f64(w)
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "axis" => "x",
                "value" => trim_f64(fo_x),
                "limit" => trim_f64(w),
            )),
        );
    }
    if let (Some(fo_y), Some(h)) = (label_state.last_fo_y, max_y)
        && fo_y > h
    {
        issues.push(
            Diagnostic::warn(
                codes::POSITION_OUT_OF_BOUNDS,
                format!(
                    "{} y position {} exceeds label height {}",
                    cmd_ctx.code,
                    trim_f64(fo_y),
                    trim_f64(h)
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "axis" => "y",
                "value" => trim_f64(fo_y),
                "limit" => trim_f64(h),
            )),
        );
    }
}

/// ZPL2303: Font reference validation for ^A + ^CW tracking.
fn validate_font_reference(
    cmd_ctx: &CommandCtx,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    // ^A font validation
    if cmd_ctx.code == "^A"
        && let Some(slot) = cmd_ctx.args.first()
        && let Some(v) = slot.value.as_ref()
        && let Some(font_char) = v.chars().next()
    {
        let is_builtin = font_char.is_ascii_uppercase() || font_char.is_ascii_digit();
        let is_loaded = label_state.loaded_fonts.contains(&font_char);
        if !is_builtin && !is_loaded {
            issues.push(Diagnostic::warn(
                codes::UNKNOWN_FONT,
                format!("^A font '{}' is not a built-in font (A-Z, 0-9) and has not been loaded via ^CW", font_char),
                cmd_ctx.span,
            ).with_context(ctx!("command" => cmd_ctx.code, "font" => font_char.to_string())));
        }
    }

    // Track ^CW font registrations
    if cmd_ctx.code == "^CW"
        && let Some(slot) = cmd_ctx.args.first()
        && let Some(v) = slot.value.as_ref()
        && let Some(ch) = v.chars().next()
    {
        label_state.loaded_fonts.insert(ch);
    }
}

/// ZPL1403: Media mode validation (^MM, ^MN, ^MT).
fn validate_media_modes(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    issues: &mut Vec<Diagnostic>,
) {
    // ^MM mode vs media.supported_modes
    if cmd_ctx.code == "^MM"
        && let Some(slot) = cmd_ctx.args.first()
        && let Some(val) = slot.value.as_ref()
        && let Some(p) = vctx.profile
        && let Some(ref media) = p.media
        && let Some(ref modes) = media.supported_modes
        && !modes.is_empty()
        && !modes.iter().any(|m| m == val)
    {
        issues.push(
            Diagnostic::warn(
                codes::MEDIA_MODE_UNSUPPORTED,
                format!(
                    "^MM mode '{}' is not in profile's supported_modes {:?}",
                    val, modes
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => "^MM",
                "kind" => "mode",
                "value" => val.clone(),
                "supported" => format!("{:?}", modes),
                "profile" => &p.id,
            )),
        );
    }

    // ^MN tracking mode vs media.supported_tracking
    if cmd_ctx.code == "^MN"
        && let Some(slot) = cmd_ctx.args.first()
        && let Some(val) = slot.value.as_ref()
        && let Some(p) = vctx.profile
        && let Some(ref media) = p.media
        && let Some(ref tracking) = media.supported_tracking
        && !tracking.is_empty()
        && !tracking.iter().any(|t| t == val)
    {
        issues.push(
            Diagnostic::warn(
                codes::MEDIA_MODE_UNSUPPORTED,
                format!(
                    "^MN tracking mode '{}' is not in profile's supported_tracking {:?}",
                    val, tracking
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => "^MN",
                "kind" => "tracking",
                "value" => val.clone(),
                "supported" => format!("{:?}", tracking),
                "profile" => &p.id,
            )),
        );
    }

    // ^MT print method vs media.print_method
    if cmd_ctx.code == "^MT"
        && let Some(slot) = cmd_ctx.args.first()
        && let Some(val) = slot.value.as_ref()
        && let Some(p) = vctx.profile
        && let Some(ref media) = p.media
        && let Some(ref method) = media.print_method
    {
        let compatible = match method {
            zpl_toolchain_profile::PrintMethod::Both => true,
            zpl_toolchain_profile::PrintMethod::DirectThermal => val == "D",
            zpl_toolchain_profile::PrintMethod::ThermalTransfer => val == "T",
        };
        if !compatible {
            issues.push(
                Diagnostic::warn(
                    codes::MEDIA_MODE_UNSUPPORTED,
                    format!(
                        "^MT media type '{}' conflicts with profile print method '{:?}'",
                        val, method
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!(
                    "command" => "^MT",
                    "kind" => "method",
                    "value" => val.clone(),
                    "profile_method" => format!("{:?}", method),
                    "profile" => &p.id,
                )),
            );
        }
    }
}

/// ZPL2307: ^GF data length validation.
fn validate_gf_data_length(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    issues: &mut Vec<Diagnostic>,
) {
    // Compare actual data length against declared binary_byte_count.
    // For multi-line payloads, the parser creates Node::RawData continuation
    // nodes after the ^GF command node. We accumulate data from both the
    // command's args[4] and any trailing RawData nodes.
    //
    // ^GF arg layout: [0]=compression (a), [1]=binary_byte_count (b),
    //   [2]=graphic_field_count (c), [3]=bytes_per_row (d), [4]=data
    // See spec/commands/^GF.jsonc for full definition.
    if cmd_ctx.code != "^GF" || cmd_ctx.args.len() < 5 {
        return;
    }

    let compression = cmd_ctx.args[0].value.as_deref().unwrap_or("A"); // compression format (A=ASCII hex, B=binary, C=compressed)
    let byte_count_val = cmd_ctx.args[1].value.as_deref(); // declared binary byte count
    let data_val = cmd_ctx.args[4].value.as_deref(); // raw graphic data (inline portion)

    if let (Some(bc_str), Some(data)) = (byte_count_val, data_val)
        && let Ok(declared) = bc_str.parse::<usize>()
    {
        // Accumulate total data length: inline args[4] + any RawData continuation nodes.
        // For ASCII hex (A) and compressed (C) formats, whitespace (newlines, spaces)
        // in multi-line data is not significant and must be excluded from the count.
        // For binary (B), all bytes are significant.
        let strip_ws = compression != "B";
        let effective_len = |s: &str| -> usize {
            if strip_ws {
                s.bytes().filter(|b| !b.is_ascii_whitespace()).count()
            } else {
                s.len()
            }
        };
        let mut total_data_len = effective_len(data);
        for continuation in &vctx.label_nodes[cmd_ctx.node_idx + 1..] {
            if let crate::grammar::ast::Node::RawData {
                command,
                data: raw_data,
                ..
            } = continuation
            {
                if command == "^GF" {
                    total_data_len += raw_data.as_deref().map_or(0, &effective_len);
                } else {
                    break; // Different command's raw data — stop accumulating
                }
            } else {
                break; // Non-RawData node — stop accumulating
            }
        }

        let mismatch = match compression {
            "A" => {
                // ASCII hex: 2 hex chars per byte
                let expected = declared * 2;
                if total_data_len != expected {
                    Some((total_data_len, expected, "ASCII hex (2 chars per byte)"))
                } else {
                    None
                }
            }
            "B" => {
                // Binary: 1 byte per byte
                if total_data_len != declared {
                    Some((total_data_len, declared, "binary (1:1)"))
                } else {
                    None
                }
            }
            // "C" = compressed binary — skip (decompression needed)
            _ => None,
        };

        if let Some((actual_len, expected_len, fmt)) = mismatch {
            issues.push(Diagnostic::error(
                codes::GF_DATA_LENGTH_MISMATCH,
                format!(
                    "^GF data length mismatch: declared {} bytes ({}), but data is {} chars (expected {})",
                    declared, fmt, actual_len, expected_len
                ),
                cmd_ctx.span,
            ).with_context(ctx!(
                "command" => cmd_ctx.code,
                "format" => compression,
                "declared" => declared.to_string(),
                "actual" => actual_len.to_string(),
                "expected" => expected_len.to_string(),
            )));
        }
    }
}

/// ZPL2308: ^GF graphic bounds check + ZPL2309: accumulate graphic bytes.
///
/// When a `^GF` command is encountered:
/// - Accumulates `graphic_field_count` (arg[2]) into `label_state.gf_total_bytes`
///   for memory estimation (ZPL2309 checked in `validate_preflight`).
/// - If position tracking data is available (`last_fo_x`/`last_fo_y`), calculates
///   the graphic dimensions and checks against effective label bounds.
fn validate_gf_preflight_tracking(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    if cmd_ctx.code != "^GF" || cmd_ctx.args.len() < 4 {
        return;
    }

    // arg[2] = graphic_field_count (total bytes), arg[3] = bytes_per_row
    let gfc_val = cmd_ctx.args.get(2).and_then(|s| s.value.as_deref());
    let bpr_val = cmd_ctx.args.get(3).and_then(|s| s.value.as_deref());

    if let Some(gfc_str) = gfc_val
        && let Ok(graphic_field_count) = gfc_str.parse::<u32>()
    {
        // Accumulate for ZPL2309 memory check
        label_state.gf_total_bytes = label_state
            .gf_total_bytes
            .saturating_add(graphic_field_count);

        // ZPL2308: bounds check
        if let Some(bpr_str) = bpr_val
            && let Ok(bytes_per_row) = bpr_str.parse::<u32>()
            && bytes_per_row > 0
        {
            let graphic_width = bytes_per_row.saturating_mul(8);
            let graphic_height = graphic_field_count.div_ceil(bytes_per_row);

            // Determine effective bounds: label ^PW/^LL > profile > none
            let max_x = label_state.effective_width.or_else(|| {
                vctx.profile
                    .and_then(|p| resolve_profile_field(p, "page.width_dots"))
            });
            let max_y = label_state.effective_height.or_else(|| {
                vctx.profile
                    .and_then(|p| resolve_profile_field(p, "page.height_dots"))
            });

            // Skip bounds check when units are non-dots and DPI is unknown —
            // we can't reliably compare since graphic dimensions are in dots
            // but ^PW/^LL values would be in non-dot units.
            let can_check_bounds =
                vctx.device_state.dpi.is_some() || vctx.device_state.units == Units::Dots;

            if can_check_bounds
                && let (Some(fo_x), Some(fo_y)) = (label_state.last_fo_x, label_state.last_fo_y)
            {
                let overflows_x = max_x.is_some_and(|w| fo_x + graphic_width as f64 > w);
                let overflows_y = max_y.is_some_and(|h| fo_y + graphic_height as f64 > h);

                if overflows_x || overflows_y {
                    let ew = max_x.map_or("?".to_string(), trim_f64);
                    let eh = max_y.map_or("?".to_string(), trim_f64);
                    issues.push(
                        Diagnostic::warn(
                            codes::GF_BOUNDS_OVERFLOW,
                            format!(
                                "Graphic field at ({}, {}) extends beyond label bounds ({}×{} dots)",
                                trim_f64(fo_x),
                                trim_f64(fo_y),
                                ew,
                                eh,
                            ),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!(
                            "command" => cmd_ctx.code,
                            "x" => trim_f64(fo_x),
                            "y" => trim_f64(fo_y),
                            "graphic_width" => graphic_width.to_string(),
                            "graphic_height" => graphic_height.to_string(),
                            "label_width" => ew,
                            "label_height" => eh,
                        )),
                    );
                }
            }
        }
    }
}

/// Validate semantic state: cross-command references, field numbers, positions, fonts, data.
fn validate_semantic_state(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    // Mark consumed producers via defaultFrom references
    if let Some(spec_args) = cmd_ctx.cmd.args.as_ref() {
        for sa in spec_args {
            let arg = match sa {
                zpl_toolchain_spec_tables::ArgUnion::Single(a) => Some(a.as_ref()),
                zpl_toolchain_spec_tables::ArgUnion::OneOf { one_of } => one_of.first(),
            };
            if let Some(a) = arg
                && let Some(df) = &a.default_from
            {
                label_state.mark_consumed(df);
            }
        }
    }

    // ZPL2301: Duplicate ^FN field number
    validate_field_number(cmd_ctx, label_state, issues);

    // ZPL2302: ^PW/^LL tracking + position bounds checking for ^FO/^FT
    validate_position_bounds(cmd_ctx, vctx, label_state, issues);

    // ZPL2303: Font reference validation for ^A + ^CW tracking
    validate_font_reference(cmd_ctx, label_state, issues);

    // ZPL1403: Media mode validation
    validate_media_modes(cmd_ctx, vctx, issues);

    // ZPL2307: ^GF data length validation
    validate_gf_data_length(cmd_ctx, vctx, issues);

    // ZPL2308/2309: ^GF bounds check + memory accumulation
    validate_gf_preflight_tracking(cmd_ctx, vctx, label_state, issues);
}

// ─── Post-label preflight checks ────────────────────────────────────────────

/// Preflight validation that runs after all nodes in a label have been processed.
///
/// Checks:
/// - **ZPL2309**: Total graphic memory exceeds available RAM (profile-gated)
/// - **ZPL2310**: Label lacks explicit ^PW/^LL when profile provides dimensions
fn validate_preflight(
    vctx: &ValidationContext,
    label_state: &LabelState,
    label_span: Option<Span>,
    issues: &mut Vec<Diagnostic>,
) {
    // ZPL2309: Graphics memory estimation
    if label_state.gf_total_bytes > 0
        && let Some(profile) = vctx.profile
        && let Some(ram_kb) = resolve_profile_field(profile, "memory.ram_kb")
    {
        let ram_bytes = ram_kb as u64 * 1024;
        if label_state.gf_total_bytes as u64 > ram_bytes {
            issues.push(
                Diagnostic::warn(
                    codes::GF_MEMORY_EXCEEDED,
                    format!(
                        "Total graphic data ({} bytes) exceeds available RAM ({} bytes / {} KB)",
                        label_state.gf_total_bytes, ram_bytes, ram_kb as u64,
                    ),
                    label_span,
                )
                .with_context(ctx!(
                    "command" => "^GF",
                    "total_bytes" => label_state.gf_total_bytes.to_string(),
                    "ram_bytes" => ram_bytes.to_string(),
                )),
            );
        }
    }

    // ZPL2310: Missing explicit dimensions
    if let Some(profile) = vctx.profile {
        let profile_has_width = resolve_profile_field(profile, "page.width_dots").is_some();
        let profile_has_height = resolve_profile_field(profile, "page.height_dots").is_some();

        if (profile_has_width || profile_has_height)
            && (!label_state.has_explicit_pw || !label_state.has_explicit_ll)
        {
            let mut missing = Vec::new();
            if !label_state.has_explicit_pw && profile_has_width {
                missing.push("^PW");
            }
            if !label_state.has_explicit_ll && profile_has_height {
                missing.push("^LL");
            }
            if !missing.is_empty() {
                let missing_str = missing.join(", ");
                issues.push(
                    Diagnostic::info(
                        codes::MISSING_EXPLICIT_DIMENSIONS,
                        format!(
                            "Label relies on profile for dimensions but does not contain explicit {} — consider adding for portability",
                            missing_str,
                        ),
                        label_span,
                    )
                    .with_context(ctx!(
                        "missing_commands" => missing_str,
                    )),
                );
            }
        }
    }
}

// ─── Main validation entry points ──────────────────────────────────────────

/// Validate a ZPL AST using spec tables and an optional printer profile.
///
/// Returns a [`ValidationResult`] containing all diagnostics and an overall pass/fail flag.
pub fn validate_with_profile(
    ast: &Ast,
    tables: &ParserTables,
    profile: Option<&Profile>,
) -> ValidationResult {
    let mut issues = Vec::new();
    let mut resolved_labels = Vec::new();
    let known = tables.code_set();

    let mut device_state = DeviceState::default();
    // Initialize DPI from profile if available
    if let Some(p) = profile {
        device_state.dpi = Some(p.dpi);
    }

    for label in &ast.labels {
        let mut label_state = LabelState::default();

        let mut field_tracker = FieldTracker::default();
        let mut has_printable = false;
        // Incrementally built set of command codes seen so far in this label,
        // used for O(1) order constraint checks instead of O(n) slice scans.
        let mut seen_codes: HashSet<&str> = HashSet::new();
        // Field-local command set used by field-scoped order constraints.
        let mut seen_field_codes: HashSet<&str> = HashSet::new();

        // Precomputed set of ALL command codes in this label — used for O(1)
        // Requires/Incompatible constraint checks instead of O(n) node scans.
        let label_codes: HashSet<&str> = label
            .nodes
            .iter()
            .filter_map(|n| {
                if let crate::grammar::ast::Node::Command { code, .. } = n {
                    Some(code.as_str())
                } else {
                    None
                }
            })
            .collect();

        // Precompute field memberships and full per-field command sets so
        // constraint.scope=field can evaluate against the whole field (not
        // only commands seen so far).
        let mut field_id_by_node: Vec<Option<usize>> = vec![None; label.nodes.len()];
        let mut field_codes: Vec<HashSet<&str>> = Vec::new();
        let mut current_field_id: Option<usize> = None;
        for (idx, node) in label.nodes.iter().enumerate() {
            if let crate::grammar::ast::Node::Command { code, .. } = node {
                if known.contains(code)
                    && let Some(cmd) = tables.cmd_by_code(code)
                    && cmd.opens_field
                {
                    let new_id = field_codes.len();
                    field_codes.push(HashSet::new());
                    current_field_id = Some(new_id);
                }

                if let Some(fid) = current_field_id {
                    field_id_by_node[idx] = Some(fid);
                    if let Some(set) = field_codes.get_mut(fid) {
                        set.insert(code.as_str());
                    }
                }

                if known.contains(code)
                    && let Some(cmd) = tables.cmd_by_code(code)
                    && cmd.closes_field
                {
                    current_field_id = None;
                }
            }
        }

        // Tracks whether the current node position is between ^XA and ^XZ.
        // Parser labels may include pre-^XA commands, which should not be
        // treated as "inside label" for scope diagnostics like ZPL2205.
        let mut inside_format_bounds = false;

        for (node_idx, node) in label.nodes.iter().enumerate() {
            if let crate::grammar::ast::Node::Command { code, args, span } = node {
                let dspan = Some(*span);

                if code == "^XA" {
                    inside_format_bounds = true;
                } else if code == "^XZ" {
                    inside_format_bounds = false;
                }

                // Track printable content for empty-label detection (ZPL2202)
                if !matches!(code.as_str(), "^XA" | "^XZ") {
                    has_printable = true;
                }

                // ─── Command validation (known commands only) ────────────
                if known.contains(code)
                    && let Some(cmd) = tables.cmd_by_code(code)
                {
                    let producer_key = cmd.codes.first().map(String::as_str).unwrap_or(code);
                    if cmd.opens_field {
                        seen_field_codes.clear();
                    }
                    // ZPL2305: Redundant state-setting detection (check BEFORE recording)
                    if cmd.effects.is_some()
                        && let Some(&consumed) = label_state.producer_consumed.get(producer_key)
                        && !consumed
                    {
                        issues.push(Diagnostic::info(
                            codes::REDUNDANT_STATE,
                            format!(
                                "{} overrides a previous {} without any command consuming the earlier value",
                                code, producer_key
                            ),
                            dspan,
                        ).with_context(ctx!("command" => code, "producer" => producer_key)));
                    }

                    // Record state effects before validation so later commands can reference
                    if cmd.effects.is_some() {
                        label_state.record_producer(producer_key, node_idx);
                        label_state
                            .value_state
                            .apply_producer(code, args, &device_state);
                    }

                    if !cmd.field_data && (args.len() as u32) > cmd.arity {
                        issues.push(
                            Diagnostic::error(
                                codes::ARITY,
                                format!(
                                    "{} has too many arguments ({}>{})",
                                    code,
                                    args.len(),
                                    cmd.arity
                                ),
                                dspan,
                            )
                            .with_context(ctx!(
                                "command" => code,
                                "arity" => cmd.arity.to_string(),
                                "actual" => args.len().to_string(),
                            )),
                        );
                    }

                    // Build context structs for validation functions
                    let cmd_ctx = CommandCtx {
                        code,
                        args,
                        cmd,
                        span: dspan,
                        node_idx,
                    };
                    let vctx = ValidationContext {
                        profile,
                        label_nodes: &label.nodes,
                        label_codes: &label_codes,
                        device_state: &device_state,
                    };

                    validate_command_args(&cmd_ctx, &vctx, &label_state, &mut issues);
                    validate_command_constraints(
                        &cmd_ctx,
                        &vctx,
                        &seen_codes,
                        &seen_field_codes,
                        field_id_by_node[node_idx].and_then(|fid| field_codes.get(fid)),
                        &mut issues,
                    );
                    validate_semantic_state(&cmd_ctx, &vctx, &mut label_state, &mut issues);

                    // ─── Printer gate enforcement ────────────────────────
                    if let Some(gates) = &cmd.printer_gates
                        && let Some(p) = profile
                        && let Some(ref features) = p.features
                    {
                        for gate in gates {
                            if let Some(false) = zpl_toolchain_profile::resolve_gate(features, gate)
                            {
                                issues.push(Diagnostic::error(
                                    codes::PRINTER_GATE,
                                    format!("{} requires '{}' capability not available in profile '{}'",
                                        code, gate, &p.id),
                                    dspan,
                                ).with_context(ctx!(
                                    "command" => code,
                                    "gate" => gate.clone(),
                                    "level" => "command",
                                    "profile" => &p.id,
                                )));
                            }
                        }
                    }

                    // ZPL2205: placement validation for commands inside ^XA/^XZ bounds
                    if inside_format_bounds && !matches!(code.as_str(), "^XA" | "^XZ") && {
                        let allowed_inside =
                            cmd.placement.as_ref().and_then(|p| p.allowed_inside_label);
                        match allowed_inside {
                            Some(flag) => !flag,
                            None => matches!(cmd.plane, Some(Plane::Host | Plane::Device)),
                        }
                    } {
                        let plane = match cmd.plane {
                            Some(Plane::Format) => "format",
                            Some(Plane::Device) => "device",
                            Some(Plane::Host) => "host",
                            Some(Plane::Config) => "config",
                            None => "unknown",
                        };
                        issues.push(
                            Diagnostic::warn(
                                codes::HOST_COMMAND_IN_LABEL,
                                format!("{} should not appear inside a label (^XA/^XZ)", code),
                                dspan,
                            )
                            .with_context(ctx!("command" => code, "plane" => plane)),
                        );
                    }

                    // Enforce explicit outside-label placement restrictions when provided.
                    if !inside_format_bounds
                        && !matches!(code.as_str(), "^XA" | "^XZ")
                        && cmd.placement.as_ref().and_then(|p| p.allowed_outside_label)
                            == Some(false)
                    {
                        let plane = match cmd.plane {
                            Some(Plane::Format) => "format",
                            Some(Plane::Device) => "device",
                            Some(Plane::Host) => "host",
                            Some(Plane::Config) => "config",
                            None => "unknown",
                        };
                        issues.push(
                            Diagnostic::warn(
                                codes::HOST_COMMAND_IN_LABEL,
                                format!("{} should not appear outside a label (^XA/^XZ)", code),
                                dspan,
                            )
                            .with_context(ctx!("command" => code, "plane" => plane)),
                        );
                    }

                    // ─── Structural validation (spec-driven) ─────────────
                    field_tracker.process_command(&cmd_ctx, &vctx, &label_state, &mut issues);

                    // Track session-scoped state in DeviceState
                    // (placed after all validation so device_state isn't mutably
                    // borrowed while ValidationContext holds an immutable ref)
                    if cmd.scope == Some(CommandScope::Session) {
                        if code == "^MU" {
                            device_state.apply_mu(args);
                        }
                        device_state
                            .session_producers
                            .insert(producer_key.to_string());
                    }

                    // Track field-local command order for field-scoped constraints.
                    if cmd.closes_field {
                        seen_field_codes.clear();
                    } else if field_tracker.open || cmd.opens_field {
                        seen_field_codes.insert(code.as_str());
                    }
                }

                // Update seen_codes for ALL commands (not just known ones)
                // so order constraints can reference any command code.
                seen_codes.insert(code.as_str());
                if field_tracker.open {
                    seen_field_codes.insert(code.as_str());
                }
            }
        }

        // Unclosed field at end of label (ZPL2203 / FIELD_NOT_CLOSED)
        if field_tracker.open {
            let dspan = label.nodes.last().and_then(|n| {
                if let crate::grammar::ast::Node::Command { span, .. } = n {
                    Some(*span)
                } else {
                    None
                }
            });
            let mut diag = Diagnostic::warn(
                codes::FIELD_NOT_CLOSED,
                "field opened but never closed with ^FS before end of label".to_string(),
                dspan,
            );
            if let Some(crate::grammar::ast::Node::Command { code, .. }) =
                label.nodes.get(field_tracker.start_idx)
            {
                diag = diag.with_context(ctx!("command" => code));
            }
            issues.push(diag);
        }

        // ─── Preflight validation (post-label) ──────────────────────
        {
            let label_span = label.nodes.first().and_then(|n| {
                if let crate::grammar::ast::Node::Command { span, .. } = n {
                    Some(*span)
                } else {
                    None
                }
            });
            let vctx = ValidationContext {
                profile,
                label_nodes: &label.nodes,
                label_codes: &label_codes,
                device_state: &device_state,
            };
            validate_preflight(&vctx, &label_state, label_span, &mut issues);
        }

        resolved_labels.push(ResolvedLabelState {
            values: label_state.value_state.clone(),
            effective_width: label_state.effective_width,
            effective_height: label_state.effective_height,
        });

        // ZPL2202: Empty label check (no printable content)
        if !has_printable {
            let dspan = label.nodes.first().and_then(|n| {
                if let crate::grammar::ast::Node::Command { span, .. } = n {
                    Some(*span)
                } else {
                    None
                }
            });
            issues.push(Diagnostic::info(
                codes::EMPTY_LABEL,
                "Empty label (no commands between ^XA and ^XZ)",
                dspan,
            ));
        }
    }

    let ok = !issues.iter().any(|d| matches!(d.severity, Severity::Error));
    ValidationResult {
        ok,
        issues,
        resolved_labels,
    }
}

/// Validate a ZPL AST without a printer profile.
pub fn validate(ast: &Ast, tables: &ParserTables) -> ValidationResult {
    validate_with_profile(ast, tables, None)
}
