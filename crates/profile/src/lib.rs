//! Printer profile definitions and validation for the ZPL toolchain.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when loading or validating a printer profile.
#[derive(Debug, Error)]
pub enum ProfileError {
    /// JSON deserialization failed.
    #[error("invalid profile JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// A required field value is out of its valid range.
    #[error("invalid {field}: {reason}")]
    InvalidField {
        /// The name of the field that failed validation.
        field: String,
        /// A human-readable explanation of why the field value is invalid.
        reason: String,
    },
}

/// A printer profile describing the capabilities and constraints of a
/// specific Zebra label printer (or class of printers).
///
/// Profiles drive data-driven validation: command arguments annotated with
/// `profileConstraint` in the spec are automatically checked against the
/// corresponding profile field at lint time, and `printerGates` on commands
/// or enum values are checked against the `features` flags.
///
/// # Example
/// ```
/// let profile = zpl_toolchain_profile::Profile {
///     id: "zebra-generic-203".into(),
///     schema_version: "1.0.0".into(),
///     dpi: 203,
///     page: Some(zpl_toolchain_profile::Page {
///         width_dots: Some(812),
///         height_dots: Some(1218),
///     }),
///     speed_range: Some(zpl_toolchain_profile::Range { min: 2, max: 8 }),
///     darkness_range: Some(zpl_toolchain_profile::Range { min: 0, max: 30 }),
///     features: Some(zpl_toolchain_profile::Features {
///         cutter: Some(false),
///         rfid: Some(false),
///         ..Default::default()
///     }),
///     media: None,
///     memory: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    /// Unique profile identifier (e.g., `"zebra-generic-203"`).
    pub id: String,
    /// Profile schema version for forward compatibility (e.g., `"1.0.0"`).
    pub schema_version: String,
    /// Print resolution in dots per inch (typically 150, 200, 203, 300, or 600).
    pub dpi: u32,
    /// Page/label dimension constraints.
    pub page: Option<Page>,
    /// Supported print speed range in inches per second.
    pub speed_range: Option<Range>,
    /// Supported darkness setting range.
    pub darkness_range: Option<Range>,
    /// Hardware feature flags for `printerGates` enforcement.
    pub features: Option<Features>,
    /// Media capability descriptors.
    pub media: Option<Media>,
    /// Memory and firmware information.
    pub memory: Option<Memory>,
}

/// Page/label dimension constraints for a printer profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Page {
    /// Maximum printhead width in dots.
    pub width_dots: Option<u32>,
    /// Maximum label length in dots (memory-dependent).
    pub height_dots: Option<u32>,
}

/// A min/max range for numeric printer capabilities (e.g., speed, darkness).
///
/// Invariant: `min <= max`.
///
/// Prefer [`Range::new`] or [`Range::try_new`] to construct a `Range` with
/// automatic invariant checking. Direct struct-literal construction is still
/// possible for convenience (e.g. in tests or serde), but callers must uphold
/// the invariant themselves; [`load_profile_from_str`] validates it for
/// deserialized profiles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    /// Lower bound (inclusive).
    pub min: u32,
    /// Upper bound (inclusive).
    pub max: u32,
}

impl Range {
    /// Create a new `Range`.
    ///
    /// # Panics
    /// Panics if `min > max`.
    pub fn new(min: u32, max: u32) -> Self {
        assert!(min <= max, "Range: min ({min}) must not exceed max ({max})");
        Self { min, max }
    }

    /// Try to create a new `Range`, returning `Err` if `min > max`.
    pub fn try_new(min: u32, max: u32) -> Result<Self, String> {
        if min > max {
            return Err(format!("Range: min ({min}) must not exceed max ({max})"));
        }
        Ok(Self { min, max })
    }
}

