use crate::grammar::tables::ParserTables;
use std::collections::HashSet;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::{CommandEntry, StructuralRuleIndex, StructuralTrigger};

use super::resolve_profile_field;

#[derive(Default)]
pub(super) struct SemanticIndexView {
    pub(super) semantic_codes: HashSet<String>,
}

impl SemanticIndexView {
    pub(super) fn from_tables(tables: &ParserTables) -> Option<Self> {
        let mut semantic_codes = HashSet::new();
        if let Some(idx) = tables.structural_rule_index.as_ref() {
            for codes in idx.by_kind.values() {
                semantic_codes.extend(codes.iter().cloned());
            }
        } else {
            for cmd in &tables.commands {
                if cmd.structural_rules.is_some() {
                    semantic_codes.extend(cmd.codes.iter().cloned());
                }
            }
        }
        Some(Self { semantic_codes })
    }

    #[cfg(test)]
    pub(super) fn contains(&self, code: &str) -> bool {
        self.semantic_codes.contains(code)
    }
}

#[derive(Default)]
pub(super) struct EffectIndexView {
    pub(super) producer_codes: HashSet<String>,
}

impl EffectIndexView {
    pub(super) fn from_tables(tables: &ParserTables) -> Option<Self> {
        let mut producer_codes = HashSet::new();
        if let Some(idx) = tables.structural_rule_index.as_ref() {
            for codes in idx.by_effect.values() {
                producer_codes.extend(codes.iter().cloned());
            }
        } else {
            for cmd in &tables.commands {
                if cmd.effects.is_some() {
                    producer_codes.extend(cmd.codes.iter().cloned());
                }
            }
        }
        Some(Self { producer_codes })
    }

    pub(super) fn contains(&self, code: &str) -> bool {
        self.producer_codes.contains(code)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StructuralFlags {
    pub(super) opens_field: bool,
    pub(super) closes_field: bool,
    pub(super) field_data: bool,
    pub(super) field_number: bool,
    pub(super) serialization: bool,
    pub(super) requires_field: bool,
    pub(super) hex_escape_modifier: bool,
}

impl StructuralFlags {
    pub(super) fn is_field_related(self) -> bool {
        self.opens_field
            || self.closes_field
            || self.field_data
            || self.requires_field
            || self.hex_escape_modifier
            || self.field_number
            || self.serialization
    }
}

#[derive(Default)]
pub(super) struct ValidationPlanContext {
    semantic_index: Option<SemanticIndexView>,
    effect_index: Option<EffectIndexView>,
    structural_index: Option<StructuralIndexView>,
}

impl ValidationPlanContext {
    pub(super) fn from_tables(tables: &ParserTables) -> Self {
        Self {
            semantic_index: SemanticIndexView::from_tables(tables),
            effect_index: EffectIndexView::from_tables(tables),
            structural_index: StructuralIndexView::from_tables(tables),
        }
    }

    #[cfg(test)]
    pub(super) fn from_views(
        semantic_index: Option<SemanticIndexView>,
        effect_index: Option<EffectIndexView>,
        structural_index: Option<StructuralIndexView>,
    ) -> Self {
        Self {
            semantic_index,
            effect_index,
            structural_index,
        }
    }

    pub(super) fn plan_for_label(
        &self,
        label_codes: &HashSet<&str>,
        profile: Option<&Profile>,
    ) -> LabelExecutionPlan {
        LabelExecutionPlan::from_indexes(
            label_codes,
            self.semantic_index.as_ref(),
            self.effect_index.as_ref(),
            self.structural_index.as_ref(),
            profile,
        )
    }

    pub(super) fn should_run_structural_semantics(
        &self,
        _code: &str,
        cmd_has_structural_rules: bool,
        plan: &LabelExecutionPlan,
    ) -> bool {
        cmd_has_structural_rules || plan.run_semantic_batch
    }

    pub(super) fn is_effect_producer(
        &self,
        code: &str,
        cmd_has_effects: bool,
        plan: &LabelExecutionPlan,
    ) -> bool {
        cmd_has_effects
            || (plan.run_effect_batch
                && self
                    .effect_index
                    .as_ref()
                    .is_some_and(|idx| idx.contains(code)))
    }

    fn opens_field(&self, code: &str, cmd_opens_field: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_opens_field, |idx| idx.opens_field.contains(code))
    }

    fn closes_field(&self, code: &str, cmd_closes_field: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_closes_field, |idx| idx.closes_field.contains(code))
    }

