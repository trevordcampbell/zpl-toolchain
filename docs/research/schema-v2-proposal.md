# Schema v2 Proposal (Future)

> **Status**: Unimplemented proposal. The current schema is v1.1.1 (`spec/schema/zpl-spec.schema.jsonc`).
> See `docs/BACKLOG.md` for the backlog entry.

## Motivation

The v1.x schema is a single registry file with a `commands[]` array validated by one JSON Schema document. This works well but has scaling limitations for richer semantic modeling.

## Proposed Changes

| Area | v1.x (current) | v2 (proposed) |
|------|----------------|---------------|
| Command storage | `commands[]` array | Object map keyed by opcode — O(1) lookup, prevents duplicates |
| Args | Flat `args[]` list | Nested `schema.params[]` with syntax mode (`positional` / `kv` / `mixed`) |
| Effects | `{ "sets": [...] }` | Structured `{ "stateChanges": [...], "dependencies": [...], "sideEffects": [...] }` |
| Constraints | `kind` enum with `expr` string | DSL-based predicates with `appliesTo` scoping and `onViolation` actions |
| Typing | `int` / `float` / `enum` / `string` / `char` | Richer type classes with `nullable`, `implicitDefault`, `format` (units) |
| Composition | `$defs` in one file | Modular JSONC files — `paramGroup` for shared param layouts, `familyRegistry` for command clusters |
| Provenance | None | `source.url` and `source.rev` for traceability to Zebra docs |
| Examples | `zpl` string + `pngHash` | `zpl` + `render` (PNG/SVG) + `expected` + `notes` + `source` |

## Key Design Decisions

1. **Opcode-keyed map** instead of array — eliminates duplicates, enables direct lookup
2. **Reusable `paramGroup`** — shared argument layouts across related commands (e.g., all `^R*` RFID commands)
3. **`familyRegistry`** — groups related opcodes for inheritance and metadata propagation
4. **Declarative signature templates** — `"template": "^RS{t},{p},{v},{n}"` enables programmatic signature synthesis

## Trade-offs

- Not pure JSON Schema — requires toolchain validator instead of generic AJV
- More nesting/complexity for schema authors
- Migration cost: old parsers expect `args` at top level
- Multi-file imports require a build/resolution step

## Migration Strategy

1. Keep v1.x schema as validator for existing `schemaVersion <= 1.1.x` files
2. Provide a compiler bridge to flatten v2 modules into v1.x shape (and vice versa)
3. Dual-loader during transition period
4. Update downstream consumers (doc generator, CLI, validation) to v2 interfaces

## Example: Old vs New

```jsonc
// v1.x (current)
{ "codes": ["^RS"], "arity": 8,
  "args": [
    { "name": "tag type", "key": "t", "type": "enum", "enum": ["8"], "default": "8" },
    { "name": "program position", "key": "p", "type": "string" }
  ]
}

// v2 (proposed)
{ "opcode": "^RS", "scope": "job", "plane": "device",
  "schema": {
    "params": [
      { "key": "t", "name": "tag type", "type": "enum", "enum": ["8"], "default": "8", "doc": "EPC Class 1, Gen 2" },
      { "key": "p", "name": "program position", "type": "string", "doc": "F0–Fxxx or B0–B30" }
    ],
    "syntax": "positional", "joiner": ","
  },
  "effects": { "stateChanges": ["rfid.errorPolicy", "rfid.position"] },
  "source": { "url": "https://docs.zebra.com/...", "rev": "2025-01" }
}
```

---

*Consolidated from the original SchemaRefactor.md, SchemaRefactorEvolution.md, and SchemaRefactorDiffs.md design documents (Feb 2026).*
