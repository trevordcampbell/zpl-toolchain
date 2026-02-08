# ADR 0003: Opcode trie for longest-match lookups

## Status
Accepted

## Context
ZPL opcodes can be 2–3+ characters and include caret/tilde variants. Longest-match lookups avoid ambiguity and backtracking.

## Decision
- The spec-compiler generates `opcode_trie.json` from registry `codes[]`.
- The parser uses the trie for O(k) opcode recognition with correct longest-match semantics.

## Consequences
- Predictable performance; supports adding multi-char opcodes safely.
- Parser remains table-driven and targetable to WASM.

## Update (2026-02-07)
The standalone `opcode_trie.json` artifact was removed. The trie data is now
only emitted as the `opcode_trie` field inside `parser_tables.json`. The trie
generation logic and runtime lookup behaviour are unchanged.
