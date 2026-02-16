# zpltoolchain — Go bindings for the ZPL toolchain

Go wrapper for the ZPL toolchain via the C FFI shared library (`libzpl_toolchain_ffi`).

## Install

```bash
go get github.com/trevordcampbell/zpl-toolchain/packages/go/zpltoolchain
```

## Prerequisites

You need the FFI shared library (`libzpl_toolchain_ffi.so` / `.dylib` / `.dll`).

**Option 1: Download prebuilt** (recommended)

Download the FFI library for your platform from the latest
[GitHub Release](https://github.com/trevordcampbell/zpl-toolchain/releases),
then place it on your linker search path (e.g. `LD_LIBRARY_PATH` on Linux,
`DYLD_LIBRARY_PATH` on macOS, or alongside your executable on Windows).

**Option 2: Build from source** (requires Rust toolchain)

```bash
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated
cargo build -p zpl_toolchain_ffi --release
```

The library will be at `target/release/libzpl_toolchain_ffi.{so,dylib,dll}`.

## Usage

```go
package main

import (
    "fmt"
    "log"
    "github.com/trevordcampbell/zpl-toolchain/packages/go/zpltoolchain"
)

func main() {
    // Parse ZPL
    result, err := zpltoolchain.Parse("^XA^FDHello^FS^XZ")
    if err != nil {
        panic(err)
    }
    fmt.Printf("Labels: %d\n", len(result.Ast.Labels))

    // Format ZPL
    formatted, _ := zpltoolchain.Format("^XA^FD Hello ^FS^XZ", "label")
    fmt.Println(formatted)

    // Format with compaction
    compact, _ := zpltoolchain.FormatWithOptions("^XA^FO30,30^A0N,30,30^FDHello^FS^XZ", "label", "field")
    fmt.Println(compact)

    // Validate ZPL
    validation, _ := zpltoolchain.Validate("^XA^FDHello^FS^XZ", "")
    fmt.Printf("OK: %v\n", validation.OK)

    // Validate with explicit parser tables
    parserTablesJSON := `{"commands":{}}` // parser tables JSON payload
    validation2, _ := zpltoolchain.ValidateWithTables("^XA^FDHello^FS^XZ", parserTablesJSON, "")
    fmt.Printf("OK (tables): %v\n", validation2.OK)

    // Explain a diagnostic code
    explanation := zpltoolchain.Explain("ZPL1201")
    fmt.Println(explanation)

    // Print ZPL to a network printer
    printResult, err := zpltoolchain.Print("^XA^FDHello^FS^XZ", "192.168.1.100", "", true)
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Sent %d bytes\n", printResult.BytesSent)

    // Query printer status (typed)
    status, err := zpltoolchain.QueryStatusTyped("192.168.1.100")
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Printer mode: %s\n", status.PrintMode)

    // Query printer info (typed)
    info, err := zpltoolchain.QueryInfoTyped("192.168.1.100")
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Model: %s, Firmware: %s\n", info.Model, info.Firmware)
}
```

## API

| Function | Signature | Description |
|---|---|---|
| `Parse` | `(input string) (*ParseResult, error)` | Parse ZPL, return AST + diagnostics |
| `ParseWithTables` | `(input, tablesJSON string) (*ParseResult, error)` | Parse with explicit tables |
| `Validate` | `(input, profileJSON string) (*ValidationResult, error)` | Parse + validate |
| `ValidateWithTables` | `(input, tablesJSON, profileJSON string) (*ValidationResult, error)` | Parse + validate with explicit tables |
| `Format` | `(input, indent string) (string, error)` | Format ZPL |
| `FormatWithOptions` | `(input, indent, compaction string) (string, error)` | Format ZPL with optional compaction (`none`/`field`) |
| `Explain` | `(id string) string` | Explain a diagnostic code |
| `Print` | `(zpl, printerAddr, profileJSON string, validate bool) (*PrintResult, error)` | Send ZPL to a network printer |
| `PrintWithOptions` | `(zpl, printerAddr, profileJSON string, validate bool, opts *PrintOptions) (*PrintResult, error)` | Print with timeout/config overrides |
| `QueryStatus` | `(printerAddr string) (string, error)` | Query printer host status (raw JSON) |
| `QueryStatusWithOptions` | `(printerAddr string, timeoutMs uint64, configJSON string) (string, error)` | Query status with timeout/config overrides |
| `QueryStatusTyped` | `(printerAddr string) (*HostStatus, error)` | Query printer host status (typed) |
| `QueryStatusTypedWithOptions` | `(printerAddr string, timeoutMs uint64, configJSON string) (*HostStatus, error)` | Typed status query with timeout/config overrides |
| `QueryInfo` | `(printerAddr string) (string, error)` | Query printer identification (raw JSON) |
| `QueryInfoWithOptions` | `(printerAddr string, timeoutMs uint64, configJSON string) (string, error)` | Query info with timeout/config overrides |
| `QueryInfoTyped` | `(printerAddr string) (*PrinterInfo, error)` | Query printer identification (typed) |
| `QueryInfoTypedWithOptions` | `(printerAddr string, timeoutMs uint64, configJSON string) (*PrinterInfo, error)` | Typed info query with timeout/config overrides |

## Types

The `Node` type uses a custom `UnmarshalJSON` to handle Rust's internally-tagged enum format (`{"kind": "Command", ...}`). Access the specific variant via `node.Command`, `node.Field`, `node.Raw`, or `node.Trivia` (check `node.Kind` first).

`ValidationResult` also includes optional `resolved_labels` entries with renderer-ready per-label resolved state snapshots (`values`, `effective_width`, `effective_height`).

See `types.go` for full type definitions.

---

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain).
