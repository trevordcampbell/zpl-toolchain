# ADR 0004: JSONC vs JSON for spec authoring

## Status
Accepted

## Context
We require human-friendly, commentable files for a large, evolving command registry. Plain JSON lacks comments and is verbose for manual editing.

## Decision
- Use JSONC (JSON with comments) for authoring under `spec/commands/` and for the schema at `spec/schema/zpl-spec.schema.jsonc`.
- The spec-compiler strips comments before schema validation and merging.

## Consequences
- Improves readability and maintainability with inline explanations and field docs.
- Requires a preprocessing step in the compiler.

