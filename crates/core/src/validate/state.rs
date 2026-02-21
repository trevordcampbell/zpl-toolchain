use crate::state::LabelValueState;
use std::collections::{HashMap, HashSet};

/// Tracks label-local producer/consumer state used by validator checks.
#[derive(Debug, Default)]
pub(super) struct LabelState {
    /// Set of producer commands that have been seen in this label.
    pub(super) producers_seen: HashSet<String>,
    /// Track field numbers seen (for duplicate ^FN detection)
    pub(super) field_numbers: HashMap<String, usize>, // value -> first node_idx
    /// Track ^CW font registrations (font letter -> node_idx)
    pub(super) loaded_fonts: HashSet<char>,
    /// Track last producer position for redundant state detection
    pub(super) last_producer_idx: HashMap<String, usize>,
    /// Track whether any consumer has used a producer's state since it was set
    pub(super) producer_consumed: HashMap<String, bool>,
    /// Track effective print width (from ^PW) and label length (from ^LL)
    pub(super) effective_width: Option<f64>,
    pub(super) effective_height: Option<f64>,
    /// Whether ^PW was explicitly set in this label (vs inherited from profile).
    pub(super) has_explicit_pw: bool,
    /// Whether ^LL was explicitly set in this label (vs inherited from profile).
    pub(super) has_explicit_ll: bool,
    /// Last ^FO x position (for graphic bounds checking).
    pub(super) last_fo_x: Option<f64>,
    /// Last ^FO y position (for graphic bounds checking).
    pub(super) last_fo_y: Option<f64>,
    /// Accumulated total graphic bytes from ^GF commands (for memory estimation).
    pub(super) gf_total_bytes: u32,
    /// Typed producer values for renderer/validator default resolution.
    pub(super) value_state: LabelValueState,
}

impl LabelState {
    /// Record that a state-producing command was seen.
    pub(super) fn record_producer(&mut self, code: &str, node_idx: usize) {
        let key = code.to_string();
        self.producers_seen.insert(key.clone());
        self.last_producer_idx.insert(key.clone(), node_idx);
        self.producer_consumed.insert(key, false);
    }

    /// Check if a given producer command has been seen.
    pub(super) fn has_producer(&self, code: &str) -> bool {
        self.producers_seen.contains(code)
    }

    /// Mark a producer as consumed (its state was used by a consumer command).
    pub(super) fn mark_consumed(&mut self, producer_code: &str) {
        if let Some(consumed) = self.producer_consumed.get_mut(producer_code) {
            *consumed = true;
        }
    }
}
