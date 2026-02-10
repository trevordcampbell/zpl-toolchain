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

    // Validate ZPL
    validation, _ := zpltoolchain.Validate("^XA^FDHello^FS^XZ", "")
    fmt.Printf("OK: %v\n", validation.OK)

    // Explain a diagnostic code
    explanation := zpltoolchain.Explain("ZPL1201")
    fmt.Println(explanation)

    // Print ZPL to a network printer
    printResult, err := zpltoolchain.Print("^XA^FDHello^FS^XZ", "192.168.1.100", "", true)
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Sent %d bytes\n", printResult.BytesSent)

    // Query printer status
    statusJSON, err := zpltoolchain.QueryStatus("192.168.1.100")
    if err != nil {
        log.Fatal(err)
    }
    fmt.Println(statusJSON)
}
```

## API

| Function | Signature | Description |
|---|---|---|
| `Parse` | `(input string) (*ParseResult, error)` | Parse ZPL, return AST + diagnostics |
| `ParseWithTables` | `(input, tablesJSON string) (*ParseResult, error)` | Parse with explicit tables |
| `Validate` | `(input, profileJSON string) (*ValidationResult, error)` | Parse + validate |
| `Format` | `(input, indent string) (string, error)` | Format ZPL |
| `Explain` | `(id string) string` | Explain a diagnostic code |
| `Print` | `(zpl, printerAddr, profileJSON string, validate bool) (*PrintResult, error)` | Send ZPL to a network printer |
| `QueryStatus` | `(printerAddr string) (string, error)` | Query printer host status |

## Types

The `Node` type uses a custom `UnmarshalJSON` to handle Rust's internally-tagged enum format (`{"kind": "Command", ...}`). Access the specific variant via `node.Command`, `node.Field`, `node.Raw`, or `node.Trivia` (check `node.Kind` first).

See `types.go` for full type definitions.

---

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain).
