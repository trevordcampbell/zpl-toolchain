use super::context::{CommandCtx, ValidationContext};
use super::diagnostics_util::{
    Diagnostic, diagnostic_with_constraint_severity, diagnostic_with_spec_severity,
    render_diagnostic_message, trim_f64,
};
use super::plan::StructuralFlags;
use super::state::LabelState;
use super::{ctx, resolve_profile_field};
use crate::grammar::diag::{Severity, Span, codes};
use zpl_toolchain_diagnostics::policy::{
    OBJECT_BOUNDS_LOW_CONFIDENCE_MAX_OVERFLOW_DOTS,
    OBJECT_BOUNDS_LOW_CONFIDENCE_MAX_OVERFLOW_RATIO, OBJECT_BOUNDS_LOW_CONFIDENCE_SEVERITY,
};

/// Tracks field-level structural state within a label.
/// Reset when a field-opening command is encountered.
pub(super) struct FieldTracker {
    /// Whether a field is currently open (between ^FO/^FT and ^FS).
    pub(super) open: bool,
    /// Whether ^FH was seen in the current field.
    has_fh: bool,
    /// The hex escape indicator character (default `_`, configurable via ^FH arg).
    fh_indicator: u8,
    /// Whether ^FN was seen in the current field.
    has_fn: bool,
    /// Whether ^SN/^SF was seen in the current field.
    has_serial: bool,
    /// Node index of the field-opening command.
    pub(super) start_idx: usize,
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
    pub(super) fn process_command(
        &mut self,
        cmd_ctx: &CommandCtx,
        vctx: &ValidationContext,
        label_state: &LabelState,
        structural_flags: StructuralFlags,
        issues: &mut Vec<Diagnostic>,
    ) {
        if structural_flags.opens_field {
            if self.open {
                issues.push(
                    diagnostic_with_spec_severity(
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

        if structural_flags.closes_field {
            self.validate_field_close(cmd_ctx, vctx, label_state, issues);
        }

        if (structural_flags.field_data || structural_flags.requires_field) && !self.open {
            issues.push(
                diagnostic_with_spec_severity(
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

        if structural_flags.hex_escape_modifier {
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
        if structural_flags.field_number {
            self.has_fn = true;
        }
        if structural_flags.serialization {
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
                diagnostic_with_spec_severity(
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
                            diagnostic_with_spec_severity(
                                codes::INVALID_HEX_ESCAPE,
                                err.message,
                                dspan,
                            )
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
                diagnostic_with_spec_severity(
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
        let overflow_x = if overflows_x {
            (fo_x + est_width - max_x).max(0.0)
        } else {
            0.0
        };
        let overflow_y = if overflows_y {
            (fo_y + est_height - max_y).max(0.0)
        } else {
            0.0
        };
        let overflow_x_ratio = if max_x > 0.0 { overflow_x / max_x } else { 0.0 };
        let overflow_y_ratio = if max_y > 0.0 { overflow_y / max_y } else { 0.0 };
        let max_overflow_dots = overflow_x.max(overflow_y);
        let max_overflow_ratio = overflow_x_ratio.max(overflow_y_ratio);
        let low_confidence = max_overflow_dots <= OBJECT_BOUNDS_LOW_CONFIDENCE_MAX_OVERFLOW_DOTS
            && max_overflow_ratio <= OBJECT_BOUNDS_LOW_CONFIDENCE_MAX_OVERFLOW_RATIO;
        let severity = if low_confidence {
            OBJECT_BOUNDS_LOW_CONFIDENCE_SEVERITY
        } else {
            Severity::Warn
        };
        let confidence = if low_confidence { "low" } else { "high" };
        let x = trim_f64(fo_x);
        let y = trim_f64(fo_y);
        let label_width = trim_f64(max_x);
        let label_height = trim_f64(max_y);
        let message = if low_confidence {
            render_diagnostic_message(
                codes::OBJECT_BOUNDS_OVERFLOW,
                "lowConfidence",
                &[
                    ("object_type", object_type.to_string()),
                    ("x", x.clone()),
                    ("y", y.clone()),
                    ("label_width", label_width.clone()),
                    ("label_height", label_height.clone()),
                ],
                format!(
                    "{object_type} at ({x}, {y}) may extend beyond label bounds ({label_width}×{label_height} dots, estimated)"
                ),
            )
        } else {
            render_diagnostic_message(
                codes::OBJECT_BOUNDS_OVERFLOW,
                "highConfidence",
                &[
                    ("object_type", object_type.to_string()),
                    ("x", x.clone()),
                    ("y", y.clone()),
                    ("label_width", label_width.clone()),
                    ("label_height", label_height.clone()),
                ],
                format!(
                    "{object_type} at ({x}, {y}) extends beyond label bounds ({label_width}×{label_height} dots)"
                ),
            )
        };
        issues.push(
            Diagnostic::new(
                codes::OBJECT_BOUNDS_OVERFLOW,
                severity,
                message,
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "object_type" => object_type,
                "x" => x,
                "y" => y,
                "estimated_width" => trim_f64(est_width),
                "estimated_height" => trim_f64(est_height),
                "label_width" => label_width,
                "label_height" => label_height,
                "overflow_x" => trim_f64(overflow_x),
                "overflow_y" => trim_f64(overflow_y),
                "overflow_x_ratio" => trim_f64(overflow_x_ratio),
                "overflow_y_ratio" => trim_f64(overflow_y_ratio),
                "confidence" => confidence,
                "audience" => "problem",
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
    let charset_severity = rules
        .character_set_severity
        .unwrap_or(zpl_toolchain_spec_tables::ConstraintSeverity::Error);
    let length_severity = rules
        .length_severity
        .unwrap_or(zpl_toolchain_spec_tables::ConstraintSeverity::Warn);

    // Character set validation
    if let Some(charset) = &rules.character_set {
        for (i, ch) in fd_content.chars().enumerate() {
            if !char_in_set(ch, charset) {
                let position = i.to_string();
                let message = render_diagnostic_message(
                    codes::BARCODE_INVALID_CHAR,
                    "invalidChar",
                    &[
                        ("character", ch.to_string()),
                        ("position", position.clone()),
                        ("command", barcode_code.to_string()),
                        ("allowedSet", charset.clone()),
                    ],
                    format!(
                        "invalid character '{}' at position {} in {} field data (allowed: [{}])",
                        ch, i, barcode_code, charset
                    ),
                );
                issues.push(
                    diagnostic_with_constraint_severity(
                        codes::BARCODE_INVALID_CHAR,
                        charset_severity,
                        message,
                        dspan,
                    )
                    .with_context(ctx!(
                        "command" => barcode_code,
                        "character" => ch.to_string(),
                        "position" => position,
                        "allowedSet" => charset.clone(),
                    )),
                );
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
            let actual = len.to_string();
            let message = render_diagnostic_message(
                codes::BARCODE_DATA_LENGTH,
                "allowedLengths",
                &[
                    ("command", barcode_code.to_string()),
                    ("actual", actual.clone()),
                    ("expected", expected.clone()),
                ],
                format!(
                    "{} field data length {} (expected one of [{}])",
                    barcode_code, len, expected
                ),
            );
            issues.push(
                diagnostic_with_constraint_severity(
                    codes::BARCODE_DATA_LENGTH,
                    length_severity,
                    message,
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => actual,
                    "expected" => expected,
                )),
            );
        }
    // exactLength takes precedence over min/max.
    } else if let Some(exact) = rules.exact_length {
        if len != exact {
            let actual = len.to_string();
            let expected = exact.to_string();
            let message = render_diagnostic_message(
                codes::BARCODE_DATA_LENGTH,
                "exactLength",
                &[
                    ("command", barcode_code.to_string()),
                    ("actual", actual.clone()),
                    ("expected", expected.clone()),
                ],
                format!(
                    "{} field data length {} (expected exactly {})",
                    barcode_code, len, exact
                ),
            );
            issues.push(
                diagnostic_with_constraint_severity(
                    codes::BARCODE_DATA_LENGTH,
                    length_severity,
                    message,
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => actual,
                    "expected" => expected,
                )),
            );
        }
    } else {
        if let Some(min) = rules.min_length
            && len < min
        {
            let actual = len.to_string();
            let min = min.to_string();
            let message = render_diagnostic_message(
                codes::BARCODE_DATA_LENGTH,
                "minLength",
                &[
                    ("command", barcode_code.to_string()),
                    ("actual", actual.clone()),
                    ("min", min.clone()),
                ],
                format!(
                    "{} field data too short: {} chars (minimum {})",
                    barcode_code, actual, min
                ),
            );
            issues.push(
                diagnostic_with_constraint_severity(
                    codes::BARCODE_DATA_LENGTH,
                    length_severity,
                    message,
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => actual,
                    "min" => min,
                )),
            );
        }
        if let Some(max) = rules.max_length
            && len > max
        {
            let actual = len.to_string();
            let max = max.to_string();
            let message = render_diagnostic_message(
                codes::BARCODE_DATA_LENGTH,
                "maxLength",
                &[
                    ("command", barcode_code.to_string()),
                    ("actual", actual.clone()),
                    ("max", max.clone()),
                ],
                format!(
                    "{} field data too long: {} chars (maximum {})",
                    barcode_code, actual, max
                ),
            );
            issues.push(
                diagnostic_with_constraint_severity(
                    codes::BARCODE_DATA_LENGTH,
                    length_severity,
                    message,
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => actual,
                    "max" => max,
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
            let actual = len.to_string();
            let actual_parity = if even { "even" } else { "odd" }.to_string();
            let message = render_diagnostic_message(
                codes::BARCODE_DATA_LENGTH,
                "parity",
                &[
                    ("command", barcode_code.to_string()),
                    ("actual", actual.clone()),
                    ("parity", parity.clone()),
                    ("actualParity", actual_parity.clone()),
                ],
                format!(
                    "{} field data length {} should be {} (got {})",
                    barcode_code, len, parity, actual_parity
                ),
            );
            issues.push(
                diagnostic_with_constraint_severity(
                    codes::BARCODE_DATA_LENGTH,
                    length_severity,
                    message,
                    dspan,
                )
                .with_context(ctx!(
                    "command" => barcode_code,
                    "actual" => actual,
                    "parity" => parity.clone(),
                    "actualParity" => actual_parity,
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
