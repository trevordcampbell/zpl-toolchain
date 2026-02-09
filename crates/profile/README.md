# zpl_toolchain_profile

Printer profile crate for the ZPL toolchain. Defines the `Profile` struct and loading utilities that drive data-driven validation.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

> **Full guide:** See [`docs/PROFILE_GUIDE.md`](../../docs/PROFILE_GUIDE.md) for the complete profile system reference — schema, printerGates semantics, DPI-dependent defaults, and custom profile creation.

## Profile Schema

Profiles describe a printer's capabilities: resolution, page dimensions, speed/darkness ranges, hardware features, media capabilities, and memory/firmware info. The canonical schema lives in `spec/schema/profile.schema.jsonc` and is cross-validated by the spec-compiler.

```json
{
  "id": "zebra-generic-203",
  "schema_version": "1.1.0",
  "dpi": 203,
  "page": { "width_dots": 812, "height_dots": 1218 },
  "speed_range": { "min": 2, "max": 8 },
  "darkness_range": { "min": 0, "max": 30 },
  "features": {
    "cutter": false, "peel": false, "rewinder": false,
    "applicator": false, "rfid": false, "rtc": false,
    "battery": false, "zbi": false, "lcd": false, "kiosk": false
  },
  "media": {
    "print_method": "direct_thermal",
    "supported_modes": ["T"],
    "supported_tracking": ["N", "Y", "W", "M"]
  },
  "memory": {
    "ram_kb": 32768,
    "flash_kb": 65536
  }
}
```

## Structs
- **`Profile`** — top-level printer profile with required `id`, `schema_version`, `dpi` and optional `page`, `speed_range`, `darkness_range`, `features`, `media`, `memory`
- **`Page`** — page/label dimension constraints (`width_dots`, `height_dots` as `Option<u32>`)
- **`Range`** — min/max range for numeric capabilities (`min: u32`, `max: u32`); validated that `min <= max` on load. Constructors: `Range::new(min, max)` (panics if `min > max`) and `Range::try_new(min, max) -> Option<Range>` (returns `None` if invalid)
- **`Features`** — hardware feature flags for `printerGates` enforcement (`cutter`, `peel`, `rewinder`, `applicator`, `rfid`, `rtc`, `battery`, `zbi`, `lcd`, `kiosk` as `Option<bool>`); three-state semantics: `true` = has feature, `false` = lacks feature (triggers ZPL1402), `None` = unknown (gate skipped)
- **`Media`** — media capability descriptors (`print_method`, `supported_modes`, `supported_tracking` as `Option`)
- **`Memory`** — memory and firmware info (`ram_kb`, `flash_kb` as `Option<u32>`, `firmware_version` as `Option<String>`)

Derives: `Debug`, `Clone`, `Serialize`, `Deserialize`, `Default`, `PartialEq`, `Eq` (Profile, Page, Features, Media, Memory); `Range` derives all except `Default`.

## Gate Resolution
The validator resolves `printerGates` against `Profile.features`:
- Feature `true` → gate passes
- Feature `false` → gate fails → ZPL1402
- Feature `None` / `features` absent → gate check skipped (no false positives)

Command-level gates emit errors; enum value-level gates emit warnings.

## Usage
- CLI `--profile profiles/zebra-generic-203.json` loads a profile and enables `profileConstraint` checks (e.g., `^PW` width ≤ `page.width_dots`, `~SD` darkness ≤ `darkness_range.max`) and `printerGates` enforcement.
- `load_profile_from_str()` deserializes and validates structural invariants, returning `ProfileError` on failure. `ProfileError` has two variants: `InvalidJson` (serde parse failure) and `InvalidField` (structural invariant violation such as `min > max`, empty `id`, DPI out of 100–600, non-positive page dimensions, speed outside 1–14, darkness outside 0–30, or non-positive memory).
- `resolve_profile_field()` in the validator maps dotted paths (e.g., `"page.width_dots"`) to profile values.
- The `all_profile_constraint_fields_are_resolvable` test ensures the resolver covers every field referenced in command specs.

## Shipped Profiles
- `profiles/zebra-generic-203.json` — 203 dpi generic (4" printhead, 812×1218 dots, direct thermal, no cutter/RFID)
- `profiles/zebra-generic-300.json` — 300 dpi generic (4" printhead, 1218×1800 dots, direct thermal, no cutter/RFID)
