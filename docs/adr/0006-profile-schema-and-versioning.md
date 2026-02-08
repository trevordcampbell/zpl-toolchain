# ADR 0006: Profile schema and versioning

## Status
Accepted

## Context
Validator checks depend on printer capabilities (page width, dpi, media). Profiles need to evolve without breaking older validators.

## Decision
- Define a JSON profile schema in `crates/profile`, versioned with a `schema_version` field.
- Start with `dpi` and `page.width_dots`; add media, timing, and defaults later.
- Validators should treat unknown fields as forward-compatible.

## Implementation (2026-02)
Fully implemented. The profile schema at `spec/schema/profile.schema.jsonc` (v1.1.0) defines:
- Core fields: `id`, `schema_version`, `dpi`, `page` (width/height in dots)
- Ranges: `speed_range`, `darkness_range` (with `min <= max` invariant)
- Features: 10 `printerGates` (`cutter`, `peel`, `rewinder`, `applicator`, `rfid`, `rtc`, `battery`, `zbi`, `lcd`, `kiosk`)
- Media: `print_method` (typed enum), `supported_modes`, `supported_tracking`
- Memory/firmware: `ram_kb`, `flash_kb`, `firmware_version`
- DPI-dependent defaults via `defaultByDpi` on spec args

11 shipped profiles covering popular Zebra printers (desktop, industrial, mobile) plus two generic profiles — see `profiles/` and `docs/PROFILE_GUIDE.md`.

## Consequences
- Enables contextual checks across devices; supports growth over time.
- Requires careful defaulting and clear version bumps for breaking changes.

