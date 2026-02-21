use super::context::{CommandCtx, ValidationContext};
use super::ctx;
use super::diagnostics_util::{diagnostic_with_spec_severity, render_diagnostic_message, trim_f64};
use super::predicates::{enum_contains, predicate_matches};
use super::profile_constraints::check_profile_op;
use super::resolve_profile_field;
use super::state::LabelState;
use crate::grammar::diag::{Diagnostic, codes};
use crate::state::{Units, convert_to_dots};
use std::collections::HashMap;
use zpl_toolchain_spec_tables::{ComparisonOp, RoundingMode};

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
                diagnostic_with_spec_severity(
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
            diagnostic_with_spec_severity(
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
            diagnostic_with_spec_severity(
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
    let mut epsilon = rp.as_ref().map(|policy| policy.epsilon).unwrap_or(1e-9);
    if let Some(rpw) = spec_arg.rounding_policy_when.as_ref() {
        for c in rpw {
            if predicate_matches(&c.when, cmd_ctx.args) {
                epsilon = c.epsilon.unwrap_or(epsilon);
                rp = Some(zpl_toolchain_spec_tables::RoundingPolicy {
                    unit: None,
                    mode: c.mode,
                    multiple: c.multiple,
                    epsilon,
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
        if rem > pol.epsilon && (1.0 - rem) > pol.epsilon {
            let value = trim_f64(n);
            let multiple = trim_f64(m);
            let message = render_diagnostic_message(
                codes::ROUNDING_VIOLATION,
                "notMultiple",
                &[
                    ("command", cmd_ctx.code.to_string()),
                    ("arg", lookup_key.to_string()),
                    ("value", value.clone()),
                    ("multiple", multiple.clone()),
                ],
                format!(
                    "{}.{}={} not a multiple of {}",
                    cmd_ctx.code, lookup_key, value, multiple
                ),
            );
            issues.push(
                diagnostic_with_spec_severity(codes::ROUNDING_VIOLATION, message, cmd_ctx.span)
                    .with_context(ctx!(
                        "command" => cmd_ctx.code,
                        "arg" => lookup_key,
                        "value" => val,
                        "multiple" => multiple,
                        "epsilon" => trim_f64(pol.epsilon),
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
            diagnostic_with_spec_severity(
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
                        issues.push(
                            diagnostic_with_spec_severity(
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
                            )
                            .with_context(ctx!(
                                "command" => cmd_ctx.code,
                                "arg" => lookup_key,
                                "value" => val,
                                "gate" => gate.clone(),
                                "level" => "enum",
                                "profile" => &p.id,
                            )),
                        );
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
                        diagnostic_with_spec_severity(
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
                    diagnostic_with_spec_severity(
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
                    diagnostic_with_spec_severity(
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
                    diagnostic_with_spec_severity(
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
pub(super) fn validate_command_args(
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
                        diagnostic_with_spec_severity(
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
                            diagnostic_with_spec_severity(
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
                            diagnostic_with_spec_severity(
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
