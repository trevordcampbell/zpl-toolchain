# ADR 0002: Data-driven constraints engine

## Status
Accepted

## Context
Validation rules (requires/incompatible/order/range) were previously hardcoded and scattered. We need consistency and evolution without frequent code edits.

## Decision
- Express constraints in the per-command spec (`constraints[]`, `args[]` range/enum/conditional/rounding metadata).
- The validator interprets these tables to enforce presence, order, incompatibilities, and conditional numeric rules.

## Consequences
- Rules evolve with data; code remains generic.
- Easier to test and document; supports future WASM rulelets.

