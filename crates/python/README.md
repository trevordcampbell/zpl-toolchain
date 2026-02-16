# zpl-toolchain

![ZPL Toolchain logo](https://raw.githubusercontent.com/trevordcampbell/zpl-toolchain/main/docs/assets/branding/logo-square-128.png)

Python bindings for the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) — a spec-first, offline, deterministic ZPL II toolchain for parsing, validating, formatting, and printing Zebra Programming Language files.

Built with Rust for performance, exposed to Python via [PyO3](https://pyo3.rs/).

## Installation

```bash
pip install zpl-toolchain
```

## Quick Start

```python
import zpl_toolchain

# Parse ZPL — returns native dict/list structures by default
result = zpl_toolchain.parse("^XA^FDHello^FS^XZ")
print(f"Labels: {len(result['ast']['labels'])}")

# Validate ZPL
validation = zpl_toolchain.validate("^XA^FDHello^FS^XZ")
print(f"Valid: {validation['ok']}")

# Validate with explicit parser tables
# (tables are embedded by default for parse/validate; this is only needed for explicit override flows)
tables_json = open("generated/parser_tables.json").read()
validation2 = zpl_toolchain.validate_with_tables("^XA^FDHello^FS^XZ", tables_json)
print(f"Valid with tables: {validation2['ok']}")

# Format ZPL
formatted = zpl_toolchain.format("^XA^FD Hello ^FS^XZ", "label")

# Format with field compaction
compact = zpl_toolchain.format("^XA^FO30,30^A0N,30,30^FDHello^FS^XZ", "none", "field")
print(formatted)

# Explain a diagnostic code
explanation = zpl_toolchain.explain("ZPL1201")
print(explanation)
```

## Printing

Send ZPL directly to network printers over TCP:

```python
import zpl_toolchain

# Print ZPL to a printer (with optional validation)
result = zpl_toolchain.print_zpl(
    "^XA^FDHello^FS^XZ",
    "192.168.1.100",   # printer address (IP or hostname:port)
)
print(f"Success: {result['success']}, Bytes sent: {result['bytes_sent']}")

# Print with profile-based validation
profile_json = open("profiles/zebra-generic-203.json").read()
result = zpl_toolchain.print_zpl(
    "^XA^FDHello^FS^XZ",
    "192.168.1.100",
    profile_json,      # optional printer profile for validation
    True,              # validate before sending
)

# Query printer status
status = zpl_toolchain.query_printer_status("192.168.1.100")
print(f"Paper out: {status['paper_out']}, Paused: {status['paused']}")

# Query printer status with timeout/config overrides
status = zpl_toolchain.query_printer_status_with_options(
    "192.168.1.100",
    timeout_ms=2000,
    config_json='{"retry":{"max_attempts":2}}',
)

# Query printer identification (~HI)
info = zpl_toolchain.query_printer_info("192.168.1.100")
print(f"Model: {info.get('model')}, Firmware: {info.get('firmware')}")

# Print with timeout/config overrides
result = zpl_toolchain.print_zpl_with_options(
    "^XA^FDHello^FS^XZ",
    "192.168.1.100",
    timeout_ms=1500,
    config_json='{"timeouts":{"read_ms":4000}}',
)

```

## API

`parse`, `parse_with_tables`, `validate`, and all print/query functions return native Python `dict`/`list` objects.
`format` returns a plain formatted string, and `explain` returns a plain string (or `None`).

### Core Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `parse` | `(input: str) -> dict` | Parse ZPL, return AST + diagnostics |
| `parse_with_tables` | `(input: str, tables_json: str) -> dict` | Parse with explicit parser tables |
| `validate` | `(input: str, profile_json: str? = None) -> dict` | Parse + validate (optional profile) |
| `validate_with_tables` | `(input: str, tables_json: str, profile_json: str? = None) -> dict` | Parse + validate using explicit parser tables |
| `format` | `(input: str, indent: str? = None, compaction: str? = None) -> str` | Format ZPL (`indent`: `"none"`, `"label"`, `"field"`; `compaction`: `"none"` or `"field"`) |
| `explain` | `(id: str) -> str?` | Explain a diagnostic code, or `None` |

### Print Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `print_zpl` | `(zpl: str, addr: str, profile: str? = None, validate: bool = True) -> dict` | Send ZPL to a network printer over TCP |
| `print_zpl_with_options` | `(zpl: str, addr: str, profile: str? = None, validate: bool = True, timeout_ms: int? = None, config_json: str? = None) -> dict` | Print with timeout/config overrides |
| `query_printer_status` | `(addr: str) -> dict` | Query `~HS` host status from a printer |
| `query_printer_status_with_options` | `(addr: str, timeout_ms: int? = None, config_json: str? = None) -> dict` | Query `~HS` with timeout/config overrides |
| `query_printer_info` | `(addr: str) -> dict` | Query `~HI` printer identification |
| `query_printer_info_with_options` | `(addr: str, timeout_ms: int? = None, config_json: str? = None) -> dict` | Query `~HI` with timeout/config overrides |

## Features

- **46 diagnostic codes** covering syntax, semantics, formatting, and preflight checks
- **Printer profiles** for model-specific validation (label dimensions, DPI, memory limits)
- **Deterministic output** — identical input always produces identical results
- **Spec-driven** — parser tables generated from ZPL II command specifications
- **Fast** — native Rust performance with zero Python runtime overhead

## Requirements

- Python 3.9+
- No additional dependencies (self-contained native extension)

## Documentation

- [Print Client Guide](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/PRINT_CLIENT.md)
- [Diagnostic Codes](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/DIAGNOSTIC_CODES.md)
- [GitHub Repository](https://github.com/trevordcampbell/zpl-toolchain)

## Building from Source

```bash
pip install maturin

# Build parser tables first
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated

# Build and install (development mode)
maturin develop -m crates/python/Cargo.toml

# Or build a wheel
maturin build -m crates/python/Cargo.toml
```

## License

Dual-licensed under MIT or Apache-2.0.
