pub use crate::grammar::diag::Diagnostic;
use crate::grammar::{ast::Ast, diag::Severity, tables::ParserTables};
use crate::state::{DeviceState, ResolvedLabelState};
use serde::Serialize;
use zpl_toolchain_profile::Profile;

mod args;
mod constraints;
mod context;
mod diagnostics_util;
mod field;
mod pipeline;
mod plan;
mod predicates;
mod preflight;
mod profile_constraints;
mod semantic;
mod state;

use self::diagnostics_util::sort_diagnostics_deterministically;
use self::pipeline::validate_label;
use self::plan::ValidationPlanContext;
#[cfg(test)]
use self::plan::{EffectIndexView, SemanticIndexView, StructuralIndexView};
#[cfg(test)]
pub(crate) use self::predicates::{firmware_version_gte, profile_predicate_matches};
pub use self::profile_constraints::resolve_profile_field;

/// Shorthand for building a `BTreeMap<String, String>` context from key-value pairs.
///
/// ```ignore
/// ctx!("command" => code, "arg" => name, "value" => val)
/// ```
macro_rules! ctx {
    ($($k:expr => $v:expr),+ $(,)?) => {
        std::collections::BTreeMap::from([$(($k.into(), $v.into())),+])
    };
}
pub(super) use ctx;

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
    let plan_ctx = ValidationPlanContext::from_tables(tables);

    let mut device_state = DeviceState::default();
    // Initialize DPI from profile if available
    if let Some(p) = profile {
        device_state.dpi = Some(p.dpi);
    }

    for label in &ast.labels {
        resolved_labels.push(validate_label(
            label,
            tables,
            known,
            &plan_ctx,
            profile,
            &mut device_state,
            &mut issues,
        ));
    }

    sort_diagnostics_deterministically(&mut issues);
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

#[cfg(test)]
mod tests {
    use super::predicates::any_target_in_set;
    use super::*;
    use std::collections::HashSet;
    use zpl_toolchain_profile::{Features, Memory, Profile};

    #[test]
    fn profile_predicate_id_matches() {
        let p = Profile {
            id: "zebra-xi4-203".into(),
            schema_version: "1.0".into(),
            dpi: 203,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: None,
            media: None,
            memory: None,
        };
        assert!(profile_predicate_matches(
            "profile:id:zebra-xi4-203",
            Some(&p)
        ));
        assert!(profile_predicate_matches(
            "profile:id:zebra-xi4-203|other",
            Some(&p)
        ));
        assert!(!profile_predicate_matches("profile:id:other-id", Some(&p)));
        assert!(!profile_predicate_matches("profile:id:zebra-xi4-203", None));
    }

    #[test]
    fn profile_predicate_dpi_matches() {
        let p = Profile {
            id: "test".into(),
            schema_version: "1.0".into(),
            dpi: 600,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: None,
            media: None,
            memory: None,
        };
        assert!(profile_predicate_matches("profile:dpi:600", Some(&p)));
        assert!(profile_predicate_matches("profile:dpi:203|600", Some(&p)));
        assert!(!profile_predicate_matches("profile:dpi:203", Some(&p)));
    }

    #[test]
    fn profile_predicate_feature_matches() {
        let p = Profile {
            id: "test".into(),
            schema_version: "1.0".into(),
            dpi: 203,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: Some(Features {
                cutter: Some(true),
                rfid: Some(false),
                ..Default::default()
            }),
            media: None,
            memory: None,
        };
        assert!(profile_predicate_matches(
            "profile:feature:cutter",
            Some(&p)
        ));
        assert!(profile_predicate_matches(
            "profile:featureMissing:rfid",
            Some(&p)
        ));
        assert!(!profile_predicate_matches("profile:feature:rfid", Some(&p)));
        assert!(!profile_predicate_matches(
            "profile:featureMissing:cutter",
            Some(&p)
        ));
    }

    #[test]
    fn profile_predicate_firmware_prefix() {
        let p = Profile {
            id: "test".into(),
            schema_version: "1.0".into(),
            dpi: 203,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: None,
            media: None,
            memory: Some(Memory {
                ram_kb: None,
                flash_kb: None,
                firmware_version: Some("V60.19.15Z".into()),
            }),
        };
        assert!(profile_predicate_matches("profile:firmware:V60", Some(&p)));
        assert!(profile_predicate_matches(
            "profile:firmware:V60.19",
            Some(&p)
        ));
        assert!(!profile_predicate_matches("profile:firmware:V50", Some(&p)));
    }

