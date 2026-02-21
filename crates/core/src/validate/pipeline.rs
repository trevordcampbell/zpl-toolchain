use super::args::validate_command_args;
use super::constraints::validate_command_constraints;
use super::context::{CommandCtx, ValidationContext};
use super::diagnostics_util::diagnostic_with_spec_severity;
use super::field::FieldTracker;
use super::plan::{LabelExecutionPlan, StructuralFlags, ValidationPlanContext};
use super::preflight::validate_preflight;
use super::semantic::{consume_default_from_refs, validate_structural_semantics};
use super::state::LabelState;
use super::{Diagnostic, ctx};
use crate::grammar::ast::{ArgSlot, Label, Node};
use crate::grammar::diag::codes;
use crate::grammar::tables::ParserTables;
use crate::state::{DeviceState, ResolvedLabelState};
use std::collections::HashSet;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::{CommandEntry, CommandScope, Plane};

struct FieldMembership<'a> {
    field_id_by_node: Vec<Option<usize>>,
    field_codes: Vec<HashSet<&'a str>>,
}

#[derive(Clone, Copy)]
struct PlanningContext<'a> {
    plan_ctx: &'a ValidationPlanContext,
    plan: &'a LabelExecutionPlan,
}

struct ConstraintSets<'a> {
    seen_codes: &'a HashSet<&'a str>,
    seen_field_codes: &'a HashSet<&'a str>,
    field_scope_codes: Option<&'a HashSet<&'a str>>,
}

struct KnownCommandEnv<'a> {
    label: &'a Label,
    profile: Option<&'a Profile>,
    label_codes: &'a HashSet<&'a str>,
    field_membership: &'a FieldMembership<'a>,
    inside_format_bounds: bool,
    planning: PlanningContext<'a>,
}

struct LabelCommandEnv<'a> {
    label: &'a Label,
    tables: &'a ParserTables,
    known: &'a HashSet<String>,
    profile: Option<&'a Profile>,
    label_codes: &'a HashSet<&'a str>,
    field_membership: &'a FieldMembership<'a>,
    planning: PlanningContext<'a>,
}

struct LabelCommandState<'a> {
    label_state: &'a mut LabelState,
    field_tracker: &'a mut FieldTracker,
    device_state: &'a mut DeviceState,
    issues: &'a mut Vec<Diagnostic>,
}

struct CommandNode<'a> {
    node_idx: usize,
    code: &'a str,
    args: &'a [ArgSlot],
    span: zpl_toolchain_diagnostics::Span,
    cmd: &'a CommandEntry,
}

pub(super) fn validate_label(
    label: &Label,
    tables: &ParserTables,
    known: &HashSet<String>,
    plan_ctx: &ValidationPlanContext,
    profile: Option<&Profile>,
    device_state: &mut DeviceState,
    issues: &mut Vec<Diagnostic>,
) -> ResolvedLabelState {
    let label_codes = collect_label_codes(label);
    let plan = plan_ctx.plan_for_label(&label_codes, profile);
    let field_membership = build_field_membership(label, tables, known, plan_ctx);

    let mut label_state = LabelState::default();
    let mut field_tracker = FieldTracker::default();
    let command_env = LabelCommandEnv {
        label,
        tables,
        known,
        profile,
        label_codes: &label_codes,
        field_membership: &field_membership,
        planning: PlanningContext {
            plan_ctx,
            plan: &plan,
        },
    };
    let mut command_state = LabelCommandState {
        label_state: &mut label_state,
        field_tracker: &mut field_tracker,
        device_state,
        issues,
    };
    let has_printable = process_label_commands(&command_env, &mut command_state);

    emit_unclosed_field_diagnostic(label, &field_tracker, issues);
    run_label_preflight(
        label,
        profile,
        device_state,
        &label_codes,
        &plan,
        &label_state,
        issues,
    );
    emit_empty_label_diagnostic(label, has_printable, issues);

    ResolvedLabelState {
        values: label_state.value_state.clone(),
        // Keep effective dimensions populated for downstream consumers even if
        // semantic rule indexing is sparse; typed producer state remains canonical.
        effective_width: label_state
            .effective_width
            .or(label_state.value_state.layout.print_width),
        effective_height: label_state
            .effective_height
            .or(label_state.value_state.layout.label_length),
    }
}

fn collect_label_codes(label: &Label) -> HashSet<&str> {
    label
        .nodes
        .iter()
        .filter_map(|n| {
            if let Node::Command { code, .. } = n {
                Some(code.as_str())
            } else {
                None
            }
        })
        .collect()
}