    fn field_data(&self, code: &str, cmd_field_data: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_field_data, |idx| idx.field_data.contains(code))
    }

    fn requires_field(&self, code: &str, cmd_requires_field: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_requires_field, |idx| idx.requires_field.contains(code))
    }

    fn hex_escape_modifier(&self, code: &str, cmd_hex_escape_modifier: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_hex_escape_modifier, |idx| {
                idx.hex_escape_modifier.contains(code)
            })
    }

    fn field_number(&self, code: &str, cmd_field_number: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_field_number, |idx| idx.field_number.contains(code))
    }

    fn serialization(&self, code: &str, cmd_serialization: bool) -> bool {
        self.structural_index
            .as_ref()
            .map_or(cmd_serialization, |idx| idx.serialization.contains(code))
    }

    pub(super) fn resolve_structural_flags(
        &self,
        code: &str,
        cmd: &CommandEntry,
    ) -> StructuralFlags {
        StructuralFlags {
            opens_field: self.opens_field(code, cmd.opens_field),
            closes_field: self.closes_field(code, cmd.closes_field),
            field_data: self.field_data(code, cmd.field_data),
            field_number: self.field_number(code, cmd.field_number),
            serialization: self.serialization(code, cmd.serialization),
            requires_field: self.requires_field(code, cmd.requires_field),
            hex_escape_modifier: self.hex_escape_modifier(code, cmd.hex_escape_modifier),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct LabelExecutionPlan {
    pub(super) run_semantic_batch: bool,
    pub(super) run_effect_batch: bool,
    pub(super) run_field_batch: bool,
    pub(super) run_preflight_gf_memory: bool,
    pub(super) run_preflight_missing_dimensions: bool,
}

impl LabelExecutionPlan {
    fn from_indexes(
        label_codes: &HashSet<&str>,
        semantic_index: Option<&SemanticIndexView>,
        effect_index: Option<&EffectIndexView>,
        structural_index: Option<&StructuralIndexView>,
        profile: Option<&Profile>,
    ) -> Self {
        let run_semantic_batch = semantic_index
            .map(|idx| {
                if idx.semantic_codes.is_empty() {
                    return true;
                }
                idx.semantic_codes
                    .iter()
                    .any(|c| label_codes.contains(c.as_str()))
            })
            .unwrap_or(true);
        let run_effect_batch = effect_index
            .map(|idx| {
                idx.producer_codes
                    .iter()
                    .any(|c| label_codes.contains(c.as_str()))
            })
            .unwrap_or(true);
        let run_field_batch = structural_index
            .map(|idx| {
                idx.opens_field
                    .iter()
                    .chain(idx.closes_field.iter())
                    .chain(idx.field_data.iter())
                    .chain(idx.requires_field.iter())
                    .chain(idx.hex_escape_modifier.iter())
                    .chain(idx.field_number.iter())
                    .chain(idx.serialization.iter())
                    .any(|c| label_codes.contains(c.as_str()))
            })
            .unwrap_or(true);
        let run_preflight_gf_memory = label_codes.contains("^GF");
        let run_preflight_missing_dimensions = profile.is_some_and(|p| {
            resolve_profile_field(p, "page.width_dots").is_some()
                || resolve_profile_field(p, "page.height_dots").is_some()
        });
        Self {
            run_semantic_batch,
            run_effect_batch,
            run_field_batch,
            run_preflight_gf_memory,
            run_preflight_missing_dimensions,
        }
    }
}

#[derive(Default)]
pub(super) struct StructuralIndexView {
    pub(super) opens_field: HashSet<String>,
    pub(super) closes_field: HashSet<String>,
    pub(super) field_data: HashSet<String>,
    pub(super) field_number: HashSet<String>,
    pub(super) serialization: HashSet<String>,
    pub(super) requires_field: HashSet<String>,
    pub(super) hex_escape_modifier: HashSet<String>,
}

impl StructuralIndexView {
    fn from_tables(tables: &ParserTables) -> Option<Self> {
        if let Some(idx) = tables.structural_rule_index.as_ref() {
            return Some(Self {
                opens_field: codes_for_trigger(idx, StructuralTrigger::OpensField),
                closes_field: codes_for_trigger(idx, StructuralTrigger::ClosesField),
                field_data: codes_for_trigger(idx, StructuralTrigger::FieldData),
                field_number: codes_for_trigger(idx, StructuralTrigger::FieldNumber),
                serialization: codes_for_trigger(idx, StructuralTrigger::Serialization),
                requires_field: codes_for_trigger(idx, StructuralTrigger::RequiresField),
                hex_escape_modifier: codes_for_trigger(idx, StructuralTrigger::HexEscapeModifier),
            });
        }

        let mut view = Self::default();
        for cmd in &tables.commands {
            let add_codes = |target: &mut HashSet<String>| {
                target.extend(cmd.codes.iter().cloned());
            };
            if cmd.opens_field {
                add_codes(&mut view.opens_field);
            }
            if cmd.closes_field {
                add_codes(&mut view.closes_field);
            }
            if cmd.field_data {
                add_codes(&mut view.field_data);
            }
            if cmd.field_number {
                add_codes(&mut view.field_number);
            }
            if cmd.serialization {
                add_codes(&mut view.serialization);
            }
            if cmd.requires_field {
                add_codes(&mut view.requires_field);
            }
            if cmd.hex_escape_modifier {
                add_codes(&mut view.hex_escape_modifier);
            }
        }
        Some(view)
    }
}

fn codes_for_trigger(index: &StructuralRuleIndex, trigger: StructuralTrigger) -> HashSet<String> {
    index
        .by_trigger
        .get(&trigger)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect()
}
