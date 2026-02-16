use crate::grammar::{diag::Severity, diag::Span};
use std::collections::HashMap;
use zpl_toolchain_diagnostics::{message_template_for, severity_for_code};

pub(super) use crate::grammar::diag::Diagnostic;

pub(super) fn map_constraint_sev(sev: zpl_toolchain_spec_tables::ConstraintSeverity) -> Severity {
    match sev {
        zpl_toolchain_spec_tables::ConstraintSeverity::Error => Severity::Error,
        zpl_toolchain_spec_tables::ConstraintSeverity::Info => Severity::Info,
        zpl_toolchain_spec_tables::ConstraintSeverity::Warn => Severity::Warn,
    }
}

pub(super) fn map_sev(
    sev: Option<&zpl_toolchain_spec_tables::ConstraintSeverity>,
    default: Option<&zpl_toolchain_spec_tables::ConstraintSeverity>,
) -> Severity {
    if let Some(sev) = sev {
        return map_constraint_sev(*sev);
    }
    if let Some(default) = default {
        return map_constraint_sev(*default);
    }
    Severity::Warn
}

pub(super) fn diagnostic_with_spec_severity(
    id: &'static str,
    message: impl Into<String>,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::new(
        id,
        severity_for_code(id).unwrap_or(Severity::Warn),
        message.into(),
        span,
    )
}

pub(super) fn diagnostic_with_constraint_severity(
    id: &'static str,
    severity: zpl_toolchain_spec_tables::ConstraintSeverity,
    message: impl Into<String>,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::new(id, map_constraint_sev(severity), message.into(), span)
}

pub(super) fn render_diagnostic_message(
    id: &'static str,
    variant: &str,
    substitutions: &[(&str, String)],
    fallback: String,
) -> String {
    let Some(template) = message_template_for(id, variant) else {
        return fallback;
    };
    let substitution_map: HashMap<&str, &str> = substitutions
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    let mut rendered = String::with_capacity(template.len() + 16);
    let mut scan_from = 0usize;
    while let Some(open_rel) = template[scan_from..].find('{') {
        let open = scan_from + open_rel;
        rendered.push_str(&template[scan_from..open]);
        let after_open = open + 1;
        if let Some(close_rel) = template[after_open..].find('}') {
            let close = after_open + close_rel;
            let key = &template[after_open..close];
            if let Some(value) = substitution_map.get(key) {
                rendered.push_str(value);
            } else {
                rendered.push_str(&template[open..=close]);
            }
            scan_from = close + 1;
        } else {
            rendered.push_str(&template[open..]);
            return rendered;
        }
    }
    rendered.push_str(&template[scan_from..]);
    rendered
}

pub(super) fn trim_f64(n: f64) -> String {
    let s = format!("{:.6}", n);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".to_string() } else { s }
}
