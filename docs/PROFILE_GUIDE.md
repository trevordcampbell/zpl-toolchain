# Profile System Guide

> Comprehensive reference for ZPL toolchain printer profiles — data-driven validation
> tied to real printer hardware.

---

## 1. Introduction

A **profile** is a JSON file that describes a specific Zebra label printer's capabilities
and constraints: resolution, page dimensions, speed/darkness ranges, installed hardware
features, media capabilities, and memory.

**Why profiles exist.** The ZPL toolchain validates ZPL II source against per-command
specs. Many validation bounds are not universal — they depend on the printer model. For
example, `^PW` (print width) should not exceed the printhead width, and `^MM C` (cutter
mode) should only be used when a cutter is installed. Profiles make these checks
**data-driven**: spec authors annotate args with `profileConstraint` for numeric bounds
and `printerGates` for hardware capability gating, and the validator resolves values from
the loaded profile at lint time.

**How they're used.** Pass a profile to the CLI with `--profile`:

```bash
zpl lint label.zpl --profile profiles/zebra-generic-203.json
```

Without `--profile`, profile-dependent checks are skipped entirely — no false positives.

---

## 2. Profile Schema Reference

The canonical profile schema lives at `spec/schema/profile.schema.jsonc`. The spec-compiler
cross-validates that every `profileConstraint.field` reference in command specs resolves
to a field defined in this schema.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique identifier (e.g., `"zebra-generic-203"`) |
| `schema_version` | string | yes | Schema version (currently `"1.1.0"`) |
| `dpi` | int | yes | Print resolution in dots per inch (100–600) |
| `page.width_dots` | int | no | Maximum printhead width in dots |
| `page.height_dots` | int | no | Maximum label length in dots |
| `speed_range.min` | int | no | Minimum print speed (1–14 ips) |
| `speed_range.max` | int | no | Maximum print speed (1–14 ips) |
| `darkness_range.min` | int | no | Minimum darkness setting (0–30) |
| `darkness_range.max` | int | no | Maximum darkness setting (0–30) |
| `memory.ram_kb` | int | no | RAM in kilobytes |
| `memory.flash_kb` | int | no | Flash storage in kilobytes |
| `memory.firmware_version` | string | no | Firmware version string |
| `features.*` | bool | no | Hardware feature flags (see §3) |
| `media.print_method` | string | no | `"direct_thermal"`, `"thermal_transfer"`, or `"both"` |
| `media.supported_modes` | string[] | no | Valid `^MM` mode letters |
| `media.supported_tracking` | string[] | no | Valid `^MN` tracking modes |

**Structural invariants** enforced on load (`load_profile_from_str`), which returns `ProfileError` on failure (`InvalidJson` for parse errors, `InvalidField` for constraint violations):
- `id` must be non-empty
- `schema_version` must be non-empty
- `dpi` must be in the range 100–600
- `page.width_dots` and `page.height_dots` must be positive (if present)
- `speed_range.min` and `speed_range.max` must be in the range 1–14, and `min <= max` (if present)
- `darkness_range.min` and `darkness_range.max` must be in the range 0–30, and `min <= max` (if present)
- `memory.ram_kb` and `memory.flash_kb` must be positive (if present)

All fields except `id`, `schema_version`, and `dpi` are optional. Missing fields cause
the corresponding checks to be skipped — never to fail.

---

## 3. Features (printerGates)

The `features` object contains boolean flags for hardware capabilities. These are
evaluated by the `printerGates` mechanism, not by `profileConstraint`.

| Feature | Description |
|---------|-------------|
| `cutter` | Cutter hardware (gates `^MM` C/D modes) |
| `peel` | Peel-off mechanism (gates `^MM` P mode) |
| `rewinder` | Rewinder hardware (gates `^MM` R mode) |
| `applicator` | Applicator device (gates `^MM` A mode, `^JJ`) |
| `rfid` | RFID encoder (gates `^RF`, `^RS`, `^RW`, `^HR`, `^RL`, `^RU`, `^RB`, `^MM` F mode) |
| `rtc` | Real-time clock (gates `^ST`, `^SL`, `^KD`) |
| `battery` | Battery-powered printer (gates `~JF`, `~KB`) |
| `zbi` | Zebra BASIC Interpreter (gates `^JI`, `~JI`, `~JQ`) |
| `lcd` | LCD/control panel (gates `^KP`, `^KL`, `^JH`) |
| `kiosk` | Kiosk/presenter mode (gates `^MM` K mode, `^KV`, `^CN`) |

