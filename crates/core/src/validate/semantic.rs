use super::context::{CommandCtx, ValidationContext};
use super::ctx;
use super::diagnostics_util::{diagnostic_with_spec_severity, trim_f64};
use super::resolve_profile_field;
use super::state::LabelState;
use crate::grammar::diag::{Diagnostic, codes};
use crate::state::{Units, convert_to_dots};
use zpl_toolchain_spec_tables::ArgUnion;
use zpl_toolchain_spec_tables::{
    FontReferenceAction, MediaModesTarget, PositionBoundsAction, StructuralRule,
};

/// ZPL2301: Duplicate ^FN field number detection.
fn validate_field_number(
    cmd_ctx: &CommandCtx,
    arg_index: usize,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(slot) = cmd_ctx.args.get(arg_index)
        && let Some(n) = slot.value.as_ref()
    {
        if let Some(&first_idx) = label_state.field_numbers.get(n) {
            issues.push(
                diagnostic_with_spec_severity(
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
    action: PositionBoundsAction,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    match action {
        PositionBoundsAction::TrackWidth => {
            if let Some(w) = label_state.value_state.layout.print_width {
                label_state.effective_width = Some(w);
            }
            label_state.has_explicit_pw = true;
            return;
        }
        PositionBoundsAction::TrackHeight => {
            if let Some(h) = label_state.value_state.layout.label_length {
                label_state.effective_height = Some(h);
            }
            label_state.has_explicit_ll = true;
            return;
        }
        PositionBoundsAction::TrackFieldOrigin => {
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
            return;
        }
        PositionBoundsAction::ValidateFieldOrigin => {}
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
            diagnostic_with_spec_severity(
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
            diagnostic_with_spec_severity(
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
    action: FontReferenceAction,
    arg_index: usize,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(slot) = cmd_ctx.args.get(arg_index)
        && let Some(v) = slot.value.as_ref()
        && let Some(ch) = v.chars().next()
    {
        match action {
            FontReferenceAction::Register => {
                label_state.loaded_fonts.insert(ch);
            }
            FontReferenceAction::Validate => {
                let is_builtin = ch.is_ascii_uppercase() || ch.is_ascii_digit();
                let is_loaded = label_state.loaded_fonts.contains(&ch);
                if !is_builtin && !is_loaded {
                    issues.push(
                        diagnostic_with_spec_severity(
                            codes::UNKNOWN_FONT,
                            format!(
                                "{} font '{}' is not a built-in font (A-Z, 0-9) and has not been loaded via ^CW",
                                cmd_ctx.code, ch
                            ),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!("command" => cmd_ctx.code, "font" => ch.to_string())),
                    );
                }
            }
        }
    }
}

/// ZPL1403: Media mode validation (^MM, ^MN, ^MT).
fn validate_media_modes(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    target: MediaModesTarget,
    arg_index: usize,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(slot) = cmd_ctx.args.get(arg_index)
        && let Some(val) = slot.value.as_ref()
        && let Some(p) = vctx.profile
        && let Some(ref media) = p.media
    {
        match target {
            MediaModesTarget::SupportedModes => {
                if let Some(ref modes) = media.supported_modes
                    && !modes.is_empty()
                    && !modes.iter().any(|m| m == val)
                {
                    issues.push(
                        diagnostic_with_spec_severity(
                            codes::MEDIA_MODE_UNSUPPORTED,
                            format!(
                                "{} mode '{}' is not in profile's supported_modes {:?}",
                                cmd_ctx.code, val, modes
                            ),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!(
                            "command" => cmd_ctx.code,
                            "kind" => "mode",
                            "value" => val.clone(),
                            "supported" => format!("{:?}", modes),
                            "profile" => &p.id,
                        )),
                    );
                }
            }
            MediaModesTarget::SupportedTracking => {
                if let Some(ref tracking) = media.supported_tracking
                    && !tracking.is_empty()
                    && !tracking.iter().any(|t| t == val)
                {
                    issues.push(
                        diagnostic_with_spec_severity(
                            codes::MEDIA_MODE_UNSUPPORTED,
                            format!(
                                "{} tracking mode '{}' is not in profile's supported_tracking {:?}",
                                cmd_ctx.code, val, tracking
                            ),
                            cmd_ctx.span,
                        )
                        .with_context(ctx!(
                            "command" => cmd_ctx.code,
                            "kind" => "tracking",
                            "value" => val.clone(),
                            "supported" => format!("{:?}", tracking),
                            "profile" => &p.id,
                        )),
                    );
                }
            }
            MediaModesTarget::PrintMethod => {
                if let Some(ref method) = media.print_method {
                    let compatible = match method {
                        zpl_toolchain_profile::PrintMethod::Both => true,
                        zpl_toolchain_profile::PrintMethod::DirectThermal => val == "D",
                        zpl_toolchain_profile::PrintMethod::ThermalTransfer => val == "T",
                    };
                    if !compatible {
                        issues.push(
                            diagnostic_with_spec_severity(
                                codes::MEDIA_MODE_UNSUPPORTED,
                                format!(
                                    "{} media type '{}' conflicts with profile print method '{:?}'",
                                    cmd_ctx.code, val, method
                                ),
                                cmd_ctx.span,
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "kind" => "method",
                                "value" => val.clone(),
                                "profile_method" => format!("{:?}", method),
                                "profile" => &p.id,
                            )),
                        );
                    }
                }
            }
        }
    }
}

/// ZPL2307: ^GF data length validation.
fn validate_gf_data_length(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    compression_arg_index: usize,
    declared_byte_count_arg_index: usize,
    data_arg_index: usize,
    issues: &mut Vec<Diagnostic>,
) {
    if cmd_ctx.args.len() <= data_arg_index {
        return;
    }

    let compression = cmd_ctx.args[compression_arg_index]
        .value
        .as_deref()
        .unwrap_or("A");
    let byte_count_val = cmd_ctx.args[declared_byte_count_arg_index].value.as_deref();
    let data_val = cmd_ctx.args[data_arg_index].value.as_deref();

    if let (Some(bc_str), Some(data)) = (byte_count_val, data_val)
        && let Ok(declared) = bc_str.parse::<usize>()
    {
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
                if command == cmd_ctx.code {
                    total_data_len += raw_data.as_deref().map_or(0, &effective_len);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let mismatch = match compression {
            "A" => {
                let expected = declared * 2;
                if total_data_len != expected {
                    Some((total_data_len, expected, "ASCII hex (2 chars per byte)"))
                } else {
                    None
                }
            }
            "B" => {
                if total_data_len != declared {
                    Some((total_data_len, declared, "binary (1:1)"))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some((actual_len, expected_len, fmt)) = mismatch {
            issues.push(
                diagnostic_with_spec_severity(
                    codes::GF_DATA_LENGTH_MISMATCH,
                    format!(
                        "^GF data length mismatch: declared {} bytes ({}), but data is {} chars (expected {})",
                        declared, fmt, actual_len, expected_len
                    ),
                    cmd_ctx.span,
                )
                .with_context(ctx!(
                    "command" => cmd_ctx.code,
                    "format" => compression,
                    "declared" => declared.to_string(),
                    "actual" => actual_len.to_string(),
                    "expected" => expected_len.to_string(),
                )),
            );
        }
    }
}

/// ZPL2308: ^GF graphic bounds check + ZPL2309: accumulate graphic bytes.
fn validate_gf_preflight_tracking(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    graphic_field_count_arg_index: usize,
    bytes_per_row_arg_index: usize,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    if cmd_ctx.args.len() <= bytes_per_row_arg_index {
        return;
    }

    let gfc_val = cmd_ctx
        .args
        .get(graphic_field_count_arg_index)
        .and_then(|s| s.value.as_deref());
    let bpr_val = cmd_ctx
        .args
        .get(bytes_per_row_arg_index)
        .and_then(|s| s.value.as_deref());

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
                        diagnostic_with_spec_severity(
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

pub(super) fn consume_default_from_refs(cmd_ctx: &CommandCtx, label_state: &mut LabelState) {
    // Mark consumed producers via defaultFrom references.
    if let Some(spec_args) = cmd_ctx.cmd.args.as_ref() {
        for sa in spec_args {
            let arg = match sa {
                ArgUnion::Single(a) => Some(a.as_ref()),
                ArgUnion::OneOf { one_of } => one_of.first(),
            };
            if let Some(a) = arg
                && let Some(df) = &a.default_from
            {
                label_state.mark_consumed(df);
            }
        }
    }
}

fn run_semantic_rule(
    rule: &StructuralRule,
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    match rule {
        StructuralRule::DuplicateFieldNumber { arg_index } => {
            validate_field_number(cmd_ctx, *arg_index, label_state, issues);
        }
        StructuralRule::PositionBounds { action } => {
            validate_position_bounds(cmd_ctx, vctx, *action, label_state, issues);
        }
        StructuralRule::FontReference { action, arg_index } => {
            validate_font_reference(cmd_ctx, *action, *arg_index, label_state, issues);
        }
        StructuralRule::MediaModes { target, arg_index } => {
            validate_media_modes(cmd_ctx, vctx, *target, *arg_index, issues);
        }
        StructuralRule::GfDataLength {
            compression_arg_index,
            declared_byte_count_arg_index,
            data_arg_index,
        } => validate_gf_data_length(
            cmd_ctx,
            vctx,
            *compression_arg_index,
            *declared_byte_count_arg_index,
            *data_arg_index,
            issues,
        ),
        StructuralRule::GfPreflightTracking {
            graphic_field_count_arg_index,
            bytes_per_row_arg_index,
        } => validate_gf_preflight_tracking(
            cmd_ctx,
            vctx,
            *graphic_field_count_arg_index,
            *bytes_per_row_arg_index,
            label_state,
            issues,
        ),
    }
}

/// Validate structural semantic rules declared on this command.
pub(super) fn validate_structural_semantics(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    let Some(rules) = cmd_ctx.cmd.structural_rules.as_ref() else {
        return;
    };
    for rule in rules {
        run_semantic_rule(rule, cmd_ctx, vctx, label_state, issues);
    }
}