/// Hardware feature flags for a printer profile.
///
/// Each flag uses `Option<bool>` for three-state semantics:
/// - `Some(true)` — printer has this feature
/// - `Some(false)` — printer definitely lacks this feature
/// - `None` — unknown (gate checks are skipped for this feature)
///
/// This design ensures backward compatibility: profiles without `features`
/// don't trigger false `printerGates` violations.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Features {
    /// Cutter hardware installed (gates `^MM` C/D modes).
    pub cutter: Option<bool>,
    /// Peel-off mechanism installed (gates `^MM` P mode).
    pub peel: Option<bool>,
    /// Rewinder hardware installed (gates `^MM` R mode).
    pub rewinder: Option<bool>,
    /// Applicator device installed (gates `^MM` A mode, `^JJ`).
    pub applicator: Option<bool>,
    /// RFID encoder installed (gates `^RF`, `^RS`, `^RW`, `^HR`, `^RL`, `^RU`, `^RB`, `^MM` F mode).
    pub rfid: Option<bool>,
    /// Real-time clock installed (gates `^ST`, `^SL`, `^KD`).
    pub rtc: Option<bool>,
    /// Battery-powered printer (gates `~JF`, `~KB`).
    pub battery: Option<bool>,
    /// Zebra BASIC Interpreter available (gates `^JI`, `~JI`, `~JQ`).
    pub zbi: Option<bool>,
    /// LCD/control panel present (gates `^KP`, `^KL`, `^JH`).
    pub lcd: Option<bool>,
    /// Kiosk/presenter mode available (gates `^MM` K mode, `^KV`, `^CN`).
    pub kiosk: Option<bool>,
}

/// Resolve a gate string (e.g., `"cutter"`, `"rfid"`) against a [`Features`] struct.
///
/// Returns:
/// - `Some(true)` if the feature is present
/// - `Some(false)` if the feature is explicitly absent
/// - `None` if the feature is unknown (gate should be skipped)
pub fn resolve_gate(features: &Features, gate: &str) -> Option<bool> {
    match gate {
        "cutter" => features.cutter,
        "peel" => features.peel,
        "rewinder" => features.rewinder,
        "applicator" => features.applicator,
        "rfid" => features.rfid,
        "rtc" => features.rtc,
        "battery" => features.battery,
        "zbi" => features.zbi,
        "lcd" => features.lcd,
        "kiosk" => features.kiosk,
        _ => None, // Unknown gate — skip
    }
}

/// Supported print method for media.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrintMethod {
    /// Direct thermal printing (heat-sensitive media, no ribbon).
    DirectThermal,
    /// Thermal transfer printing (requires ribbon).
    ThermalTransfer,
    /// Both direct thermal and thermal transfer methods supported.
    Both,
}

/// Media capability descriptors for a printer profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Media {
    /// Supported print method for this printer.
    pub print_method: Option<PrintMethod>,
    /// Valid `^MM` print mode letters this printer supports (e.g., `["T","P","C"]`).
    pub supported_modes: Option<Vec<String>>,
    /// Valid `^MN` media tracking modes this printer supports (e.g., `["N","Y","M"]`).
    pub supported_tracking: Option<Vec<String>>,
}

/// Memory and firmware information for a printer profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Memory {
    /// Available RAM in kilobytes.
    pub ram_kb: Option<u32>,
    /// Available flash storage in kilobytes.
    pub flash_kb: Option<u32>,
    /// Firmware version string (e.g., `"V60.19.15Z"`).
    pub firmware_version: Option<String>,
}

