//! Shared state tracking for validator and future renderer.
//!
//! This module captures typed cross-command state values instead of only
//! producer presence, enabling a single source of truth for effective defaults
//! and layout-affecting values.

use crate::grammar::ast::ArgSlot;
use serde::Serialize;
use std::collections::HashSet;

/// Unit system for measurement conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum Units {
    /// Values are already in printer dots.
    #[default]
    Dots,
    /// Values are expressed in inches.
    Inches,
    /// Values are expressed in millimeters.
    Millimeters,
}

/// Convert a value from the active unit system to dots.
pub fn convert_to_dots(value: f64, units: Units, dpi: u32) -> f64 {
    match units {
        Units::Dots => value,
        Units::Inches => value * dpi as f64,
        Units::Millimeters => value * dpi as f64 / 25.4,
    }
}

/// Session/device-scoped state that persists across labels.
#[derive(Debug, Default, Clone, Serialize)]
pub struct DeviceState {
    /// Session-scoped producer tracking (persists across labels).
    pub session_producers: HashSet<String>,
    /// Active unit system from `^MU` (default: dots).
    pub units: Units,
    /// DPI for unit conversion (from profile or `^MU`).
    pub dpi: Option<u32>,
}

impl DeviceState {
    /// Applies `^MU` settings to active units and optional conversion DPI.
    pub fn apply_mu(&mut self, args: &[ArgSlot]) {
        if let Some(unit_arg) = args.first().and_then(|a| a.value.as_deref()) {
            self.units = match unit_arg.to_ascii_uppercase().as_str() {
                "I" => Units::Inches,
                "M" => Units::Millimeters,
                _ => Units::Dots,
            };
        }

        let format_base_dpi = args
            .get(1)
            .and_then(|a| a.value.as_deref())
            .and_then(|s| s.parse::<u32>().ok());
        let desired_dpi = args
            .get(2)
            .and_then(|a| a.value.as_deref())
            .and_then(|s| s.parse::<u32>().ok());

        if format_base_dpi.is_some()
            && let Some(dpi) = desired_dpi
        {
            self.dpi = Some(dpi);
        }
    }
}

/// Typed barcode defaults from `^BY`.
#[derive(Debug, Default, Clone, Serialize)]
pub struct BarcodeDefaults {
    /// Default module width in dots.
    pub module_width: Option<u32>,
    /// Default wide-to-narrow ratio.
    pub ratio: Option<f64>,
    /// Default barcode height in dots.
    pub height: Option<u32>,
}

/// Typed font defaults from `^CF`.
#[derive(Debug, Default, Clone, Serialize)]
pub struct FontDefaults {
    /// Default font identifier.
    pub font: Option<char>,
    /// Default font height in dots.
    pub height: Option<u32>,
    /// Default font width in dots.
    pub width: Option<u32>,
}

/// Typed field orientation defaults from `^FW`.
#[derive(Debug, Default, Clone, Serialize)]
pub struct FieldOrientationDefaults {
    /// Default orientation (N/R/I/B).
    pub orientation: Option<char>,
    /// Default justification value.
    pub justification: Option<u8>,
}

/// Typed layout-affecting settings used by validator and renderer.
#[derive(Debug, Default, Clone, Serialize)]
pub struct LayoutDefaults {
    /// Print width (`^PW`) in dots.
    pub print_width: Option<f64>,
    /// Label length (`^LL`) in dots.
    pub label_length: Option<f64>,
    /// Print orientation (`^PO`), e.g. `N` or `I`.
    pub print_orientation: Option<char>,
    /// Mirror image setting (`^PM`), usually `Y`/`N`.
    pub mirror_image: Option<char>,
    /// Label reverse print (`^LR`), usually `Y`/`N`.
    pub reverse_print: Option<char>,
    /// Label top offset (`^LT`) in dots.
    pub label_top: Option<f64>,
    /// Label shift (`^LS`) in dots.
    pub label_shift: Option<f64>,
}

/// Typed label-home offset from `^LH` (stored in dots).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LabelHome {
    /// Home X offset in dots.
    pub x: f64,
    /// Home Y offset in dots.
    pub y: f64,
}

impl Default for LabelHome {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// Per-label typed producer values.
#[derive(Debug, Default, Clone, Serialize)]
pub struct LabelValueState {
    /// Barcode defaults resolved from `^BY`.
    pub barcode: BarcodeDefaults,
    /// Font defaults resolved from `^CF`.
    pub font: FontDefaults,
    /// Field orientation defaults resolved from `^FW`.
    pub field: FieldOrientationDefaults,
    /// Label home offset resolved from `^LH`.
    pub label_home: LabelHome,
    /// Additional layout-affecting defaults.
    pub layout: LayoutDefaults,
}

/// Stable renderer-ready snapshot of resolved per-label state.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ResolvedLabelState {
    /// Typed values produced by stateful commands in this label.
    pub values: LabelValueState,
    /// Effective print width after profile + in-label overrides, in dots.
    pub effective_width: Option<f64>,
    /// Effective label length after profile + in-label overrides, in dots.
    pub effective_height: Option<f64>,
}

