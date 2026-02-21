# Spec Authoring Guide (per-command JSONC)

This guide describes how to author ZPL II command specifications under `spec/commands/`. Each command has its own JSONC file. The spec-compiler merges these into a single registry and emits parser/validator tables.

## File layout

- One file per command family, named by the canonical opcode, e.g. `^BC.jsonc`, `^A.jsonc`, `^XG.jsonc`.
- Files are JSONC (allowing `//` and `/* */` comments); the spec-compiler strips comments before validation.

## Minimal file template

```jsonc
{
  "version": "0.1.0",
  "schemaVersion": "1.1.1",
  "commands": [
    {
      "codes": ["^BC"],                    // one or more opcodes (first is canonical)
      "arity": 6,                           // positional argument count
      "signature": {                        // how args appear in wire format
        "params": ["o","h","f","g","e","m"],
        "joiner": ",",
        "allowEmptyTrailing": true
      },
      "args": [                             // richer per-arg metadata (preferred)
        { "name": "module_width", "key": "m", "type": "int", "range": [1,10] },
        { "name": "wide_to_narrow_ratio", "key": "r", "type": "int", "range": [1,3], "optional": true, "default": 3 },
        { "name": "bar_height", "key": "h", "type": "int", "range": [1,32000], "unit": "dots", "optional": true }
      ],
      "docs": "Code 128 barcode"
    }
  ]
}
```

## Fields

