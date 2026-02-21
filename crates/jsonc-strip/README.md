# zpl_toolchain_jsonc_strip

Shared JSONC comment stripping utility used by multiple build-time tooling paths.

## Purpose

Provides a single `strip_jsonc()` implementation to prevent drift between:
- `crates/spec-compiler`
- `crates/diagnostics` build script

## Guarantees

- strips `//` line comments
- strips `/* ... */` block comments
- preserves string literals and escaped characters
- UTF-8 safe (operates on chars, not raw bytes)

## Usage

```rust
use zpl_toolchain_jsonc_strip::strip_jsonc;

let stripped = strip_jsonc(r#"{ "a": 1, /* comment */ "b": 2 }"#);
```
