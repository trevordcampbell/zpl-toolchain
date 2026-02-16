# ADR 0007: Raw and Field Data Handling in the Parser

## Status
Accepted

## Implementation (2026-01)
All three parsing modes are implemented:
- **Raw payload mode** (`Mode::RawData`): collects multi-line data until next command leader; emits `Node::RawData` with spans; EOF diagnostic for unterminated data. Triggered by `CommandEntry.raw_payload` flag.
- **Field data mode** (`Mode::FieldData`): collects content until `^FS`; preserves commas in field data; respects `^FH` hex escape flag. Triggered by `CommandEntry.field_data` flag.
- **Trivia**: parser preserves non-command trivia while keeping comment semantics official (`^FX`), with no semicolon-specific lexer tokens.

9 dedicated tests for raw payload mode; field data and trivia covered by parser test suite.

## Context
The AST (in `crates/core/src/grammar/ast.rs`) already defines `RawData` and `Trivia` node types, but the parser never creates them. Several classes of ZPL content require special handling that the current "normal" parsing mode does not support:

- **Raw payload commands** (`^GF`, `~DG`, `~DY`, `~DB`) send binary or hex-encoded graphic payloads after their header parameters. The payload length is determined by a `total_bytes` parameter in the command header.
- **Field data commands** (`^FD`, `^FV`) contain free-form text that runs until the next `^FS` delimiter. When `^FH` appears in the current field, hex escape sequences (e.g. `_1A`) must be respected.
- **Comments** use official ZPL `^FX` semantics; semicolon text is treated as ordinary command/data content.
- **Round-tripping** (parse → modify → emit identical ZPL) requires that whitespace, comments, and content outside `^XA`/`^XZ` blocks are preserved in the AST.

## Decision
Introduce three parsing modes, triggered by command type:

1. **Raw payload mode** (`raw_payload: true` in `CommandEntry`) — used by `^GF`, `~DG`, `~DY`, `~DB`. After parsing header parameters, the parser reads exactly *N* bytes (from the `total_bytes` parameter) as a `RawData` node.
2. **Field data mode** (`field_data: true` in `CommandEntry`) — used by `^FD`, `^FV`. The parser collects all content until the next `^FS` as field data, respecting `^FH` hex escapes if `^FH` was seen in the current field.
3. **Normal mode** — current behaviour for all other commands.

The `raw_payload` and `field_data` flags are already present in the generated `CommandEntry` tables; the parser will consult these flags after recognising an opcode to select the appropriate mode.

`Trivia` nodes should be created for:
- Inter-command whitespace.
- Content outside `^XA`/`^XZ` blocks.

## Consequences
- Parser becomes slightly more complex with mode switching, but the modes are well-isolated and triggered by table flags rather than ad-hoc logic.
- AST becomes richer and supports round-tripping; formatters and editors can reconstruct the original source byte-for-byte.
- Raw data is opaque bytes — no validation of graphic data content is performed at parse time.
- Field data content can be validated for hex escape correctness when `^FH` is active.
- Trivia preservation increases AST size but enables formatter and editor use cases.
