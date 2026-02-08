# ADR 0008: Ecosystem Bindings Architecture

## Status
Accepted

## Context
The ZPL toolchain core is written in Rust, but consumers exist across many ecosystems: web/VS Code (JavaScript/TypeScript), scripting (Python), enterprise (.NET), systems (Go/C). Exposing the toolchain to all of these requires a principled binding strategy that balances ergonomics, performance, and maintenance cost.

## Decision

### Layer architecture

Three binding layers, each wrapping the same `core` crate:

1. **WASM** (`crates/wasm/`) — `wasm-bindgen` + `serde-wasm-bindgen` for zero-copy JS interop. TypeScript wrapper at `packages/ts/core/`.
2. **Python** (`crates/python/`) — `pyo3` + `maturin` for native Python module.
3. **C FFI** (`crates/ffi/`) — `extern "C"` functions as the universal bridge for Go (`packages/go/zpltoolchain/`) and .NET (`packages/dotnet/ZplToolchain/`).

### Shared API surface

All targets expose the same 5 functions:

| Function | Description |
|---|---|
| `parse(input)` | Parse ZPL, return AST + diagnostics |
| `parseWithTables(input, tablesJson)` | Parse with explicit parser tables |
| `validate(input, profileJson?)` | Parse + validate with optional profile |
| `format(input, indent?)` | Auto-format ZPL |
| `explain(id)` | Explain a diagnostic code |

### JSON as the universal wire format

- **WASM** uses native JS objects via `serde-wasm-bindgen` (no JSON intermediate — faster).
- **Python** returns JSON strings (callers use `json.loads()`). This is simpler than `pythonize` and avoids complex type mapping.
- **C FFI** returns heap-allocated JSON C strings. Callers free with `zpl_free()`.
- **Go** and **.NET** wrappers unmarshal JSON into native types.

### Embedded tables

All binding crates embed `parser_tables.json` via `build.rs` (same pattern as the CLI, see ADR 0005). Tables are cached with `OnceLock` — parsed exactly once per process/module lifetime.

### Internally-tagged enum handling

The Rust AST serializes `Node` with `#[serde(tag = "kind")]` (internally tagged), producing `{"kind": "Command", "code": "^XA", ...}`. Each language wrapper handles this:

- **TypeScript**: Discriminated union type on `kind` field.
- **Go**: Custom `UnmarshalJSON` that peeks at `kind` before dispatching.
- **.NET**: Custom `JsonConverter<Node>` that reads `kind` first.

## Consequences

- **Unified API** across all languages — same 5 functions, same behavior.
- **Thin wrappers** — each binding crate is ~100-130 lines of Rust glue; language wrappers are similarly small.
- **Maintenance cost is low** — adding a new core feature (e.g., a 6th API function) requires updating each binding, but the pattern is mechanical.
- **JSON overhead is negligible** for a dev tool (parsing a ZPL file is microseconds; JSON serialization adds little).
- **WASM has zero JSON overhead** thanks to `serde-wasm-bindgen` converting directly to JS objects.
- **Binary size trade-off** — each binding embeds parser tables (~200KB). Acceptable for dev tooling.
- **Cross-target parity** — all targets produce identical parse/validate output (same core code).
