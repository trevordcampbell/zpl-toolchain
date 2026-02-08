# ZplToolchain — .NET bindings for the ZPL toolchain

.NET wrapper for the ZPL toolchain via P/Invoke into the C FFI shared library (`zpl_toolchain_ffi`).

## Prerequisites

Build the C FFI shared library first:

```bash
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated
cargo build -p zpl_toolchain_ffi --release
```

Ensure `zpl_toolchain_ffi.dll` / `.so` / `.dylib` is in the application's runtime directory or system library path.

## Usage

```csharp
using ZplToolchain;

// Parse ZPL
var result = Zpl.Parse("^XA^FDHello^FS^XZ");
Console.WriteLine($"Labels: {result.Ast.Labels.Count}");

// Format ZPL
var formatted = Zpl.Format("^XA^FD Hello ^FS^XZ", "label");

// Validate ZPL
var validation = Zpl.Validate("^XA^FDHello^FS^XZ");
Console.WriteLine($"OK: {validation.Ok}");

// Explain a diagnostic code
var explanation = Zpl.Explain("ZPL1201");
```

## API

| Method | Signature | Description |
|---|---|---|
| `Zpl.Parse` | `(string input) → ParseResult` | Parse ZPL, return AST + diagnostics |
| `Zpl.ParseWithTables` | `(string input, string tablesJson) → ParseResult` | Parse with explicit tables |
| `Zpl.Validate` | `(string input, string? profileJson) → ValidationResult` | Parse + validate |
| `Zpl.Format` | `(string input, string? indent) → string` | Format ZPL |
| `Zpl.Explain` | `(string id) → string?` | Explain a diagnostic code |

## Types

The `Node` type uses a custom `JsonConverter` to handle Rust's internally-tagged enum format (`{"kind": "Command", ...}`). Check the `Kind` property to determine which fields are populated.

See `Types.cs` for full type definitions.

## Target Framework

Targets .NET Standard 2.0 for broad compatibility (works with .NET Framework 4.6.1+, .NET Core 2.0+, .NET 5+).
