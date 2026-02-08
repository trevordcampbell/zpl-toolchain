# ADR 0001: Spec-first authoring in JSONC, one file per command

## Status
Accepted

## Context
ZPL has many commands and variants. Authoring and maintaining validation logic inline in code leads to drift and duplication.

## Decision
- Author a registry of ZPL commands as JSONC files under `spec/commands/`, one file per logical command family.
- Use JSONC to enable rich inline comments and documentation.
- Validate these against a JSONC schema (v1.1.1) and compile into canonical tables via a spec-compiler.

## Consequences
- Clear single source of truth for signatures/args/constraints.
- Enables documentation, tooling, and cross-target reuse.
- Requires a compiler step to produce generated tables.

