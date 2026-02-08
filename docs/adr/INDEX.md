# Architecture Decision Records (ADR) Index

- [0001: Spec-first authoring in JSONC, one file per command](0001-spec-first-jsonc-per-command.md)
  - Source-of-truth registry in JSONC under `spec/commands/`; compiled to tables.

- [0002: Data-driven constraints engine](0002-constraints-engine-data-driven.md)
  - Express validation rules in spec (`args`/`constraints`); validator interprets tables.

- [0003: Opcode trie for longest-match lookups](0003-opcode-trie-and-parser-lookups.md)
  - Opcode trie embedded in `parser_tables.json`; parser performs O(k) longest-match opcode recognition.

- [0004: JSONC vs JSON for spec authoring](0004-jsonc-vs-json.md)
  - Use JSONC for comments and readability; compiler strips comments before validation.

- [0005: Embed parser tables in CLI](0005-embed-tables-in-cli.md)
  - Embed generated tables at build time via `build.rs`, with `--tables` override. Applied to CLI, WASM, Python, and FFI crates.

- [0006: Profile schema and versioning](0006-profile-schema-and-versioning.md)
  - Versioned profile schema with dpi, page dimensions, speed/darkness ranges, features, media, memory. Two shipped profiles (203/300 dpi).

- [0007: Raw and Field Data Handling in the Parser](0007-raw-field-data-handling.md)
  - Three parsing modes (raw payload, field data, normal) triggered by table flags; trivia preservation for round-tripping.

- [0008: Ecosystem Bindings Architecture](0008-ecosystem-bindings.md)
  - Three binding layers (WASM, Python, C FFI) wrapping the core crate; shared 5-function API; JSON as universal wire format; Go/.NET wrappers on C FFI.
