use std::collections::HashMap;
use std::sync::OnceLock;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::ComparisonOp;

/// Type alias for profile field accessor functions.
type ProfileFieldFn = fn(&Profile) -> Option<f64>;

/// Declarative registry of all numeric profile fields.
///
/// Adding a new numeric profile field requires adding one entry here.
/// The `all_profile_constraint_fields_are_resolvable` test ensures
/// coverage of all fields referenced by `profileConstraint` in specs.
const PROFILE_FIELD_REGISTRY: &[(&str, ProfileFieldFn)] = &[
    ("dpi", |p| Some(p.dpi as f64)),
    ("page.width_dots", |p| {
        p.page
            .as_ref()
            .and_then(|pg| pg.width_dots.map(|v| v as f64))
    }),
    ("page.height_dots", |p| {
        p.page
            .as_ref()
            .and_then(|pg| pg.height_dots.map(|v| v as f64))
    }),
    ("speed_range.min", |p| {
        p.speed_range.as_ref().map(|r| r.min as f64)
    }),
    ("speed_range.max", |p| {
        p.speed_range.as_ref().map(|r| r.max as f64)
    }),
    ("darkness_range.min", |p| {
        p.darkness_range.as_ref().map(|r| r.min as f64)
    }),
    ("darkness_range.max", |p| {
        p.darkness_range.as_ref().map(|r| r.max as f64)
    }),
    ("memory.ram_kb", |p| {
        p.memory.as_ref().and_then(|m| m.ram_kb.map(|v| v as f64))
    }),
    ("memory.flash_kb", |p| {
        p.memory.as_ref().and_then(|m| m.flash_kb.map(|v| v as f64))
    }),
];

/// Cached lookup map from field path to accessor function.
static PROFILE_FIELD_MAP: OnceLock<HashMap<&'static str, ProfileFieldFn>> = OnceLock::new();

fn profile_field_map() -> &'static HashMap<&'static str, ProfileFieldFn> {
    PROFILE_FIELD_MAP.get_or_init(|| PROFILE_FIELD_REGISTRY.iter().copied().collect())
}

/// Resolve a profile field by dotted path (e.g., "page.width_dots").
///
/// Returns the numeric value of the named profile field, or `None` if the
/// field path is unrecognized or the corresponding value is not set in the
/// profile. Used by the validator for `profileConstraint` checks and
/// exposed publicly so that tests can verify coverage of all constraint
/// field paths referenced in command specs.
pub fn resolve_profile_field(profile: &Profile, field: &str) -> Option<f64> {
    profile_field_map().get(field).and_then(|f| f(profile))
}

/// Check a profile constraint operator.
///
/// Returns `false` (constraint violated) for non-finite values (NaN, infinity)
/// to prevent them from silently passing validation.
///
/// The `Eq` tolerance of 0.5 is intentional: all profile fields (DPI, dots,
/// speed, darkness, KB) are integer values cast to `f64`, so two values
/// represent the same integer exactly when they round to the same whole
/// number — i.e., when their difference is less than 0.5.  This is far more
/// robust than `f64::EPSILON` (~2.2e-16), which is the unit-of-least-precision
/// near 1.0 and is neither a correct nor a general-purpose equality tolerance.
pub(super) fn check_profile_op(value: f64, op: &ComparisonOp, limit: f64) -> bool {
    if !value.is_finite() || !limit.is_finite() {
        return false;
    }
    match op {
        ComparisonOp::Lte => value <= limit,
        ComparisonOp::Gte => value >= limit,
        ComparisonOp::Lt => value < limit,
        ComparisonOp::Gt => value > limit,
        ComparisonOp::Eq => (value - limit).abs() < 0.5,
    }
}