impl LabelValueState {
    /// Updates typed state from a producer command.
    pub fn apply_producer(&mut self, code: &str, args: &[ArgSlot], device_state: &DeviceState) {
        match code {
            "^BY" => self.apply_by(args),
            "^CF" => self.apply_cf(args),
            "^FW" => self.apply_fw(args),
            "^LH" => self.apply_lh(args, device_state),
            "^PW" => self.apply_pw(args, device_state),
            "^LL" => self.apply_ll(args, device_state),
            "^PO" => self.layout.print_orientation = parse_char_arg(args, 0),
            "^PM" => self.layout.mirror_image = parse_char_arg(args, 0),
            "^LR" => self.layout.reverse_print = parse_char_arg(args, 0),
            "^LT" => {
                self.layout.label_top =
                    parse_f64_arg(args, 0).map(|v| normalize_to_dots(v, device_state))
            }
            "^LS" => {
                self.layout.label_shift =
                    parse_f64_arg(args, 0).map(|v| normalize_to_dots(v, device_state))
            }
            _ => {}
        }
    }

    fn apply_by(&mut self, args: &[ArgSlot]) {
        if let Some(w) = parse_u32_arg(args, 0) {
            self.barcode.module_width = Some(w);
        }
        if let Some(r) = parse_f64_arg(args, 1) {
            self.barcode.ratio = Some(r);
        }
        if let Some(h) = parse_u32_arg(args, 2) {
            self.barcode.height = Some(h);
        }
    }

    fn apply_cf(&mut self, args: &[ArgSlot]) {
        if let Some(font_char) = args
            .first()
            .and_then(|a| a.value.as_ref())
            .and_then(|s| s.chars().next())
        {
            self.font.font = Some(font_char);
        }
        if let Some(h) = parse_u32_arg(args, 1) {
            self.font.height = Some(h);
        }
        if let Some(w) = parse_u32_arg(args, 2) {
            self.font.width = Some(w);
        }
    }

    fn apply_fw(&mut self, args: &[ArgSlot]) {
        if let Some(orientation_char) = args
            .first()
            .and_then(|a| a.value.as_ref())
            .and_then(|s| s.chars().next())
        {
            self.field.orientation = Some(orientation_char);
        }
        if let Some(justification) = parse_u8_arg(args, 1) {
            self.field.justification = Some(justification);
        }
    }

    fn apply_lh(&mut self, args: &[ArgSlot], device_state: &DeviceState) {
        if let Some(x_raw) = parse_f64_arg(args, 0) {
            self.label_home.x = normalize_to_dots(x_raw, device_state);
        }
        if let Some(y_raw) = parse_f64_arg(args, 1) {
            self.label_home.y = normalize_to_dots(y_raw, device_state);
        }
    }

    fn apply_pw(&mut self, args: &[ArgSlot], device_state: &DeviceState) {
        if let Some(width) = parse_f64_arg(args, 0)
            && width.is_finite()
            && width > 0.0
        {
            self.layout.print_width = Some(normalize_to_dots(width, device_state));
        }
    }

    fn apply_ll(&mut self, args: &[ArgSlot], device_state: &DeviceState) {
        if let Some(length) = parse_f64_arg(args, 0)
            && length.is_finite()
            && length > 0.0
        {
            self.layout.label_length = Some(normalize_to_dots(length, device_state));
        }
    }

    /// Resolve a typed default value by state key path.
    pub fn state_value_by_key(&self, key: &str) -> Option<String> {
        match key {
            "barcode.moduleWidth" => self.barcode.module_width.map(|v| v.to_string()),
            "barcode.ratio" => self.barcode.ratio.map(trim_f64),
            "barcode.height" => self.barcode.height.map(|v| v.to_string()),
            "font.name" => self.font.font.map(|c| c.to_string()),
            "font.height" => self.font.height.map(|v| v.to_string()),
            "font.width" => self.font.width.map(|v| v.to_string()),
            "field.orientation" => self.field.orientation.map(|c| c.to_string()),
            "field.justification" => self.field.justification.map(|v| v.to_string()),
            "label.home.x" => Some(trim_f64(self.label_home.x)),
            "label.home.y" => Some(trim_f64(self.label_home.y)),
            "label.width" => self.layout.print_width.map(trim_f64),
            "label.length" => self.layout.label_length.map(trim_f64),
            "print.orientation" => self.layout.print_orientation.map(|c| c.to_string()),
            "print.mirror" => self.layout.mirror_image.map(|c| c.to_string()),
            "label.reversePrint" => self.layout.reverse_print.map(|c| c.to_string()),
            "label.top" => self.layout.label_top.map(trim_f64),
            "label.shift" => self.layout.label_shift.map(trim_f64),
            _ => None,
        }
    }
}

fn normalize_to_dots(value: f64, device_state: &DeviceState) -> f64 {
    match (device_state.units, device_state.dpi) {
        (Units::Dots, _) => value,
        (_, Some(dpi)) => convert_to_dots(value, device_state.units, dpi),
        // Keep legacy validator behavior: when units are non-dot but DPI is
        // unavailable, preserve raw values (do not guess a conversion factor).
        (Units::Inches | Units::Millimeters, None) => value,
    }
}

