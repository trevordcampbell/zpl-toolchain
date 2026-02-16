bindings-common
===============

Shared core logic for ZPL toolchain language bindings (FFI, WASM, Python).

Provides:
- `embedded_tables()` — lazy-loaded parser tables via `include_str!`
- `parse_zpl()` / `parse_zpl_with_tables_json()` — parse with embedded or explicit tables (`Result<...>`)
- `validate_zpl()` — parse + validate with optional profile
- `format_zpl()` — parse + format with configurable indentation (`Result<String, String>`)
- `explain_diagnostic()` — look up diagnostic code explanations
- `parse_indent()` — convert indent string to `Indent` enum

Each binding crate wraps these with its target-specific type conversions.
