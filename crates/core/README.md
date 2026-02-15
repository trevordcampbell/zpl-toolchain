# zpl_toolchain_core

![ZPL Toolchain logo](https://raw.githubusercontent.com/trevordcampbell/zpl-toolchain/main/docs/assets/branding/logo-square-128.png)

Parser, AST, validator, and glue for consuming generated spec tables.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Parser
- Zero-allocation lexer: `Token<'a>` borrows text directly from input (`&'a str`), eliminating per-token heap allocations.
- Longest-match opcode lookup using the opcode trie (embedded in `parser_tables.json`) with cached `ParserTables` methods (`OnceLock`).
- Signature-driven argument parsing (joiner, allowEmptyTrailing).
- Handles glued forms (e.g., `^A0N` → `f=0`, `o=N`), comments `;`, field/raw regions.
- Explicit state machine: `Mode::Normal`, `Mode::FieldData`, `Mode::RawData`.
- Emits `^XA`/`^XZ` as nodes while also delimiting labels.
- Safe UTF-8 handling throughout (multi-byte character boundary checks).

## AST
- `Ast { labels: Vec<Label> }`, `Label { nodes: Vec<Node> }`.
- `Node::Command { code, args, span } | FieldData { content, hex_escaped, span } | RawData | Trivia`. `Node` is `#[non_exhaustive]` to allow future variants without breaking downstream matches.
- `span` on all `Node` variants is a required `Span` (not `Option<Span>`).
- `ArgSlot { key, presence, value }` with tri-state `Presence`.
- `Span { start, end }` byte span (re-exported from `diagnostics` crate).

## Validator
- Table-driven checks from `spec-tables`:
  - Presence/required args; arity; type validation (`int`, `float`, `char`); unknown commands.
  - Numeric/string constraints, enums, conditional range.
  - Rounding policy (toMultiple) and profile gates (e.g., `^PW` width, `^LL` height).
  - Constraints: requires, incompatible, order (before:/after:), emptyData, note.
- Spec-driven structural validation via `FieldTracker` using `CommandEntry` flags (`opens_field`, `closes_field`, `requires_field`, etc.).
- Semantic validation: duplicate `^FN`, position bounds, font references, `^FH` hex escapes (configurable indicator via `hex_escape` module), `^GF` data length (with multi-line `RawData` continuation support), barcode `^FD` data format validation (character set and length/parity rules via `fieldDataRules`), and more.
- Device-level state tracking: `DeviceState` with unit system (`^MU`) persisting across labels; `convert_to_dots()` for unit-aware range validation.
- Dynamic prefix/delimiter support: `^CC`/`~CC`/`^CT`/`~CT` prefix changes and `^CD`/`~CD` delimiter changes tracked at both lexer and parser levels (lexer re-tokenizes with new delimiter character); commands with non-comma signature joiners (`:`, `.`) correctly preserved.
- Spec-driven `^A` split rule via `SplitRule` struct (replaces hardcoded font+orientation splitting).

## Usage
- Load `generated/parser_tables.json` and (optionally) a profile; run parse → validate.
- The crate root re-exports the most common entry points for convenience:
  - **Parser:** `parse_str`, `parse_with_tables`, `ParseResult`
  - **AST:** `Ast`, `Label`, `Node`, `ArgSlot`, `Presence`
  - **Emitter:** `emit_zpl`, `strip_spans`, `EmitConfig`, `Indent`, `Compaction`, `CommentPlacement`
  - **Diagnostics:** `Diagnostic`, `Span`, `Severity`, `codes`
  - **Validator:** `validate_with_profile`, `ValidationResult`
  - **Tables:** `ParserTables`
  - **Serialization:** `to_pretty_json`
- Full module paths (`grammar::parser::parse_str`, etc.) remain available for less common types.

## Tests
- 300+ tests split across focused test files:
  - `parser.rs` (61 tests) — tokenization, command recognition, AST structure, field/raw data modes, span tracking, prefix/delimiter, parser diagnostics, error recovery.
  - `validator.rs` (111 tests) — validation diagnostics, profile constraints, printer gates, media modes, structural/semantic validation, cross-command constraints, barcode field data.
  - `emit_roundtrip.rs` (30+ tests) — formatter round-trip/idempotency, compaction, comment placement.
  - `fuzz_smoke.rs` (26 tests) — adversarial input and invariant checking.
  - `snapshots.rs` (11 tests) — golden AST/diagnostic snapshots.
  - `samples.rs`, `cross_command_state.rs`, `rich_fields.rs`, `opcode_trie.rs`, `arg_union.rs` — targeted integration tests.
- Shared helpers centralized in `common/mod.rs` (`extract_codes`, `find_args`, `find_diag`, profile fixtures).
- `all_diagnostic_ids_have_explanations` test validates all diagnostic codes have `explain()` entries.