- `version`: semantic version for this data file (not the schema version).
- `schemaVersion`: must be `1.1.1`.
- `commands[]`: one or more logical command definitions (usually one per file).
  - `codes`: array of opcodes (e.g., `["^BC"]` or caret/tilde twins `["^CC","~CC"]`).
  - `arity`: declared positional argument count (wire arity). Use with `signature.params` to map short keys.
  - `signature`: canonical wire format representation.
    - `params`: ordered short keys or composite names, matching Zebra docs.
    - `joiner`: delimiter (`,` for ZPL).
    - `spacingPolicy`: spacing rule between opcode and args:
      - `"forbid"` (default): no space allowed
      - `"require"`: space required
      - `"allow"`: both forms accepted
    - `noSpaceAfterOpcode` (legacy): backward-compatible boolean mapping (`true` => `"forbid"`, `false` => `"require"`). Prefer `spacingPolicy` for new specs.
    - `allowEmptyTrailing`: keep trailing empties to preserve positions.
  - `composites` (optional): declare path-like expansions (e.g. for `^XG d:o.x`).
  - `args`: richer per-arg metadata (name/type/rangeWhen/rounding); prefer this.
  - `constraints`: declarative rules (`requires`, `incompatible`, `order`, `note`, etc.).
    - `kind: "note"` supports optional `audience`:
      - `"problem"` for diagnostics lists
      - `"contextual"` for explanatory guidance intended for rich help surfaces
        (hover/details/docs), not problem lists
    - `kind: "note"` also supports optional `expr` predicates (`when:`, `before:`,
      `after:`) for conditional emission.
  - For `kind: "note"`, see [Note constraints](#note-constraints-kind-note) below.
  - `fieldDataRules` (optional): validation rules for `^FD` content when this barcode command is active.
    - `characterSet`: compact charset notation (e.g., `"0-9"`, `"A-Z0-9 \\-.$/+%"`).
    - `minLength`, `maxLength`, `exactLength`: data length constraints.
    - `lengthParity`: `"even"` or `"odd"`.
    - `notes`: human-readable description (not used for automated validation).
    - See `docs/BARCODE_DATA_RULES.md` for the full reference.
  - `structuralRules` (optional): schema-driven semantic rule payloads used by validator dispatch/indexing.
    - Typical usages:
      - `duplicateFieldNumber` (`^FN`)
      - `positionBounds` actions (`^PW`, `^LL`, `^FO`, `^FT`)
      - `fontReference` actions (`^CW`, `^A`)
      - `mediaModes` targets (`^MM`, `^MN`, `^MT`)
      - `gfDataLength` / `gfPreflightTracking` (`^GF`)
    - See command examples in `spec/commands/^FN.jsonc`, `^PW.jsonc`, `^FO.jsonc`, and `^GF.jsonc`.
  - `docs`, `examples` (optional): documentation strings and command examples.

## Note constraints (kind: note)

Note constraints emit ZPL3001 diagnostics. Use `audience` and `expr` to control where and when they surface.

### Audience

- `audience: "problem"` (default) — Emit in CLI diagnostics and editor Problems panels.
- `audience: "contextual"` — Emit only in rich help surfaces (hover, docs, details). Omit from Problems to keep lists focused.

Use `audience: "contextual"` for explanatory guidance (e.g. "sets defaults for subsequent commands") rather than actionable issues.

### Conditional emission (`expr`)

| Prefix | Meaning |
|--------|---------|
| `after:<codes>` | Emit only if target command(s) appear earlier in the label (or field). |
| `after:first:<codes>` | Same, but only if at least one target has appeared for the first time. |
| `before:<codes>` | Emit only if target command(s) have not yet appeared. |
| `before:first:<codes>` | Same, but scoped to first occurrence within the evaluation scope. |
| `when:<predicate>` | Emit only when the predicate expression evaluates to true. |

Codes in `after:`/`before:` are pipe-separated (e.g. `after:^FD|^FV`).

### `when:` predicates

Supported predicates (combine with `&&`, `||`, `!`):

- **Arg-based:** `arg:<key>IsValue:V1|V2`, `arg:<key>Present`, `arg:<key>Empty`
- **Label-based:** `label:has:^CODE1|^CODE2`, `label:missing:^CODE`
- **Profile-based:** (when a profile is loaded; all return false when no profile)
  - `profile:id:ID1|ID2` — profile id exact match
  - `profile:dpi:203|300` — DPI match
  - `profile:feature:X|Y` — any listed feature is present (`cutter`, `rfid`, etc.)
  - `profile:featureMissing:X` — feature explicitly absent
  - `profile:firmware:V60` — firmware version starts with prefix
  - `profile:firmwareGte:V60.14` — firmware version ≥ (major.minor comparison)
  - `profile:model:X|Y` — profile id contains any listed substring

Examples:

```jsonc
{ "kind": "note", "expr": "when:arg:pIsValue:S||arg:hIsValue:S", "message": "Short calibration only on Xi4/RXi4..." }
{ "kind": "note", "expr": "when:arg:bPresent&&!arg:modeIsValue:M", "message": "Parameter b only used when mode=M." }
{ "kind": "note", "expr": "when:arg:aIsValue:28|29|30&&!profile:firmwareGte:V60.14", "message": "Values 28-30 require firmware V60.14.x or later." }
{ "kind": "note", "expr": "when:!profile:model:kr403", "message": "KR403 printer required." }
{ "kind": "note", "expr": "after:first:^FS", "message": "Compatibility note after first ^FS." }
```

### Scope

- `scope: "label"` — Evaluate constraints against commands in the entire label.
- `scope: "field"` — Evaluate within the current field (`^FO`…`^FS`).

For `kind: "order"`, `kind: "requires"`, and `kind: "incompatible"`, `scope` is required.
For `kind: "note"` and `kind: "custom"`, `scope` is optional; when omitted, evaluation follows command scope (`field` commands evaluate in-field, others evaluate label-wide).
Use `scope: "field"` for field-scoped commands so checks are evaluated per-field, not label-wide.

Examples:

```jsonc
{ "kind": "order", "expr": "before:^FD|^FV", "scope": "field", "message": "^FB should precede its field data", "severity": "info" }
{ "kind": "requires", "expr": "^BY", "scope": "label", "message": "^BY should precede ^BC", "severity": "warn" }
{ "kind": "note", "expr": "after:first:^FS", "audience": "contextual", "message": "Compatibility note after first ^FS." } // scope optional
```

## Authoring examples

### ^A (Select font)

```jsonc
{
  "version": "0.1.0",
  "schemaVersion": "1.1.1",
  "commands": [
    {
      "codes": ["^A"],
      "arity": 4,
      "signature": { "params": ["f","o","h","w"], "joiner": ",", "allowEmptyTrailing": true },
      "args": [ { "name": "font", "key": "f", "type": "string", "optional": true }, { "name": "orientation", "key": "o", "type": "enum", "enum": ["","N","R","I","B"] }, { "name": "height", "key": "h", "type": "int", "range": [0,32000], "unit": "dots" }, { "name": "width", "key": "w", "type": "int", "range": [0,32000], "unit": "dots" } ],
      "docs": "Device font selection; orientation inherits when empty"
    }
  ]
}
```

Notes:
- The parser supports glued font+orientation like `^A0N,…` by splitting `0N` → `f=0`, `o=N`.

### ^XG (recall graphic) composite

```jsonc
{
  "version": "samples-1",
  "schemaVersion": "1.1.1",
  "commands": [
    {
      "codes": ["^XG"],
      "arity": 3,
      "signature": { "params": ["d:o.x","mx","my"], "joiner": ",", "allowEmptyTrailing": true },
      "composites": [
        { "name": "d:o.x", "template": "{d}:{o}.{x}", "exposesArgs": ["d","o","x"] }
      ],
      "docs": "Recall graphic by path with magnification"
    }
  ]
}
```

### ^BY and ^BC (ranges and enums)

Use `args[]` for numeric ranges and enums (e.g., module width/ratio/height, orientation and flags). Prefer `constraints[]` for ordering/requirements (e.g., `requires:^BY` for barcodes needing module width default).

## Cross-command defaults (`defaultFrom`)

When a consumer arg inherits from producer state:

- declare producer state keys in `effects.sets` on the producer command
- declare `defaultFrom` on the consumer arg
- add `defaultFromStateKey` whenever `defaultFrom` is used

Example (`^BY` producer + barcode consumer):

```jsonc
// producer
"effects": { "sets": ["barcode.moduleWidth", "barcode.ratio", "barcode.height"] }
```

```jsonc
// consumer arg
{
  "name": "height",
  "key": "h",
  "type": "int",
  "unit": "dots",
  "optional": true,
  "defaultFrom": "^BY",
  "defaultFromStateKey": "barcode.height"
}
```

Compiler validation rules:

- `defaultFrom` must reference a known command with `effects.sets`
- `defaultFromStateKey` is required when `defaultFrom` is present
- `defaultFromStateKey` must exist in producer `effects.sets`
- use canonical keys declared by producers in `effects.sets` (see `docs/STATE_MAP.md` and generated `state_keys.json`)

## Validation & build

- Run `zpl-spec-compiler build` to validate files against the schema and emit parser tables.
- Run `zpl-spec-compiler note-audit --spec-dir spec --format json`
  to review note-quality findings (missing conditionalization opportunities,
  likely contextual-only notes). In CI, findings are treated as failures.
- The compiler merges all `spec/commands/*.jsonc` into a single registry; combined files are not supported.

## Tips

- Prefer one logical command per file; use `codes` for opcode twins.
- Keep `signature.params` aligned with Zebra docs to ensure wire-accurate formatting.
- Use `args`/`constraints` for validator-driven rules; `argsSpec` has been removed.
- Add minimal `examples[]` to enable docs/testing/hover snippets in downstream tools.


