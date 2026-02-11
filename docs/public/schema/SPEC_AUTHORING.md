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
    - `noSpaceAfterOpcode`: spacing rule between opcode and args. `true` (default) means no space is allowed; `false` means a space is required.
    - `allowEmptyTrailing`: keep trailing empties to preserve positions.
  - `composites` (optional): declare path-like expansions (e.g. for `^XG d:o.x`).
  - `args`: richer per-arg metadata (name/type/rangeWhen/rounding); prefer this.
  - `constraints`: declarative rules (`requires`, `incompatible`, `order`, `note`, etc.).
  - `fieldDataRules` (optional): validation rules for `^FD` content when this barcode command is active.
    - `characterSet`: compact charset notation (e.g., `"0-9"`, `"A-Z0-9 \\-.$/+%"`).
    - `minLength`, `maxLength`, `exactLength`: data length constraints.
    - `lengthParity`: `"even"` or `"odd"`.
    - `notes`: human-readable description (not used for automated validation).
    - See `docs/BARCODE_DATA_RULES.md` for the full reference.
  - `docs`, `examples` (optional): documentation strings and command examples.

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

## Validation & build

- Run `zpl-spec-compiler build` to validate files against the schema and emit parser tables.
- The compiler merges all `spec/commands/*.jsonc` into a single registry; combined files are not supported.

## Tips

- Prefer one logical command per file; use `codes` for opcode twins.
- Keep `signature.params` aligned with Zebra docs to ensure wire-accurate formatting.
- Use `args`/`constraints` for validator-driven rules; `argsSpec` has been removed.
- Add minimal `examples[]` to enable docs/testing/hover snippets in downstream tools.