    #[test]
    fn firmware_version_gte_ordering() {
        assert!(firmware_version_gte("V60.19.15Z", "V60.14"));
        assert!(firmware_version_gte("V60.19.15Z", "V60.19"));
        assert!(firmware_version_gte("V60.14.0", "V60.14"));
        assert!(!firmware_version_gte("V60.13.9", "V60.14"));
        assert!(!firmware_version_gte("V50.20.0", "V60.14"));
        assert!(firmware_version_gte("X60.16.0", "V60.16"));
    }

    #[test]
    fn any_target_in_set_trims_whitespace() {
        let seen = HashSet::from(["^FD", "^FV"]);
        assert!(any_target_in_set("^FD | ^FO", &seen));
        assert!(any_target_in_set(" ^FV ", &seen));
        assert!(!any_target_in_set(" | ", &seen));
    }

    #[test]
    fn semantic_index_view_tracks_semantic_codes_from_structural_rules() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../generated/parser_tables.json");
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let tables: ParserTables = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to parse {}: {}", path.display(), e));
        let view = SemanticIndexView::from_tables(&tables).expect("expected structural rule index");
        assert!(view.contains("^FN"), "expected ^FN to be present");
        assert!(view.contains("^FO"), "expected ^FO to be present");
        assert!(view.contains("^A"), "expected ^A to be present");
    }

    #[test]
    fn label_execution_plan_disables_batches_for_trivial_label() {
        let plan_ctx = ValidationPlanContext::from_views(
            Some(SemanticIndexView {
                semantic_codes: HashSet::from([String::from("^FO")]),
            }),
            Some(EffectIndexView {
                producer_codes: HashSet::from([String::from("^BY")]),
            }),
            Some(StructuralIndexView {
                opens_field: HashSet::from([String::from("^FO")]),
                closes_field: HashSet::new(),
                field_data: HashSet::new(),
                field_number: HashSet::new(),
                serialization: HashSet::new(),
                requires_field: HashSet::new(),
                hex_escape_modifier: HashSet::new(),
            }),
        );
        let label_codes = HashSet::from(["^ZZ"]);
        let plan = plan_ctx.plan_for_label(&label_codes, None);
        assert!(!plan.run_semantic_batch, "semantic batch should be skipped");
        assert!(!plan.run_effect_batch, "effect batch should be skipped");
        assert!(!plan.run_field_batch, "field batch should be skipped");
        assert!(
            !plan.run_preflight_gf_memory,
            "gf preflight batch should be skipped"
        );
        assert!(
            !plan.run_preflight_missing_dimensions,
            "dimension preflight batch should be skipped without profile"
        );
    }

    #[test]
    fn validation_plan_context_plans_from_injected_views() {
        let plan_ctx = ValidationPlanContext::from_views(
            Some(SemanticIndexView {
                semantic_codes: HashSet::from([String::from("^FO")]),
            }),
            Some(EffectIndexView {
                producer_codes: HashSet::from([String::from("^BY")]),
            }),
            Some(StructuralIndexView {
                opens_field: HashSet::from([String::from("^FO")]),
                closes_field: HashSet::new(),
                field_data: HashSet::new(),
                field_number: HashSet::new(),
                serialization: HashSet::new(),
                requires_field: HashSet::new(),
                hex_escape_modifier: HashSet::new(),
            }),
        );

        let trivial_label = HashSet::from(["^ZZ"]);
        let trivial_plan = plan_ctx.plan_for_label(&trivial_label, None);
        assert!(!trivial_plan.run_semantic_batch);
        assert!(!trivial_plan.run_effect_batch);
        assert!(!trivial_plan.run_field_batch);

        let active_label = HashSet::from(["^FO", "^BY"]);
        let active_plan = plan_ctx.plan_for_label(&active_label, None);
        assert!(active_plan.run_semantic_batch);
        assert!(active_plan.run_effect_batch);
        assert!(active_plan.run_field_batch);
    }

    #[test]
    fn structural_flags_require_index_membership_when_index_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../generated/parser_tables.json");
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let tables: ParserTables = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to parse {}: {}", path.display(), e));
        let fd_cmd = tables
            .cmd_by_code("^FD")
            .expect("expected ^FD command in parser tables");

        // Simulate a stale/partial trigger index that omits ^FD.
        let plan_ctx = ValidationPlanContext::from_views(
            None,
            None,
            Some(StructuralIndexView {
                opens_field: HashSet::new(),
                closes_field: HashSet::new(),
                field_data: HashSet::new(),
                field_number: HashSet::new(),
                serialization: HashSet::new(),
                requires_field: HashSet::new(),
                hex_escape_modifier: HashSet::new(),
            }),
        );

        let flags = plan_ctx.resolve_structural_flags("^FD", fd_cmd);
        assert!(
            !flags.field_data,
            "field_data should remain false when index is present but omits command membership"
        );
    }
}