### Three-state semantics

Each feature uses `Option<bool>` in the Rust struct, which maps to three JSON states:

| JSON value | Meaning | Behavior |
|------------|---------|----------|
| `true` | Printer has this feature | Gate passes |
| `false` | Printer lacks this feature | Gate fails → emits ZPL1402 |
| `null` / absent | Unknown | Gate check skipped (no false positives) |

This design ensures backward compatibility: profiles written before `features` was added
don't trigger spurious gate violations.

---

## 4. How profileConstraint Works

Spec authors annotate numeric args in command JSONC files with a `profileConstraint`
object. This ties the arg's validated range to a value in the loaded profile.

### Spec-side syntax

```jsonc
{
  "name": "print_width",
  "type": "int",
  "range": [1, 32000],
  "profileConstraint": {
    "field": "page.width_dots",
    "op": "lte"
  }
}
```

### Runtime behavior

1. The validator parses the arg value from the ZPL source.
2. It resolves the dotted `field` path (e.g., `"page.width_dots"`) against the loaded
   profile using `resolve_profile_field()`.
3. If the profile field is present, it evaluates the comparison operator.
4. If the check fails, it emits **ZPL1401** with a message like
   *"print_width value 1000 exceeds profile limit page.width_dots (832)"*.
5. If the profile field is absent (`None`), the check is silently skipped.

### Supported operators

| Operator | Meaning | Typical use |
|----------|---------|-------------|
| `lte` | value ≤ profile field | Width, height (must not exceed max) |
| `gte` | value ≥ profile field | Speed min, darkness min |
| `lt` | value < profile field | Strict upper bound |
| `gt` | value > profile field | Strict lower bound |
| `eq` | value = profile field | Exact match |

### Safety net

The test `all_profile_constraint_fields_are_resolvable` loads every command spec, extracts
all `profileConstraint.field` values, and verifies each one resolves against the profile
struct. This catches field path typos at test time.

---

## 5. How printerGates Works

`printerGates` gates entire commands or individual enum values behind hardware features.

### Command-level gates

Add `"printerGates"` to the command object in a JSONC spec file:

```jsonc
{
  "codes": ["^RF"],
  "name": "Read or Write RFID Format",
  "printerGates": ["rfid"],
  "args": [ ... ]
}
```

When a profile is loaded and `features.rfid` is `false`, any use of `^RF` emits
**ZPL1402** (error severity).

### Enum value-level gates

For commands where only *certain values* require hardware, use the object-style enum
syntax:

```jsonc
"enum": [
  "T",
  "P",
  { "value": "C", "printerGates": ["cutter"] },
  { "value": "K", "printerGates": ["kiosk"] }
]
```

Enum value gates emit **ZPL1402** at warning severity — the firmware silently ignores
unsupported modes, so an error would be too strict.

### Gate resolution

The validator looks up each gate name in the profile's `features` object:
- `Some(true)` → gate passes, no diagnostic
- `Some(false)` → gate fails, emit ZPL1402
- `None` / features absent → skip check

---

## 6. DPI-Dependent Defaults

Some args have defaults that vary by printer resolution. The `defaultByDpi` field
alongside `default` supports this:

```jsonc
{
  "name": "magnification",
  "type": "int",
  "default": 2,
  "defaultByDpi": {
    "150": 1,
    "200": 2,
    "203": 2,
    "300": 3,
    "600": 6
  }
}
```

When a profile is loaded, the validator looks up the profile's `dpi` value in the
`defaultByDpi` map. If a match is found, that value is used as the effective default.
If no match exists, it falls back to the static `default` value.

This is primarily used for barcode magnification (`^BQ`, `^B0`) where the Zebra
Programming Guide specifies DPI-dependent defaults.

---

## 7. Creating a Custom Profile

To create a profile for a specific printer configuration:

1. Copy an existing profile from `profiles/` as a starting point.
2. Set `id` to a unique, descriptive name.
3. Keep `schema_version` at `"1.1.0"`.
4. Fill in the fields relevant to your printer.
5. For `features`, set known capabilities to `true`/`false`. Leave unknown ones as
   `null` or omit them entirely.

### Example: ZT410 with cutter and RFID

