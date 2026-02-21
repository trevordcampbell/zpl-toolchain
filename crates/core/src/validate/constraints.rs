use super::context::{CommandCtx, ValidationContext};
use super::ctx;
use super::diagnostics_util::map_sev;
use super::predicates::{any_target_in_set, evaluate_note_when_expression};
use crate::grammar::diag::codes;
use std::collections::HashSet;
use zpl_toolchain_spec_tables::{CommandScope, ConstraintKind, ConstraintScope, NoteAudience};

pub(super) fn validate_command_constraints(
    cmd_ctx: &CommandCtx,
    vctx: &ValidationContext,
    seen_label_codes: &HashSet<&str>,
    seen_field_codes: &HashSet<&str>,
    current_field_codes: Option<&HashSet<&str>>,
    issues: &mut Vec<super::Diagnostic>,
) {
    let Some(constraints) = cmd_ctx.cmd.constraints.as_ref() else {
        return;
    };
    let constraint_default_severity = cmd_ctx
        .cmd
        .constraint_defaults
        .as_ref()
        .and_then(|defaults| defaults.severity.as_ref());
    let empty_field_codes: HashSet<&str> = HashSet::new();

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
                                super::Diagnostic::new(
                                    codes::ORDER_BEFORE,
                                    map_sev(c.severity.as_ref(), constraint_default_severity),
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
                            super::Diagnostic::new(
                                codes::ORDER_AFTER,
                                map_sev(c.severity.as_ref(), constraint_default_severity),
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
                    // Canonical default: requires evaluates label-wide unless
                    // a field scope is explicitly declared in the spec.
                    let eval_scope = c.scope.unwrap_or(ConstraintScope::Label);
                    let target_codes = if eval_scope == ConstraintScope::Field {
                        current_field_codes.unwrap_or(&empty_field_codes)
                    } else {
                        vctx.label_codes
                    };
                    if !any_target_in_set(expr, target_codes) {
                        issues.push(
                            super::Diagnostic::new(
                                codes::REQUIRED_COMMAND,
                                map_sev(c.severity.as_ref(), constraint_default_severity),
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
                    // Canonical default: incompatible evaluates label-wide unless
                    // a field scope is explicitly declared in the spec.
                    let eval_scope = c.scope.unwrap_or(ConstraintScope::Label);
                    let target_codes = if eval_scope == ConstraintScope::Field {
                        current_field_codes.unwrap_or(&empty_field_codes)
                    } else {
                        vctx.label_codes
                    };
                    if any_target_in_set(expr, target_codes) {
                        issues.push(
                            super::Diagnostic::new(
                                codes::INCOMPATIBLE_COMMAND,
                                map_sev(c.severity.as_ref(), constraint_default_severity),
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
                        super::Diagnostic::new(
                            codes::EMPTY_FIELD_DATA,
                            map_sev(c.severity.as_ref(), constraint_default_severity),
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
                // - when:<predicate expression> where predicates can reference:
                //   - arg:keyIsValue:V1|V2
                //   - arg:keyPresent / arg:keyEmpty
                //   - label:has:^CODE / label:missing:^CODE
                //   Supports ! (not), &&, and ||.
                // where <codes> can be a single command or pipe-separated list.
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
                    } else if let Some(condition) = expr.strip_prefix("when:") {
                        evaluate_note_when_expression(
                            condition.trim(),
                            cmd_ctx.args,
                            seen_codes,
                            vctx.profile,
                        )
                    } else {
                        true
                    }
                } else {
                    true
                };
                if !should_emit {
                    continue;
                }
                let mut diagnostic = super::Diagnostic::new(
                    codes::NOTE,
                    map_sev(c.severity.as_ref(), constraint_default_severity),
                    c.message.clone(),
                    cmd_ctx.span,
                )
                .with_context(ctx!("command" => cmd_ctx.code));
                if matches!(c.audience, Some(NoteAudience::Contextual))
                    && let Some(context) = diagnostic.context.as_mut()
                {
                    context.insert("audience".to_string(), "contextual".to_string());
                }
                issues.push(diagnostic);
            }
            // Range constraints are a future extension point — currently, range
            // validation is handled through `args[].range` on each Arg definition.
            // When activated, the constraint's `expr` would specify the range
            // and `message` would provide context.
            ConstraintKind::Range | ConstraintKind::Custom => {}
        }
    }
}