fn parse_f64_arg(args: &[ArgSlot], idx: usize) -> Option<f64> {
    args.get(idx)
        .and_then(|a| a.value.as_deref())
        .and_then(|s| s.parse::<f64>().ok())
}

fn parse_u32_arg(args: &[ArgSlot], idx: usize) -> Option<u32> {
    args.get(idx)
        .and_then(|a| a.value.as_deref())
        .and_then(|s| s.parse::<u32>().ok())
}

fn parse_u8_arg(args: &[ArgSlot], idx: usize) -> Option<u8> {
    args.get(idx)
        .and_then(|a| a.value.as_deref())
        .and_then(|s| s.parse::<u8>().ok())
}

fn parse_char_arg(args: &[ArgSlot], idx: usize) -> Option<char> {
    args.get(idx)
        .and_then(|a| a.value.as_deref())
        .and_then(|s| s.chars().next())
}

fn trim_f64(n: f64) -> String {
    let s = format!("{:.6}", n);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::ast::Presence;

    fn slot(value: &str) -> ArgSlot {
        ArgSlot {
            key: None,
            presence: Presence::Value,
            value: Some(value.to_string()),
        }
    }

    #[test]
    fn applies_typed_by_defaults() {
        let mut state = LabelValueState::default();
        state.apply_producer(
            "^BY",
            &[slot("3"), slot("2.5"), slot("80")],
            &DeviceState::default(),
        );
        assert_eq!(state.barcode.module_width, Some(3));
        assert_eq!(state.barcode.ratio, Some(2.5));
        assert_eq!(state.barcode.height, Some(80));
    }

    #[test]
    fn applies_typed_cf_and_fw_defaults() {
        let mut state = LabelValueState::default();
        state.apply_producer(
            "^CF",
            &[slot("A"), slot("28"), slot("16")],
            &DeviceState::default(),
        );
        state.apply_producer("^FW", &[slot("R"), slot("1")], &DeviceState::default());
        assert_eq!(state.font.font, Some('A'));
        assert_eq!(state.font.height, Some(28));
        assert_eq!(state.font.width, Some(16));
        assert_eq!(state.field.orientation, Some('R'));
        assert_eq!(state.field.justification, Some(1));
    }

    #[test]
    fn applies_label_home_in_active_units() {
        let mut device = DeviceState::default();
        device.apply_mu(&[slot("I"), slot("203"), slot("203")]);
        let mut state = LabelValueState::default();
        state.apply_producer("^LH", &[slot("1"), slot("0.5")], &device);
        assert_eq!(state.label_home.x, 203.0);
        assert_eq!(state.label_home.y, 101.5);
    }

    #[test]
    fn applies_layout_defaults_from_producers() {
        let mut device = DeviceState::default();
        device.apply_mu(&[slot("I"), slot("203"), slot("203")]);
        let mut state = LabelValueState::default();
        state.apply_producer("^PW", &[slot("4")], &device);
        state.apply_producer("^LL", &[slot("6")], &device);
        state.apply_producer("^PO", &[slot("I")], &device);
        state.apply_producer("^PM", &[slot("Y")], &device);
        state.apply_producer("^LR", &[slot("N")], &device);
        state.apply_producer("^LT", &[slot("0.5")], &device);
        state.apply_producer("^LS", &[slot("0.25")], &device);

        assert_eq!(state.layout.print_width, Some(812.0));
        assert_eq!(state.layout.label_length, Some(1218.0));
        assert_eq!(state.layout.print_orientation, Some('I'));
        assert_eq!(state.layout.mirror_image, Some('Y'));
        assert_eq!(state.layout.reverse_print, Some('N'));
        assert_eq!(state.layout.label_top, Some(101.5));
        assert_eq!(state.layout.label_shift, Some(50.75));
    }

    #[test]
    fn resolves_state_values_by_key() {
        let mut state = LabelValueState::default();
        state.apply_producer(
            "^BY",
            &[slot("3"), slot("2.5"), slot("80")],
            &DeviceState::default(),
        );
        assert_eq!(
            state.state_value_by_key("barcode.moduleWidth"),
            Some("3".to_string())
        );
        assert_eq!(
            state.state_value_by_key("barcode.ratio"),
            Some("2.5".to_string())
        );
        assert_eq!(
            state.state_value_by_key("barcode.height"),
            Some("80".to_string())
        );
    }

    #[test]
    fn partial_by_updates_only_provided_fields() {
        let mut state = LabelValueState::default();
        state.apply_producer(
            "^BY",
            &[slot("2"), slot("2.5"), slot("40")],
            &DeviceState::default(),
        );
        state.apply_producer("^BY", &[slot("4")], &DeviceState::default());
        assert_eq!(state.barcode.module_width, Some(4));
        assert_eq!(state.barcode.ratio, Some(2.5));
        assert_eq!(state.barcode.height, Some(40));
    }
}