/// Load and validate a [`Profile`] from a JSON string.
///
/// The `id`, `schema_version`, and `dpi` fields are required and must be
/// present in the JSON; deserialization fails if any of them is missing.
///
/// Performs structural validation after deserialization:
/// - `id` and `schema_version` must be non-empty
/// - `dpi` must be in range 100–600
/// - `page.width_dots` and `page.height_dots` must be > 0 (if present)
/// - `speed_range.min` must be > 0, `speed_range.min` and `speed_range.max` must be <= 14, and `min <= max` (if present)
/// - `darkness_range.max` must be <= 30, and `min <= max` (if present)
/// - `memory.ram_kb` and `memory.flash_kb` must be > 0 (if present)
pub fn load_profile_from_str(s: &str) -> Result<Profile, ProfileError> {
    let profile: Profile = serde_json::from_str(s)?;

    // -- Required string field validation --
    if profile.id.trim().is_empty() {
        return Err(ProfileError::InvalidField {
            field: "id".into(),
            reason: "must not be empty".into(),
        });
    }
    if profile.schema_version.trim().is_empty() {
        return Err(ProfileError::InvalidField {
            field: "schema_version".into(),
            reason: "must not be empty".into(),
        });
    }

    // -- DPI validation --
    if profile.dpi < 100 {
        return Err(ProfileError::InvalidField {
            field: "dpi".into(),
            reason: format!("{} is below minimum supported DPI (100)", profile.dpi),
        });
    }
    if profile.dpi > 600 {
        return Err(ProfileError::InvalidField {
            field: "dpi".into(),
            reason: format!("{} exceeds maximum supported DPI (600)", profile.dpi),
        });
    }

    // -- Page dimension validation --
    if let Some(ref page) = profile.page {
        if let Some(w) = page.width_dots
            && w == 0
        {
            return Err(ProfileError::InvalidField {
                field: "page.width_dots".into(),
                reason: "must be > 0".into(),
            });
        }
        if let Some(h) = page.height_dots
            && h == 0
        {
            return Err(ProfileError::InvalidField {
                field: "page.height_dots".into(),
                reason: "must be > 0".into(),
            });
        }
    }

    // -- Speed range validation --
    if let Some(ref r) = profile.speed_range {
        if r.min > r.max {
            return Err(ProfileError::InvalidField {
                field: "speed_range".into(),
                reason: format!("min ({}) > max ({})", r.min, r.max),
            });
        }
        if r.min == 0 {
            return Err(ProfileError::InvalidField {
                field: "speed_range.min".into(),
                reason: "must be > 0".into(),
            });
        }
        if r.min > 14 {
            return Err(ProfileError::InvalidField {
                field: "speed_range.min".into(),
                reason: format!("{} exceeds maximum print speed (14 ips)", r.min),
            });
        }
        if r.max > 14 {
            return Err(ProfileError::InvalidField {
                field: "speed_range.max".into(),
                reason: format!("{} exceeds maximum print speed (14 ips)", r.max),
            });
        }
    }

    // -- Darkness range validation --
    if let Some(ref r) = profile.darkness_range {
        if r.min > r.max {
            return Err(ProfileError::InvalidField {
                field: "darkness_range".into(),
                reason: format!("min ({}) > max ({})", r.min, r.max),
            });
        }
        if r.max > 30 {
            return Err(ProfileError::InvalidField {
                field: "darkness_range.max".into(),
                reason: format!("{} exceeds maximum darkness (30)", r.max),
            });
        }
    }

    // -- Memory validation --
    if let Some(ref mem) = profile.memory {
        if let Some(ram) = mem.ram_kb
            && ram == 0
        {
            return Err(ProfileError::InvalidField {
                field: "memory.ram_kb".into(),
                reason: "must be > 0".into(),
            });
        }
        if let Some(flash) = mem.flash_kb
            && flash == 0
        {
            return Err(ProfileError::InvalidField {
                field: "memory.flash_kb".into(),
                reason: "must be > 0".into(),
            });
        }
    }

    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_valid_profile() {
        let json = r#"{
            "id": "test",
            "schema_version": "1.0.0",
            "dpi": 203,
            "page": { "width_dots": 812, "height_dots": 1218 },
            "speed_range": { "min": 2, "max": 8 },
            "darkness_range": { "min": 0, "max": 30 },
            "features": { "cutter": true, "rfid": false },
            "media": { "print_method": "direct_thermal", "supported_modes": ["T","P","C"] },
            "memory": { "ram_kb": 512, "flash_kb": 65536 }
        }"#;
        let p = load_profile_from_str(json).unwrap();
        assert_eq!(p.id, "test");
        assert_eq!(p.dpi, 203);
        assert_eq!(p.speed_range.as_ref().unwrap().min, 2);
        assert_eq!(p.speed_range.as_ref().unwrap().max, 8);
        assert_eq!(p.features.as_ref().unwrap().cutter, Some(true));
        assert_eq!(p.features.as_ref().unwrap().rfid, Some(false));
        assert_eq!(
            p.media.as_ref().unwrap().print_method,
            Some(PrintMethod::DirectThermal)
        );
        assert_eq!(p.memory.as_ref().unwrap().ram_kb, Some(512));
    }

    #[test]
    fn load_minimal_profile() {
        let json = r#"{ "id": "minimal", "schema_version": "1.0.0", "dpi": 300 }"#;
        let p = load_profile_from_str(json).unwrap();
        assert_eq!(p.dpi, 300);
        assert!(p.page.is_none());
        assert!(p.speed_range.is_none());
        assert!(p.features.is_none());
        assert!(p.media.is_none());
        assert!(p.memory.is_none());
    }

    #[test]
    fn load_invalid_speed_range() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "speed_range": { "min": 10, "max": 5 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("speed_range"),
            "error should mention speed_range: {}",
            err
        );
    }

    #[test]
    fn load_invalid_darkness_range() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "darkness_range": { "min": 20, "max": 10 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("darkness_range"),
            "error should mention darkness_range: {}",
            err
        );
    }

    #[test]
    fn missing_required_field_rejected() {
        // Missing id — should fail deserialization
        let err = load_profile_from_str(r#"{ "dpi": 203 }"#);
        assert!(err.is_err(), "missing id should fail deserialization");
        // Missing schema_version
        let err2 = load_profile_from_str(r#"{ "id": "test", "dpi": 203 }"#);
        assert!(
            err2.is_err(),
            "missing schema_version should fail deserialization"
        );
        // Missing dpi
        let err3 = load_profile_from_str(r#"{ "id": "test", "schema_version": "1.0.0" }"#);
        assert!(err3.is_err(), "missing dpi should fail deserialization");
    }

    #[test]
    fn empty_id_rejected() {
        let json = r#"{ "id": "", "schema_version": "1.0.0", "dpi": 203 }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("id"),
            "error should mention id: {err}"
        );
    }

    #[test]
    fn empty_schema_version_rejected() {
        let json = r#"{ "id": "test", "schema_version": "", "dpi": 203 }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("schema_version"),
            "error should mention schema_version: {err}"
        );
    }

    #[test]
    fn profile_equality() {
        let a = Profile {
            id: "test".into(),
            schema_version: "1.0.0".into(),
            dpi: 203,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: None,
            media: None,
            memory: None,
        };
        let b = Profile {
            id: "test".into(),
            schema_version: "1.0.0".into(),
            dpi: 203,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: None,
            media: None,
            memory: None,
        };
        let c = Profile {
            id: "test".into(),
            schema_version: "1.0.0".into(),
            dpi: 300,
            page: None,
            speed_range: None,
            darkness_range: None,
            features: None,
            media: None,
            memory: None,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn resolve_gate_known() {
        let f = Features {
            cutter: Some(true),
            rfid: Some(false),
            ..Default::default()
        };
        assert_eq!(resolve_gate(&f, "cutter"), Some(true));
        assert_eq!(resolve_gate(&f, "rfid"), Some(false));
        assert_eq!(resolve_gate(&f, "peel"), None);
        assert_eq!(resolve_gate(&f, "unknown"), None);
    }

    #[test]
    fn features_default_all_none() {
        let f = Features::default();
        assert!(f.cutter.is_none());
        assert!(f.rfid.is_none());
        assert!(f.rtc.is_none());
    }

    #[test]
    fn profile_serde_round_trip() {
        let p = Profile {
            id: "test-rt".into(),
            schema_version: "1.1.0".into(),
            dpi: 203,
            page: Some(Page {
                width_dots: Some(812),
                height_dots: Some(1218),
            }),
            speed_range: Some(Range { min: 2, max: 8 }),
            darkness_range: Some(Range { min: 0, max: 30 }),
            features: Some(Features {
                rfid: Some(true),
                cutter: Some(false),
                ..Default::default()
            }),
            media: Some(Media {
                print_method: Some(PrintMethod::DirectThermal),
                supported_modes: Some(vec!["T".into()]),
                supported_tracking: None,
            }),
            memory: Some(Memory {
                ram_kb: Some(32768),
                flash_kb: Some(65536),
                firmware_version: None,
            }),
        };
        let json = serde_json::to_string(&p).unwrap();
        let p2: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, p2);
    }

    #[test]
    fn resolve_gate_all_known_gates() {
        let features = Features {
            cutter: Some(true),
            peel: Some(false),
            rewinder: Some(true),
            applicator: Some(false),
            rfid: Some(true),
            rtc: Some(false),
            battery: Some(true),
            zbi: Some(false),
            lcd: Some(true),
            kiosk: Some(false),
        };
        assert_eq!(resolve_gate(&features, "cutter"), Some(true));
        assert_eq!(resolve_gate(&features, "peel"), Some(false));
        assert_eq!(resolve_gate(&features, "rewinder"), Some(true));
        assert_eq!(resolve_gate(&features, "applicator"), Some(false));
        assert_eq!(resolve_gate(&features, "rfid"), Some(true));
        assert_eq!(resolve_gate(&features, "rtc"), Some(false));
        assert_eq!(resolve_gate(&features, "battery"), Some(true));
        assert_eq!(resolve_gate(&features, "zbi"), Some(false));
        assert_eq!(resolve_gate(&features, "lcd"), Some(true));
        assert_eq!(resolve_gate(&features, "kiosk"), Some(false));
        // Unknown gate
        assert_eq!(resolve_gate(&features, "unknown_feature"), None);
    }

    #[test]
    fn load_profile_malformed_json() {
        let err = load_profile_from_str("not json at all");
        assert!(err.is_err(), "malformed JSON should return error");
    }

    #[test]
    fn load_profile_equal_min_max_range() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "speed_range": { "min": 5, "max": 5 } }"#;
        let p = load_profile_from_str(json).unwrap();
        assert_eq!(p.speed_range.as_ref().unwrap().min, 5);
        assert_eq!(p.speed_range.as_ref().unwrap().max, 5);
    }

    #[test]
    fn dpi_below_minimum_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 50 }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("below minimum supported DPI"),
            "error should mention DPI minimum: {err}"
        );
    }

    #[test]
    fn dpi_too_high_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 9999 }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("exceeds maximum supported DPI"),
            "error should mention DPI limit: {err}"
        );
    }

    #[test]
    fn page_width_zero_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "page": { "width_dots": 0 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("page.width_dots"),
            "error should mention page.width_dots: {err}"
        );
    }

    #[test]
    fn page_height_zero_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "page": { "height_dots": 0 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("page.height_dots"),
            "error should mention page.height_dots: {err}"
        );
    }

    #[test]
    fn speed_min_zero_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "speed_range": { "min": 0, "max": 5 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("speed_range.min"),
            "error should mention speed_range.min: {err}"
        );
    }

    #[test]
    fn speed_max_too_high_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "speed_range": { "min": 1, "max": 20 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("speed_range.max"),
            "error should mention speed_range.max: {err}"
        );
    }

    #[test]
    fn darkness_max_too_high_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "darkness_range": { "min": 0, "max": 50 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("darkness_range.max"),
            "error should mention darkness_range.max: {err}"
        );
    }

    #[test]
    fn memory_ram_zero_rejected() {
        let json =
            r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "memory": { "ram_kb": 0 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("memory.ram_kb"),
            "error should mention memory.ram_kb: {err}"
        );
    }

    #[test]
    fn memory_flash_zero_rejected() {
        let json = r#"{ "id": "test", "schema_version": "1.0.0", "dpi": 203, "memory": { "flash_kb": 0 } }"#;
        let err = load_profile_from_str(json).unwrap_err();
        assert!(
            err.to_string().contains("memory.flash_kb"),
            "error should mention memory.flash_kb: {err}"
        );
    }

    #[test]
    fn valid_edge_cases_accepted() {
        let json = r#"{
            "id": "test",
            "schema_version": "1.0.0",
            "dpi": 600,
            "speed_range": { "min": 1, "max": 14 },
            "darkness_range": { "min": 0, "max": 30 }
        }"#;
        let p = load_profile_from_str(json).unwrap();
        assert_eq!(p.dpi, 600);
        assert_eq!(p.speed_range.as_ref().unwrap().min, 1);
        assert_eq!(p.speed_range.as_ref().unwrap().max, 14);
        assert_eq!(p.darkness_range.as_ref().unwrap().min, 0);
        assert_eq!(p.darkness_range.as_ref().unwrap().max, 30);
    }
}