fn build_field_membership<'a>(
    label: &'a Label,
    tables: &ParserTables,
    known: &HashSet<String>,
    plan_ctx: &ValidationPlanContext,
) -> FieldMembership<'a> {
    let mut field_id_by_node: Vec<Option<usize>> = vec![None; label.nodes.len()];
    let mut field_codes: Vec<HashSet<&str>> = Vec::new();
    let mut current_field_id: Option<usize> = None;

    for (idx, node) in label.nodes.iter().enumerate() {
        if let Node::Command { code, .. } = node {
            if known.contains(code)
                && let Some(cmd) = tables.cmd_by_code(code)
            {
                let structural_flags = plan_ctx.resolve_structural_flags(code, cmd);
                if structural_flags.opens_field {
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
                if structural_flags.closes_field {
                    current_field_id = None;
                }
            } else if let Some(fid) = current_field_id {
                // Unknown commands still belong to current field membership set for
                // field-scoped constraint evaluation.
                field_id_by_node[idx] = Some(fid);
                if let Some(set) = field_codes.get_mut(fid) {
                    set.insert(code.as_str());
                }
            }
        }
    }

    FieldMembership {
        field_id_by_node,
        field_codes,
    }
}

fn process_label_commands<'a>(
    env: &LabelCommandEnv<'a>,
    state: &mut LabelCommandState<'a>,
) -> bool {
    let mut has_printable = false;
    // Incrementally built set of command codes seen so far in this label,
    // used for O(1) order constraint checks instead of O(n) slice scans.
    let mut seen_codes: HashSet<&str> = HashSet::new();
    // Field-local command set used by field-scoped order constraints.
    let mut seen_field_codes: HashSet<&str> = HashSet::new();

    // Tracks whether the current node position is between ^XA and ^XZ.
    // Parser labels may include pre-^XA commands, which should not be
    // treated as "inside label" for scope diagnostics like ZPL2205.
    let mut inside_format_bounds = false;

    for (node_idx, node) in env.label.nodes.iter().enumerate() {
        if let Node::Command { code, args, span } = node {
            if code == "^XA" {
                inside_format_bounds = true;
            } else if code == "^XZ" {
                inside_format_bounds = false;
            }

            // Track printable content for empty-label detection (ZPL2202)
            if !matches!(code.as_str(), "^XA" | "^XZ") {
                has_printable = true;
            }

            if env.known.contains(code)
                && let Some(cmd) = env.tables.cmd_by_code(code)
            {
                let command = CommandNode {
                    node_idx,
                    code,
                    args,
                    span: *span,
                    cmd,
                };
                process_known_command(
                    &command,
                    &KnownCommandEnv {
                        label: env.label,
                        profile: env.profile,
                        label_codes: env.label_codes,
                        field_membership: env.field_membership,
                        inside_format_bounds,
                        planning: env.planning,
                    },
                    &seen_codes,
                    state,
                    &mut seen_field_codes,
                );
            }

            seen_codes.insert(code.as_str());
            if state.field_tracker.open {
                seen_field_codes.insert(code.as_str());
            }
        }
    }

    has_printable
}

fn process_known_command<'a>(
    command: &CommandNode<'a>,
    env: &KnownCommandEnv<'a>,
    seen_codes: &HashSet<&'a str>,
    state: &mut LabelCommandState<'a>,
    seen_field_codes: &mut HashSet<&'a str>,
) {
    let dspan = Some(command.span);
    let structural_flags = env
        .planning
        .plan_ctx
        .resolve_structural_flags(command.code, command.cmd);
    let producer_key = command
        .cmd
        .codes
        .first()
        .map(String::as_str)
        .unwrap_or(command.code);
    if structural_flags.opens_field {
        seen_field_codes.clear();
    }

    let cmd_ctx = CommandCtx {
        code: command.code,
        args: command.args,
        cmd: command.cmd,
        span: dspan,
        node_idx: command.node_idx,
    };

    apply_effects_and_arity(
        &cmd_ctx,
        structural_flags,
        producer_key,
        env.planning,
        state.label_state,
        state.device_state,
        state.issues,
    );
    let vctx = ValidationContext {
        profile: env.profile,
        label_nodes: &env.label.nodes,
        label_codes: env.label_codes,
        device_state: state.device_state,
    };
    let constraints = ConstraintSets {
        seen_codes,
        seen_field_codes,
        field_scope_codes: env.field_membership.field_id_by_node[command.node_idx]
            .and_then(|fid| env.field_membership.field_codes.get(fid)),
    };

    run_command_validations(
        &cmd_ctx,
        &vctx,
        &constraints,
        env.planning,
        state.label_state,
        state.issues,
    );
    enforce_printer_gates(command.code, command.cmd, env.profile, dspan, state.issues);
    enforce_placement(
        command.code,
        command.cmd,
        env.inside_format_bounds,
        dspan,
        state.issues,
    );

    let maybe_field_command = structural_flags.is_field_related();
    if env.planning.plan.run_field_batch || maybe_field_command {
        state.field_tracker.process_command(
            &cmd_ctx,
            &vctx,
            state.label_state,
            structural_flags,
            state.issues,
        );
    }

    update_session_state(
        command.code,
        command.args,
        command.cmd,
        producer_key,
        state.device_state,
    );

    if structural_flags.closes_field {
        seen_field_codes.clear();
    } else if state.field_tracker.open || structural_flags.opens_field {
        seen_field_codes.insert(command.code);
    }
}

