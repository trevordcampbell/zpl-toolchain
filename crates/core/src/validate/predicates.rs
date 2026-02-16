use crate::grammar::ast::{ArgSlot, Presence};
use std::collections::HashSet;
use zpl_toolchain_profile::Profile;
use zpl_toolchain_spec_tables::EnumValue;

/// Check if any of the pipe-separated targets are present in a pre-built set (O(1) per target).
pub(super) fn any_target_in_set(targets: &str, seen: &HashSet<&str>) -> bool {
    targets
        .split('|')
        .map(str::trim)
        .any(|target| !target.is_empty() && seen.contains(target))
}

/// Check if an enum value list contains a given value.
pub(super) fn enum_contains(values: &[EnumValue], target: &str) -> bool {
    values.iter().any(|e| match e {
        EnumValue::Simple(s) => s == target,
        EnumValue::Object { value, .. } => value == target,
    })
}

/// Compare Zebra firmware version strings (e.g. V60.19.15Z, X60.14.3).
/// Returns true if fw >= min_ver when both parse, false otherwise.
#[allow(dead_code)]
pub(crate) fn firmware_version_gte(fw: &str, min_ver: &str) -> bool {
    fn parse_version(s: &str) -> Option<(u32, u32)> {
        let s = s
            .strip_prefix('V')
            .or_else(|| s.strip_prefix('X'))
            .unwrap_or(s);
        let mut parts = s.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = parts
            .next()
            .and_then(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u32>()
                    .ok()
            })
            .unwrap_or(0);
        Some((major, minor))
    }
    match (parse_version(fw), parse_version(min_ver)) {
        (Some((fw_maj, fw_min)), Some((min_maj, min_min))) => {
            (fw_maj, fw_min) >= (min_maj, min_min)
        }
        _ => false,
    }
}

/// Profile predicate support for note when: expressions.
/// When profile is None, all profile predicates return false (conservative).
#[allow(dead_code)]
pub(crate) fn profile_predicate_matches(predicate: &str, profile: Option<&Profile>) -> bool {
    let Some(p) = profile else {
        return false;
    };
    if let Some(rest) = predicate.strip_prefix("profile:id:") {
        let accepted: Vec<&str> = rest
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        return accepted.is_empty() || accepted.iter().any(|id| *id == p.id);
    }
    if let Some(rest) = predicate.strip_prefix("profile:dpi:") {
        let accepted: Vec<&str> = rest
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        return accepted
            .iter()
            .any(|s| s.parse::<u32>().ok() == Some(p.dpi));
    }
    if let Some(rest) = predicate.strip_prefix("profile:feature:") {
        let gates: Vec<&str> = rest
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(features) = &p.features {
            return gates
                .iter()
                .any(|g| zpl_toolchain_profile::resolve_gate(features, g) == Some(true));
        }
        return false;
    }
    if let Some(rest) = predicate.strip_prefix("profile:featureMissing:") {
        let gates: Vec<&str> = rest
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(features) = &p.features {
            return gates
                .iter()
                .any(|g| zpl_toolchain_profile::resolve_gate(features, g) == Some(false));
        }
        return false;
    }
    if let Some(prefix) = predicate.strip_prefix("profile:firmware:") {
        return p
            .memory
            .as_ref()
            .and_then(|m| m.firmware_version.as_deref())
            .map(|fw| fw.starts_with(prefix.trim()))
            .unwrap_or(false);
    }
    if let Some(rest) = predicate.strip_prefix("profile:firmwareGte:") {
        let min_ver = rest.trim();
        let fw = p
            .memory
            .as_ref()
            .and_then(|m| m.firmware_version.as_deref());
        return fw
            .map(|v| firmware_version_gte(v, min_ver))
            .unwrap_or(false);
    }
    // profile:model: is an alias for profile:id: (profile id often encodes model)
    if let Some(rest) = predicate.strip_prefix("profile:model:") {
        let accepted: Vec<&str> = rest
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        return accepted.is_empty() || accepted.iter().any(|m| p.id.contains(m) || p.id == *m);
    }
    false
}

// Very small predicate support for conditionalRange / roundingPolicyWhen
// MVP: support keys like "arg:keyIsValue:X" or "arg:keyPresent" or "arg:keyEmpty"
pub(super) fn predicate_matches(when: &str, args: &[ArgSlot]) -> bool {
    if let Some(rest) = when.strip_prefix("arg:") {
        if let Some((k, rhs)) = rest.split_once("IsValue:") {
            let accepted: Vec<&str> = rhs.split('|').collect();
            return args.iter().any(|a| {
                a.key.as_deref() == Some(k)
                    && a.value
                        .as_deref()
                        .is_some_and(|value| accepted.contains(&value))
            });
        }
        if let Some(k) = rest.strip_suffix("Present") {
            return args
                .iter()
                .any(|a| a.key.as_deref() == Some(k) && a.presence == Presence::Value);
        }
        if let Some(k) = rest.strip_suffix("Empty") {
            return args
                .iter()
                .any(|a| a.key.as_deref() == Some(k) && a.presence == Presence::Empty);
        }
    }
    false
}

pub(super) fn evaluate_note_when_expression(
    expression: &str,
    args: &[ArgSlot],
    label_codes: &HashSet<&str>,
    profile: Option<&Profile>,
) -> bool {
    expression.split("||").any(|disjunction| {
        disjunction.split("&&").all(|term| {
            let token = term.trim();
            if token.is_empty() {
                return false;
            }
            let (negated, predicate) = if let Some(rest) = token.strip_prefix('!') {
                (true, rest.trim())
            } else {
                (false, token)
            };
            let mut matches = if predicate.starts_with("arg:") {
                predicate_matches(predicate, args)
            } else if let Some(targets) = predicate.strip_prefix("label:has:") {
                any_target_in_set(targets, label_codes)
            } else if let Some(targets) = predicate.strip_prefix("label:missing:") {
                !any_target_in_set(targets, label_codes)
            } else if predicate.starts_with("profile:") {
                profile_predicate_matches(predicate, profile)
            } else {
                false
            };
            if negated {
                matches = !matches;
            }
            matches
        })
    })
}
