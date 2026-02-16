# ZplToolchain — .NET bindings for the ZPL toolchain

.NET wrapper for the ZPL toolchain via P/Invoke into the C FFI shared library (`zpl_toolchain_ffi`).

## Prerequisites

You need the FFI shared library (`zpl_toolchain_ffi.dll` / `.so` / `.dylib`).

**Option 1: Download prebuilt** (recommended)

Download the FFI library for your platform from the latest
[GitHub Release](https://github.com/trevordcampbell/zpl-toolchain/releases),
then place it in your application's runtime directory or system library path
(e.g. `LD_LIBRARY_PATH` on Linux, `DYLD_LIBRARY_PATH` on macOS, or alongside
your executable on Windows).

**Option 2: Build from source** (requires Rust toolchain)

```bash
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated
cargo build -p zpl_toolchain_ffi --release
```

The library will be at `target/release/libzpl_toolchain_ffi.{so,dylib,dll}`.
Copy it to your application's runtime directory or add its location to your
system library path.

## Usage

```csharp
using ZplToolchain;

// Parse ZPL
var result = Zpl.Parse("^XA^FDHello^FS^XZ");
Console.WriteLine($"Labels: {result.Ast.Labels.Count}");

// Format ZPL
var formatted = Zpl.Format("^XA^FD Hello ^FS^XZ", "label");
var compact = Zpl.FormatWithOptions("^XA^FO30,30^A0N,30,30^FDHello^FS^XZ", "label", "field");

// Validate ZPL
var validation = Zpl.Validate("^XA^FDHello^FS^XZ");
Console.WriteLine($"OK: {validation.Ok}");

// Validate with explicit parser tables
string parserTablesJson = "{}"; // parser tables JSON payload
var validation2 = Zpl.ValidateWithTables("^XA^FDHello^FS^XZ", parserTablesJson);

// Explain a diagnostic code
var explanation = Zpl.Explain("ZPL1201");

// Print ZPL to a network printer
var printResult = Zpl.Print("^XA^FDHello^FS^XZ", "192.168.1.100");
Console.WriteLine($"Sent {printResult.BytesSent} bytes");

// Print with profile-based validation
string profileJson = File.ReadAllText("my-printer-profile.json");
var result2 = Zpl.Print("^XA^FDHello^FS^XZ", "192.168.1.100", profileJson: profileJson);

// Query printer status (typed)
HostStatus status = Zpl.QueryStatusTyped("192.168.1.100");
Console.WriteLine(status.PrintMode);

// Query printer info (typed)
PrinterInfo info = Zpl.QueryInfoTyped("192.168.1.100");
Console.WriteLine($"{info.Model} ({info.Firmware})");
```

## API

| Method | Signature | Description |
|---|---|---|
| `Zpl.Parse` | `(string input) → ParseResult` | Parse ZPL, return AST + diagnostics |
| `Zpl.ParseWithTables` | `(string input, string tablesJson) → ParseResult` | Parse with explicit tables |
| `Zpl.Validate` | `(string input, string? profileJson) → ValidationResult` | Parse + validate |
| `Zpl.ValidateWithTables` | `(string input, string tablesJson, string? profileJson) → ValidationResult` | Parse + validate with explicit tables |
| `Zpl.Format` | `(string input, string? indent) → string` | Format ZPL |
| `Zpl.FormatWithOptions` | `(string input, string? indent, string? compaction) → string` | Format ZPL with optional compaction (`none`/`field`) |
| `Zpl.Explain` | `(string id) → string?` | Explain a diagnostic code |
| `Zpl.Print` | `(string zpl, string printerAddr, string? profileJson, bool validate) → PrintResult` | Send ZPL to a network printer |
| `Zpl.PrintWithOptions` | `(string zpl, string printerAddr, string? profileJson, bool validate, ulong? timeoutMs, string? configJson) → PrintResult` | Print with timeout/config overrides |
| `Zpl.QueryStatus` | `(string printerAddr) → string` | Query printer host status (raw JSON) |
| `Zpl.QueryStatusWithOptions` | `(string printerAddr, ulong? timeoutMs, string? configJson) → string` | Query status with timeout/config overrides |
| `Zpl.QueryStatusTyped` | `(string printerAddr, ulong? timeoutMs, string? configJson) → HostStatus` | Query printer host status (typed) |
| `Zpl.QueryInfo` | `(string printerAddr) → string` | Query printer identification (raw JSON) |
| `Zpl.QueryInfoWithOptions` | `(string printerAddr, ulong? timeoutMs, string? configJson) → string` | Query info with timeout/config overrides |
| `Zpl.QueryInfoTyped` | `(string printerAddr, ulong? timeoutMs, string? configJson) → PrinterInfo` | Query printer identification (typed) |

## Types

The `Node` type uses a custom `JsonConverter` to handle Rust's internally-tagged enum format (`{"kind": "Command", ...}`). Check the `Kind` property to determine which fields are populated.

`ValidationResult` also includes optional `resolved_labels` entries that expose renderer-ready per-label resolved state snapshots (`values`, `effective_width`, `effective_height`).

See `Types.cs` for full type definitions.

## Target Framework

Targets .NET Standard 2.0. Requires .NET Core 3.1+ or .NET 5+ at runtime (uses `UnmanagedType.LPUTF8Str` for UTF-8 string marshalling, which is not supported on .NET Framework).

---

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain).
