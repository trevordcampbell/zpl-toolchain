# zpl_toolchain_spec_compiler

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Purpose
- Validate per-command JSONC files under `spec/commands/` against `spec/schema/zpl-spec.schema.jsonc`.
- Merge into a single registry and emit generated artifacts:
  - `generated/parser_tables.json`
  - `generated/constraints_bundle.json`
  - `generated/docs_bundle.json`
  - `generated/coverage.json`

## CLI
```bash
zpl-spec-compiler build --spec-dir spec --out-dir generated
zpl-spec-compiler check --spec-dir spec
```

## Inputs
- `spec/commands/*.jsonc`: one file per command family (`codes[]` + `arity` + `signature` + `args/constraints`).
- Schema reference: `../../spec/schema/zpl-spec.schema.jsonc`.

## Outputs
- `parser_tables.json`: canonical table set consumed by parser/validator (includes the opcode trie inline).
- `constraints_bundle.json`: constraints extracted per command code (not consumed at runtime; available for external tooling such as IDE plugins and documentation generators).
- `docs_bundle.json`: per-code docs view with signature, args, docs, enumValues, composites.exposesArgs, missingFields (not consumed at runtime; available for external tooling).
- `coverage.json`: present/missing counts; per_code stats (arg_count, union_positions, missing fields, validation_errors).

## Notes
- Comments are allowed in source JSONC; the compiler strips them before validation.
- The compiler passes through fields to `spec-tables` structures and performs cross-field validation (signature/args/composites/overrides; arg hygiene).
- A `constraint_kinds_match_schema` test validates that `ConstraintKind::ALL` (the Rust enum) and the JSONC schema's `kind` enum stay in sync.

