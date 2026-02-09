# zpltoolchain — Go bindings for the ZPL toolchain

Go wrapper for the ZPL toolchain via the C FFI shared library (`libzpl_toolchain_ffi`).

## Install

```bash
go get github.com/trevordcampbell/zpl-toolchain/packages/go/zpltoolchain
```

## Prerequisites

Build the C FFI shared library first:

```bash
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated
cargo build -p zpl_toolchain_ffi --release
```

Ensure the shared library is discoverable by the linker (e.g., in `LD_LIBRARY_PATH` or `/usr/local/lib`).

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
