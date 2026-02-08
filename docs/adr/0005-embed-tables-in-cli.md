# ADR 0005: Embed parser tables in CLI

## Status
Accepted — implemented in CLI, WASM, Python, and FFI crates via `bindings-common`.

## Context
Requiring `--tables` is inconvenient for casual use and distribution. Embedding tables can make `zpl` self-contained while still allowing overrides.

## Decision
- Use `build.rs` to include `generated/parser_tables.json` (and trie/constraints) at build time when present.
- Keep `--tables` to override embedded tables for advanced workflows.

## Consequences
- Better out-of-the-box experience; larger binary size.
- Rebuild required when spec updates.

## Implementation
Applied across all consumer crates. The `bindings-common` crate centralizes the embedded table loading via `embedded_tables()`, which is used by CLI, WASM, Python, and C FFI bindings. The `--tables` CLI flag remains as an override.