fn apply_effects_and_arity(
    cmd_ctx: &CommandCtx<'_>,
    structural_flags: StructuralFlags,
    producer_key: &str,
    planning: PlanningContext<'_>,
    label_state: &mut LabelState,
    device_state: &mut DeviceState,
    issues: &mut Vec<Diagnostic>,
) {
    let is_effect_producer = planning.plan_ctx.is_effect_producer(
        cmd_ctx.code,
        cmd_ctx.cmd.effects.is_some(),
        planning.plan,
    );
    if is_effect_producer
        && let Some(&consumed) = label_state.producer_consumed.get(producer_key)
        && !consumed
    {
        issues.push(
            diagnostic_with_spec_severity(
                codes::REDUNDANT_STATE,
                format!(
                    "{} overrides a previous {} without any command consuming the earlier value",
                    cmd_ctx.code, producer_key
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!("command" => cmd_ctx.code, "producer" => producer_key)),
        );
    }

    if is_effect_producer {
        label_state.record_producer(producer_key, cmd_ctx.node_idx);
        label_state
            .value_state
            .apply_producer(cmd_ctx.code, cmd_ctx.args, device_state);
    }

    if !structural_flags.field_data && (cmd_ctx.args.len() as u32) > cmd_ctx.cmd.arity {
        issues.push(
            diagnostic_with_spec_severity(
                codes::ARITY,
                format!(
                    "{} has too many arguments ({}>{})",
                    cmd_ctx.code,
                    cmd_ctx.args.len(),
                    cmd_ctx.cmd.arity
                ),
                cmd_ctx.span,
            )
            .with_context(ctx!(
                "command" => cmd_ctx.code,
                "arity" => cmd_ctx.cmd.arity.to_string(),
                "actual" => cmd_ctx.args.len().to_string(),
            )),
        );
    }
}

fn run_command_validations<'a>(
    cmd_ctx: &CommandCtx<'a>,
    vctx: &ValidationContext<'a>,
    constraints: &ConstraintSets<'a>,
    planning: PlanningContext<'_>,
    label_state: &mut LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    validate_command_args(cmd_ctx, vctx, label_state, issues);
    validate_command_constraints(
        cmd_ctx,
        vctx,
        constraints.seen_codes,
        constraints.seen_field_codes,
        constraints.field_scope_codes,
        issues,
    );
    consume_default_from_refs(cmd_ctx, label_state);
    let should_run_structural_semantics = planning.plan_ctx.should_run_structural_semantics(
        cmd_ctx.code,
        cmd_ctx.cmd.structural_rules.is_some(),
        planning.plan,
    );
    if should_run_structural_semantics {
        validate_structural_semantics(cmd_ctx, vctx, label_state, issues);
    }
}

fn enforce_printer_gates(
    code: &str,
    cmd: &CommandEntry,
    profile: Option<&Profile>,
    dspan: Option<zpl_toolchain_diagnostics::Span>,
    issues: &mut Vec<Diagnostic>,
) {
    if let Some(gates) = &cmd.printer_gates
        && let Some(p) = profile
        && let Some(ref features) = p.features
    {
        for gate in gates {
            if let Some(false) = zpl_toolchain_profile::resolve_gate(features, gate) {
                issues.push(
                    diagnostic_with_spec_severity(
                        codes::PRINTER_GATE,
                        format!(
                            "{} requires '{}' capability not available in profile '{}'",
                            code, gate, &p.id
                        ),
                        dspan,
                    )
                    .with_context(ctx!(
                        "command" => code,
                        "gate" => gate.clone(),
                        "level" => "command",
                        "profile" => &p.id,
                    )),
                );
            }
        }
    }
}

