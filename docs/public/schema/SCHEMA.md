# JSONC Schema (v1.1.1) Overview

This document summarizes the ZPL spec registry schema used by the spec-compiler. Author files are JSONC; the compiler strips comments and validates against `spec/schema/zpl-spec.schema.jsonc`.

## Top-level

```jsonc
{
  "version": "0.1.0",                  // content version (optional)
  "schemaVersion": "1.1.1",               // required
  "meta": { /* optional */ },              // extra metadata
  "commands": [ { /* command */ } ]        // required
}
```

## Command

```jsonc
{
  "codes": ["^BC"],                         // one or more opcodes (first is canonical)
  // or legacy: "code": "^BC"
  "arity": 6,                                // positional arity
  "raw_payload": false,                      // raw region (e.g., ^GF)
  "field_data": false,                       // field-data region (^FD/^FV)
  "signature": {                             // wire format builder
    "params": ["o","h","f","g","e","m"],
    "joiner": ",",
    "spacingPolicy": "forbid",
    "allowEmptyTrailing": true
  },
  "signatureOverrides": { /* optional per-opcode */ },
  "composites": [ {                          // optional path-like segments
    "name": "d:o.x",
    "template": "{d}:{o}.{x}",
    "exposesArgs": ["d","o","x"]
  } ],

  // Preferred rich fields
  "args": [ { /* see Arg */ } ],
  "constraints": [ { /* requires/incompatible/order/range/note/custom */ } ],

  "defaults": { /* command-level defaults */ },
  "units": "dots",                          // command-level units
  "effects": { "sets": ["state.key"] },     // cross-command state (object format only)
  "structuralRules": [ /* structural semantic rule payloads */ ],
  "fieldDataRules": {                        // barcode field data validation (optional)
    "characterSet": "0-9",                   // compact charset notation
    "exactLength": 12,                       // or minLength/maxLength
    "lengthParity": "even",                  // "even" or "odd"
    "notes": "..."                           // human-readable (informational only)
  },
  "printerGates": [ /* capability gates */ ],
  "examples": [ { "title": "...", "zpl": "..." } ],
  "docs": "...",
  "extras": { /* vendor/experimental */ }
}
```

## Arg and ArgUnion (parity skeleton)

```jsonc
// ArgUnion is either an Arg or an object with "oneOf": [ Arg, Arg, ... ]
{
  "name": "height",
  "key": "h",
  "type": "int",                               // int|float|enum|bool|string|resourceRef|char
  "unit": "dots",
  "range": [1, 32000],
  "minLength": 0, "maxLength": 8,              // for string args
  "doc": "...",
  "enum": [ "N", "Y" ],
  "optional": true,
  "presence": "emptyMeansUseDefault",          // unset|empty|value|...
  "default": 0, "defaultFrom": "^BY", "defaultFromStateKey": "barcode.height",
  "rangeWhen": [ { "when": "fontIsBitmap", "range": [0,32000] } ],
  "roundingPolicy": { "mode": "nearest" },
  "roundingPolicyWhen": [ { "when": "fontIsBitmap", "mode": "toNearestMultipleOfBaseHeight" } ],
  "resource": "graphic",
  "printerGates": [ "zd620@203" ],
  "extras": { }
}
```

## Constraints

- `requires`: other command/value must be present
- `incompatible`: cannot coexist with another command/value
- `order`: relative ordering constraint
- `range`: conditional ranges outside of args
- `note`/`custom`: documentation or tool-specific rule
- `scope`: constraint evaluation scope (`label` or `field`)

`scope` is required for `order`, `requires`, and `incompatible` constraints.
For `note` and `custom`, `scope` is optional and defaults to command scope.

Example:

```jsonc
{ "kind": "requires", "expr": "^BY", "scope": "label", "message": "^BY should precede this command" }
{ "kind": "order", "expr": "before:^FD|^FV", "scope": "field", "message": "Should precede field data" }
{ "kind": "note", "audience": "contextual", "message": "Explanatory note" } // scope optional
```

Validator enforces presence/order, numeric ranges/enums via `args`, conditional range/rounding, and profile-based gates (e.g., ^PW). More predicates and unit conversions are planned.

## Structural commands and field-data

- Structural commands (e.g., `^XA`, `^XZ`, `^FS`) typically have `arity: 0` and do not declare `signature`/`args`/`constraints`. These are still expected to have `docs` for completeness.
- Field-data commands (`^FD`/`^FV`) set `field_data: true`. They may omit `signature`/`args`/`constraints`; validators and coverage treat them specially while still expecting `docs`.
- Coverage rules account for the above: structural and field-data commands are excluded from signature/args/constraints requirements.

## Tooling notes

- Coverage: the CLI provides a human-readable and JSON summary of `generated/coverage.json`.
  - Human: `zpl coverage --coverage generated/coverage.json [--show-issues]`
  - JSON: `zpl coverage --coverage generated/coverage.json --json`
- Diagnostic explanations: `zpl explain <ID>` prints a brief description for known IDs (e.g., `ZPL1401`, `ZPL.PARSER.1202`).

## Field status notes (current usage)

- `signatureOverrides`: passed through for per-opcode tweaks; not yet consumed by parser/validator (reserved for docs and future correctness cases).
- `composites`: passed through and validated for linkage; not yet rendered in parser output; intended for documentation and advanced formatting.
- `defaults`, `units`: passed through; `units` used by `^MU` conversion in validator.
- `effects`: consumed by validator for cross-command state tracking and default resolution.
- `defaultFromStateKey`: required when `defaultFrom` is present; explicit mapping from `defaultFrom` producer -> concrete state key.
  - must be one of producer `effects.sets`
- `printerGates`: enforced via profile `features` — command-level and enum-value-level gate resolution; emits `ZPL1402` diagnostics.

## Status

- The schema (`spec/schema/zpl-spec.schema.jsonc`) implements v1.1.1 features: signatures/composites, `args`/`argUnion`, `constraints`, conditional range and rounding, enums (string/object), and examples.
- The spec-compiler validates per-command files against this schema and passes through fields into generated tables and bundles.
- The validator consumes `args` and enforces presence/order, ranges/enums, conditional range/rounding, profile constraints (`profileConstraint`), printer gates (`printerGates`), media validation, barcode `fieldDataRules` validation (ZPL2401/2402), `^MU` unit conversion, typed value-state default resolution (`defaultFrom` + `defaultFromStateKey`), device-level state tracking, and structural/semantic checks driven by `structuralRules` + `structuralRuleIndex`.

## Versioning

- Schema version (input): Derived from per-command JSONC files: the compiler reads each file’s `schemaVersion` and sets `schema_version` in `generated/parser_tables.json` to the highest value encountered. Coverage and bundles also include a `schema_versions` array for transparency.
- Table format version (output): Centralized in code at `zpl_toolchain_spec_tables::TABLE_FORMAT_VERSION`. Emitted as `format_version` in `parser_tables.json`, and included in generated artifacts.
- Generated artifacts: `docs_bundle.json`, `constraints_bundle.json`, and `state_keys.json` include `schema_versions` and `format_version` to make downstream tooling robust to changes.


