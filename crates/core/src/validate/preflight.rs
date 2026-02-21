use super::context::ValidationContext;
use super::diagnostics_util::diagnostic_with_spec_severity;
use super::resolve_profile_field;
use super::state::LabelState;
use crate::grammar::diag::{Diagnostic, Span, codes};
use std::collections::BTreeMap;

/// Preflight validation that runs after all nodes in a label have been processed.
///
/// Checks:
/// - **ZPL2309**: Total graphic memory exceeds available RAM (profile-gated)
/// - **ZPL2310**: Label lacks explicit ^PW/^LL when profile provides dimensions
pub(super) fn validate_preflight(
    vctx: &ValidationContext,
    label_state: &LabelState,
    run_gf_memory_check: bool,
    run_missing_dimensions_check: bool,
    label_span: Option<Span>,
    issues: &mut Vec<Diagnostic>,
) {
    // ZPL2309: Graphics memory estimation
    if run_gf_memory_check
        && label_state.gf_total_bytes > 0
        && let Some(profile) = vctx.profile
        && let Some(ram_kb) = resolve_profile_field(profile, "memory.ram_kb")
    {
        let ram_bytes = ram_kb as u64 * 1024;
        if label_state.gf_total_bytes as u64 > ram_bytes {
            issues.push(
                diagnostic_with_spec_severity(
                    codes::GF_MEMORY_EXCEEDED,
                    format!(
                        "Total graphic data ({} bytes) exceeds available RAM ({} bytes / {} KB)",
                        label_state.gf_total_bytes, ram_bytes, ram_kb as u64,
                    ),
                    label_span,
                )
                .with_context(BTreeMap::from([
                    ("command".into(), "^GF".into()),
                    ("total_bytes".into(), label_state.gf_total_bytes.to_string()),
                    ("ram_bytes".into(), ram_bytes.to_string()),
                ])),
            );
        }
    }

    // ZPL2310: Missing explicit dimensions
    if run_missing_dimensions_check && let Some(profile) = vctx.profile {
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
                    diagnostic_with_spec_severity(
                        codes::MISSING_EXPLICIT_DIMENSIONS,
                        format!(
                            "Label relies on profile for dimensions but does not contain explicit {} — consider adding for portability",
                            missing_str,
                        ),
                        label_span,
                    )
                    .with_context(BTreeMap::from([(
                        "missing_commands".into(),
                        missing_str,
                    )])),
                );
            }
        }
    }
}