```json
{
  "id": "zt410-cutter-rfid",
  "schema_version": "1.1.0",
  "dpi": 203,
  "page": { "width_dots": 832, "height_dots": 6400 },
  "speed_range": { "min": 2, "max": 14 },
  "darkness_range": { "min": 0, "max": 30 },
  "features": {
    "cutter": true,
    "peel": false,
    "rewinder": false,
    "applicator": false,
    "rfid": true,
    "rtc": true,
    "battery": false,
    "zbi": true,
    "lcd": true,
    "kiosk": false
  },
  "media": {
    "print_method": "both",
    "supported_modes": ["T", "C", "D"],
    "supported_tracking": ["N", "Y", "W", "M"]
  },
  "memory": {
    "ram_kb": 262144,
    "flash_kb": 65536,
    "firmware_version": "V60.19.15Z"
  }
}
```

### Tips

- **Minimal profiles work.** Only `id`, `schema_version`, and `dpi` are required. Start
  small and add fields as needed.
- **Use `false` intentionally.** Setting a feature to `false` means the validator *will*
  flag commands that require it. Only set `false` when you're certain the printer lacks
  the feature.
- **Test your profile.** Run `zpl lint` with your profile against real label files to
  verify constraints match your hardware.

---

## 8. Shipped Profiles

### Generic (conservative baselines)

| File | DPI | Width | Speed | Description |
|------|-----|-------|-------|-------------|
| `zebra-generic-203.json` | 203 | 812 dots | 2–8 ips | Generic 203 dpi baseline — direct thermal, no optional features |
| `zebra-generic-300.json` | 300 | 1218 dots | 2–6 ips | Generic 300 dpi baseline — direct thermal, no optional features |

### Desktop Printers

| File | DPI | Width | Speed | RAM | Flash | Description |
|------|-----|-------|-------|-----|-------|-------------|
| `zebra-gk420t-203.json` | 203 | 832 dots | 2–5 ips | 8 MB | 4 MB | Legacy desktop workhorse — both TT/DT, no ZBI/LCD |
| `zebra-zd420-203.json` | 203 | 832 dots | 2–6 ips | 256 MB | 512 MB | Mid-range desktop — both TT/DT, ZBI |
| `zebra-zd620-203.json` | 203 | 832 dots | 2–8 ips | 256 MB | 512 MB | Performance desktop — both TT/DT, ZBI, LCD |
| `zebra-zd621-203.json` | 203 | 832 dots | 2–8 ips | 256 MB | 512 MB | Current-gen premium desktop — both TT/DT, ZBI, LCD |

### Industrial Printers

| File | DPI | Width | Speed | RAM | Flash | Description |
|------|-----|-------|-------|-----|-------|-------------|
| `zebra-zt231-203.json` | 203 | 832 dots | 2–12 ips | 256 MB | 256 MB | Value industrial — both TT/DT, ZBI, LCD, 4.3" touchscreen |
| `zebra-zt410-203.json` | 203 | 832 dots | 2–14 ips | 256 MB | 512 MB | Previous-gen industrial — both TT/DT, ZBI, LCD, RTC |
| `zebra-zt411-203.json` | 203 | 832 dots | 2–14 ips | 256 MB | 512 MB | Current-gen industrial workhorse — both TT/DT, ZBI, LCD, RTC |
| `zebra-zt610-300.json` | 300 | 1248 dots | 2–12 ips | 512 MB | 512 MB | Wide-format industrial (6.6") — both TT/DT, ZBI, LCD, RTC |

### Mobile Printers

| File | DPI | Width | Speed | RAM | Flash | Description |
|------|-----|-------|-------|-----|-------|-------------|
| `zebra-zq520-203.json` | 203 | 832 dots | 2–5 ips | 256 MB | 512 MB | Rugged mobile — direct thermal, battery, ZBI, LCD, RTC |

All printer-specific profiles represent **base model** configurations (no optional
accessories like cutter, peeler, or RFID). To model a specific SKU with optional
features, copy a profile and set the relevant `features` flags to `true`.

---

## 9. Diagnostic Codes

Profile-related diagnostics use codes ZPL1401 (profileConstraint violations), ZPL1402 (printerGate failures), and ZPL1403 (media capability mismatches). Use `zpl explain ZPL1401` (or any code) for detailed explanations and fix guidance.

For the full diagnostic reference including these codes, severity levels, structured context fields, and all other diagnostic codes, see [`DIAGNOSTIC_CODES.md`](DIAGNOSTIC_CODES.md).

---

## See Also

- `spec/schema/profile.schema.jsonc` — canonical profile schema
- `crates/profile/README.md` — Rust crate documentation
- `docs/DIAGNOSTIC_CODES.md` — full diagnostic code reference
- `docs/public/schema/SPEC_AUTHORING.md` — spec authoring guide (covers `profileConstraint` and `printerGates` syntax)
