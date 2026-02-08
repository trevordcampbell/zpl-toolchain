pub use crate::grammar::diag::Diagnostic;
use crate::grammar::{ast::Ast, diag::Severity, diag::Span, diag::codes, tables::ParserTables};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::{ComparisonOp, ConstraintKind, RoundingMode};

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

/// Parse the first argument as f64, if present and valid.
fn first_arg_f64(args: &[crate::grammar::ast::ArgSlot]) -> Option<f64> {
    args.first()
        .and_then(|slot| slot.value.as_ref())
        .and_then(|v| v.parse::<f64>().ok())
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

// ─── Device-level state tracking ────────────────────────────────────────────

/// Unit system for measurement conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Units {
    #[default]
    Dots,
    Inches,
    Millimeters,
}

/// Device/session-scoped state that persists across labels.
///
/// Created once before the label loop and updated by session-scoped commands
/// (e.g., `^MU`, `^MD`, `^PR`).
#[derive(Debug, Default)]
struct DeviceState {
    /// Session-scoped producer tracking (persists across labels).
    session_producers: HashSet<String>,
    /// Active unit system from ^MU (default: dots).
    units: Units,
    /// DPI for unit conversion (from profile or ^MU).
    dpi: Option<u32>,
}

/// Convert a value from the active unit system to dots.
fn convert_to_dots(value: f64, units: Units, dpi: u32) -> f64 {
    match units {
        Units::Dots => value,
        Units::Inches => value * dpi as f64,
        Units::Millimeters => value * dpi as f64 / 25.4,
    }
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
    /// Active barcode command and its field data rules.
    /// Set when a barcode command (^B*) is seen, used at ^FS to validate ^FD content.
    active_barcode: Option<(String, zpl_toolchain_spec_tables::FieldDataRules)>,
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
            active_barcode: None,
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
        self.active_barcode = None;
    }

    /// Process a command's structural flags and emit diagnostics.
    fn process_command(
        &mut self,
        cmd_ctx: &CommandCtx,
        vctx: &ValidationContext,
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
            self.validate_field_close(cmd_ctx, vctx, issues);
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
                || rules.min_length.is_some()
                || rules.max_length.is_some()
                || rules.length_parity.is_some())
        {
            self.active_barcode = Some((cmd_ctx.code.to_string(), rules.clone()));
        }
    }

    /// Validate when a field-closing command (^FS) is encountered.
    fn validate_field_close(
        &mut self,
        cmd_ctx: &CommandCtx,
        vctx: &ValidationContext,
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
                if let crate::grammar::ast::Node::FieldData {
                    content,
                    span: fd_span,
                    ..
                } = field_node
                {
                    let fd_dspan = Some(*fd_span);
                    for err in crate::hex_escape::validate_hex_escapes(content, indicator) {
                        issues.push(Diagnostic::error(
                            codes::INVALID_HEX_ESCAPE,
                            err.message,
                            fd_dspan,
                        ).with_context(ctx!("command" => "^FH", "indicator" => String::from(indicator as char))));
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
        // Field data can live in two places:
        // 1. Inline: the first arg (key="data") of a ^FD/^FV command node
        // 2. Multi-line: a FieldData node following the ^FD/^FV command
        if !self.has_fh
            && let Some((barcode_code, rules)) = &self.active_barcode
        {
            for field_node in &vctx.label_nodes[self.start_idx..cmd_ctx.node_idx] {
                match field_node {
                    crate::grammar::ast::Node::Command { code, args, span }
                        if code == "^FD" || code == "^FV" =>
                    {
                        if let Some(slot) = args.first()
                            && let Some(val) = slot.value.as_deref()
                            && !val.is_empty()
                        {
                            validate_barcode_field_data(
                                barcode_code,
                                val,
                                rules,
                                Some(*span),
                                issues,
                            );
                        }
                    }
                    crate::grammar::ast::Node::FieldData {
                        content,
                        span: fd_span,
                        ..
                    } => {
                        validate_barcode_field_data(
                            barcode_code,
                            content,
                            rules,
                            Some(*fd_span),
                            issues,
                        );
                    }
                    _ => {}
                }
            }
        }

        self.open = false;
        self.reset();
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
    let len = fd_content.len();

    // exactLength takes precedence
    if let Some(exact) = rules.exact_length {
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
        && !check_profile_op(n, &pc.op, limit)
    {
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
                "actual" => val,
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

/// Validate command arguments: presence, enum, type, range, length, rounding, profile.
fn validate_command_args(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    let spec_args = match cmd_ctx.cmd.args.as_ref() {
        Some(sa) => sa,
        None => return,
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

        // Presence checks (skip for args with defaultFrom when producer seen)
        if let Some(arg) = eff
            && !arg.optional
        {
            let has_state_default = arg
                .default_from
                .as_ref()
                .is_some_and(|df| label_state.has_producer(df));
            let has_static_default = arg.default.is_some()
                || arg.default_by_dpi.as_ref().is_some_and(|m| {
                    vctx.profile
                        .map(|p| p.dpi)
                        .is_some_and(|d| m.contains_key(&d.to_string()))
                });
            let has_any_default = has_state_default || has_static_default;

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
        }
    }
}

/// Validate command constraints: order, requires, incompatible, empty data, notes.
fn validate_command_constraints(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    seen_codes: &HashSet<&str>,
    issues: &mut Vec<Diagnostic>,
) {
    let constraints = match cmd_ctx.cmd.constraints.as_ref() {
        Some(c) => c,
        None => return,
    };

    for c in constraints {
        match c.kind {
            ConstraintKind::Order => {
                if let Some(expr) = c.expr.as_ref() {
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
                            )),
                        );
                    }
                }
            }
            ConstraintKind::Requires => {
                if let Some(expr) = c.expr.as_ref()
                    && !any_target_in_set(expr, vctx.label_codes)
                {
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
                        )),
                    );
                }
            }
            ConstraintKind::Incompatible => {
                if let Some(expr) = c.expr.as_ref()
                    && any_target_in_set(expr, vctx.label_codes)
                {
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
                        )),
                    );
                }
            }
            ConstraintKind::EmptyData => {
                // Check both the command's args AND any following FieldData node
                let fd_has_content = cmd_ctx
                    .args
                    .first()
                    .and_then(|a| a.value.as_ref())
                    .is_some_and(|s| !s.is_empty());
                let next_is_field_data = vctx.label_nodes.get(cmd_ctx.node_idx + 1)
                    .is_some_and(|n| matches!(n, crate::grammar::ast::Node::FieldData { content, .. } if !content.is_empty()));
                if !fd_has_content && !next_is_field_data {
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
    // Track ^PW and ^LL values for position bounds checking.
    // Only store finite positive values — NaN/infinity would corrupt bounds.
    if cmd_ctx.code == "^PW"
        && let Some(w) = first_arg_f64(cmd_ctx.args)
        && w.is_finite()
        && w > 0.0
    {
        label_state.effective_width = Some(w);
    }
    if cmd_ctx.code == "^LL"
        && let Some(h) = first_arg_f64(cmd_ctx.args)
        && h.is_finite()
        && h > 0.0
    {
        label_state.effective_height = Some(h);
        // Profile height check is handled by the generic profileConstraint
        // mechanism in validate_command_args() — no hardcoded check here.
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

    if let Some(x_slot) = cmd_ctx.args.first()
        && let Some(x_val) = x_slot.value.as_ref()
        && let (Ok(x), Some(w)) = (x_val.parse::<f64>(), max_x)
        && x > w
    {
        issues.push(
            Diagnostic::warn(
                codes::POSITION_OUT_OF_BOUNDS,
                format!(
                    "{} x position {} exceeds label width {}",
                    cmd_ctx.code,
                    x_val,
                    trim_f64(w)
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "axis" => "x",
                "value" => x_val.clone(),
                "limit" => trim_f64(w),
            )),
        );
    }
    if let Some(y_slot) = cmd_ctx.args.get(1)
        && let Some(y_val) = y_slot.value.as_ref()
        && let (Ok(y), Some(h)) = (y_val.parse::<f64>(), max_y)
        && y > h
    {
        issues.push(
            Diagnostic::warn(
                codes::POSITION_OUT_OF_BOUNDS,
                format!(
                    "{} y position {} exceeds label height {}",
                    cmd_ctx.code,
                    y_val,
                    trim_f64(h)
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "axis" => "y",
                "value" => y_val.clone(),
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

        for (node_idx, node) in label.nodes.iter().enumerate() {
            if let crate::grammar::ast::Node::Command { code, args, span } = node {
                let dspan = Some(*span);

                // Track printable content for empty-label detection (ZPL2202)
                if !matches!(code.as_str(), "^XA" | "^XZ") {
                    has_printable = true;
                }

                // ─── Command validation (known commands only) ────────────
                if known.contains(code)
                    && let Some(cmd) = tables.cmd_by_code(code)
                {
                    // ZPL2305: Redundant state-setting detection (check BEFORE recording)
                    if cmd.effects.is_some()
                        && let Some(&consumed) = label_state.producer_consumed.get(code.as_str())
                        && !consumed
                    {
                        issues.push(Diagnostic::info(
                            codes::REDUNDANT_STATE,
                            format!("{} overrides a previous {} without any command consuming the earlier value", code, code),
                            dspan,
                        ).with_context(ctx!("command" => code)));
                    }

                    // Record state effects before validation so later commands can reference
                    if cmd.effects.is_some() {
                        label_state.record_producer(code, node_idx);
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
                    validate_command_constraints(&cmd_ctx, &vctx, &seen_codes, &mut issues);
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

                    // ZPL2205: Scope validation — host/device plane commands inside labels
                    if !matches!(code.as_str(), "^XA" | "^XZ")
                        && let Some(plane) = &cmd.plane
                        && matches!(
                            plane,
                            zpl_toolchain_spec_tables::Plane::Host
                                | zpl_toolchain_spec_tables::Plane::Device
                        )
                    {
                        issues.push(Diagnostic::warn(
                            codes::HOST_COMMAND_IN_LABEL,
                            format!("{} is a {} command and should not appear inside a label (^XA/^XZ)", code, plane),
                            dspan,
                        ).with_context(ctx!("command" => code, "plane" => format!("{}", plane))));
                    }

                    // ─── Structural validation (spec-driven) ─────────────
                    field_tracker.process_command(&cmd_ctx, &vctx, &mut issues);

                    // Track session-scoped state in DeviceState
                    // (placed after all validation so device_state isn't mutably
                    // borrowed while ValidationContext holds an immutable ref)
                    if cmd.scope == Some(zpl_toolchain_spec_tables::CommandScope::Session) {
                        // ^MU: update unit system
                        if code == "^MU"
                            && let Some(unit_arg) = args.first().and_then(|a| a.value.as_deref())
                        {
                            device_state.units = match unit_arg {
                                "I" => Units::Inches,
                                "M" => Units::Millimeters,
                                _ => Units::Dots,
                            };
                        }
                        device_state.session_producers.insert(code.to_string());
                    }
                }

                // Update seen_codes for ALL commands (not just known ones)
                // so order constraints can reference any command code.
                seen_codes.insert(code.as_str());
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
            issues.push(Diagnostic::warn(
                codes::FIELD_NOT_CLOSED,
                "field opened but never closed with ^FS before end of label".to_string(),
                dspan,
            ));
        }

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
    ValidationResult { ok, issues }
}

/// Validate a ZPL AST without a printer profile.
pub fn validate(ast: &Ast, tables: &ParserTables) -> ValidationResult {
    validate_with_profile(ast, tables, None)
}
