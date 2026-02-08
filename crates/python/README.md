python
======

Python bindings for the ZPL toolchain via [PyO3](https://pyo3.rs/) + [maturin](https://www.maturin.rs/).

Build
-----
```bash
# Install maturin
pip install maturin

# Build parser tables first
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated

# Build and install the Python wheel (development mode)
maturin develop -m crates/python/Cargo.toml

# Or build a wheel for distribution
maturin build -m crates/python/Cargo.toml
```

Usage
-----
```python
import json
import zpl_toolchain

# Parse ZPL — returns a JSON string
result = json.loads(zpl_toolchain.parse("^XA^FDHello^FS^XZ"))
print(result["ast"]["labels"])

# Parse with explicit tables
tables_json = open("generated/parser_tables.json").read()
result = json.loads(zpl_toolchain.parse_with_tables("^XA^FDHello^FS^XZ", tables_json))

# Validate
validation = json.loads(zpl_toolchain.validate("^XA^FDHello^FS^XZ"))
print(validation["ok"])  # True

# Format
formatted = zpl_toolchain.format("^XA^FD Hello ^FS^XZ", "label")

# Explain a diagnostic code
explanation = zpl_toolchain.explain("ZPL1201")
```

API
---

All functions return **JSON strings** (callers use `json.loads()`) for simplicity and zero-dependency interop.

| Function | Signature | Returns |
|---|---|---|
| `parse` | `(input: str) → str` | JSON `{ "ast": ..., "diagnostics": [...] }` |
| `parse_with_tables` | `(input: str, tables_json: str) → str` | JSON `{ "ast": ..., "diagnostics": [...] }` |
| `validate` | `(input: str, profile_json: str? = None) → str` | JSON `{ "ok": ..., "issues": [...] }` |
| `format` | `(input: str, indent: str? = None) → str` | Formatted ZPL string |
| `explain` | `(id: str) → str?` | Explanation or None |

Architecture
------------
- Thin wrapper over `crates/bindings-common/` which provides shared parse/validate/format/explain logic and embedded table management.
- Built with `pyo3` 0.23 + `extension-module` feature.
- `pyproject.toml` uses maturin as build backend for pip/PyPI compatibility.
- Module name is `zpl_toolchain` (matches `import zpl_toolchain`).
- ~78 lines of PyO3 glue code. Python 3.9+ supported.
