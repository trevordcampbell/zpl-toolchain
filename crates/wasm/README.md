wasm
====

WASM bindings for the ZPL toolchain, exposing parse, validate, format, and explain to JavaScript/TypeScript via `wasm-bindgen`.

Build
-----
```bash
# Install wasm-pack
cargo install --locked wasm-pack

# Build parser tables first (required for embedded tables)
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated

# Build WASM package (bundler target for both web and Node.js)
wasm-pack build crates/wasm --target bundler
```

The build produces `crates/wasm/pkg/` with `.wasm`, `.js`, and `.d.ts` files.

API
---

All functions use `serde-wasm-bindgen` to return native JS objects (no JSON intermediate).

| Function | Signature | Returns |
|---|---|---|
| `parse` | `(input: string) → JsValue` | `{ ast, diagnostics }` |
| `parseWithTables` | `(input: string, tablesJson: string) → JsValue` | `{ ast, diagnostics }` |
| `validate` | `(input: string, profileJson?: string) → JsValue` | `{ ok, issues }` |
| `format` | `(input: string, indent?: string) → string` | Formatted ZPL |
| `explain` | `(id: string) → string?` | Explanation or null |

Embedded tables
---------------
Parser tables are embedded at build time via `build.rs` (same pattern as the CLI — see ADR 0005). `parse()` and `format()` work out of the box. `parseWithTables()` accepts explicit tables for override.

Architecture
------------
- Thin wrapper over `crates/bindings-common/` which provides shared parse/validate/format/explain logic and embedded table management.
- `crate-type = ["cdylib", "rlib"]` — `cdylib` for `wasm-pack`, `rlib` for Rust consumers.
- ~65 lines of `wasm_bindgen` glue code.

See `packages/ts/core/` for the TypeScript wrapper that provides ergonomic types on top.