fn enforce_placement(
    code: &str,
    cmd: &CommandEntry,
    inside_format_bounds: bool,
    dspan: Option<zpl_toolchain_diagnostics::Span>,
    issues: &mut Vec<Diagnostic>,
) {
    if inside_format_bounds && !matches!(code, "^XA" | "^XZ") && {
        let allowed_inside = cmd.placement.as_ref().and_then(|p| p.allowed_inside_label);
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
            diagnostic_with_spec_severity(
                codes::HOST_COMMAND_IN_LABEL,
                format!("{} should not appear inside a label (^XA/^XZ)", code),
                dspan,
            )
            .with_context(ctx!("command" => code, "plane" => plane)),
        );
    }

    if !inside_format_bounds
        && !matches!(code, "^XA" | "^XZ")
        && cmd.placement.as_ref().and_then(|p| p.allowed_outside_label) == Some(false)
    {
        let plane = match cmd.plane {
            Some(Plane::Format) => "format",
            Some(Plane::Device) => "device",
            Some(Plane::Host) => "host",
            Some(Plane::Config) => "config",
            None => "unknown",
        };
        issues.push(
            diagnostic_with_spec_severity(
                codes::HOST_COMMAND_IN_LABEL,
                format!("{} should not appear outside a label (^XA/^XZ)", code),
                dspan,
            )
            .with_context(ctx!("command" => code, "plane" => plane)),
        );
    }
}

fn update_session_state(
    code: &str,
    args: &[crate::grammar::ast::ArgSlot],
    cmd: &CommandEntry,
    producer_key: &str,
    device_state: &mut DeviceState,
) {
    if cmd.scope == Some(CommandScope::Session) {
        if code == "^MU" {
            device_state.apply_mu(args);
        }
        device_state
            .session_producers
            .insert(producer_key.to_string());
    }
}

fn emit_unclosed_field_diagnostic(
    label: &Label,
    field_tracker: &FieldTracker,
    issues: &mut Vec<Diagnostic>,
) {
    if !field_tracker.open {
        return;
    }

    let dspan = last_command_span(label);
    let mut diag = diagnostic_with_spec_severity(
        codes::FIELD_NOT_CLOSED,
        "field opened but never closed with ^FS before end of label",
        dspan,
    );
    if let Some(Node::Command { code, .. }) = label.nodes.get(field_tracker.start_idx) {
        diag = diag.with_context(ctx!("command" => code));
    }
    issues.push(diag);
}

fn run_label_preflight(
    label: &Label,
    profile: Option<&Profile>,
    device_state: &DeviceState,
    label_codes: &HashSet<&str>,
    plan: &LabelExecutionPlan,
    label_state: &LabelState,
    issues: &mut Vec<Diagnostic>,
) {
    if !plan.run_preflight_gf_memory && !plan.run_preflight_missing_dimensions {
        return;
    }

    let vctx = ValidationContext {
        profile,
        label_nodes: &label.nodes,
        label_codes,
        device_state,
    };
    validate_preflight(
        &vctx,
        label_state,
        plan.run_preflight_gf_memory,
        plan.run_preflight_missing_dimensions,
        first_command_span(label),
        issues,
    );
}

fn emit_empty_label_diagnostic(label: &Label, has_printable: bool, issues: &mut Vec<Diagnostic>) {
    if has_printable {
        return;
    }
    issues.push(diagnostic_with_spec_severity(
        codes::EMPTY_LABEL,
        "Empty label (no commands between ^XA and ^XZ)",
        first_command_span(label),
    ));
}

fn first_command_span(label: &Label) -> Option<zpl_toolchain_diagnostics::Span> {
    label.nodes.first().and_then(|n| {
        if let Node::Command { span, .. } = n {
            Some(*span)
        } else {
            None
        }
    })
}

fn last_command_span(label: &Label) -> Option<zpl_toolchain_diagnostics::Span> {
    label.nodes.last().and_then(|n| {
        if let Node::Command { span, .. } = n {
            Some(*span)
        } else {
            None
        }
    })
}
