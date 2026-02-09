# @zpl-toolchain/core

TypeScript wrapper for the ZPL toolchain WASM bindings. Provides full TypeScript types and an ergonomic API for parsing, validating, formatting, and explaining ZPL code.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Installation

```bash
npm install @zpl-toolchain/core
```

## Usage

```ts
import { init, parse, format, validate, explain } from "@zpl-toolchain/core";

// Initialize WASM module (required once before calling any function)
await init();

// Parse ZPL
const result = parse("^XA^FDHello^FS^XZ");
console.log(result.ast.labels.length); // 1

// Format ZPL
const formatted = format("^XA^FD Hello ^FS^XZ", "label");

// Validate ZPL
const validation = validate("^XA^FDHello^FS^XZ");
console.log(validation.ok); // true

// Explain a diagnostic code
const explanation = explain("ZPL1201");
```

## API

| Function | Signature | Description |
|---|---|---|
| `init()` | `() → Promise<void>` | Initialize WASM module (call once) |
| `parse(input)` | `(string) → ParseResult` | Parse ZPL, return AST + diagnostics |
| `parseWithTables(input, tablesJson)` | `(string, string) → ParseResult` | Parse with explicit parser tables |
| `validate(input, profileJson?)` | `(string, string?) → ValidationResult` | Parse + validate |
| `format(input, indent?)` | `(string, IndentStyle?) → string` | Format ZPL |
| `explain(id)` | `(string) → string \| null` | Explain a diagnostic code |

## Types

All types are exported and match the Rust AST serialization format:

- **`Node`** — discriminated union on `kind`: `CommandNode | FieldDataNode | RawDataNode | TriviaNode`
- **`Severity`** — `"error" | "warn" | "info"` (lowercase, matching Rust serde)
- **`Presence`** — `"unset" | "empty" | "value"` (lowercase)
- **`IndentStyle`** — `"none" | "label" | "field"`

See `src/index.ts` for the full type definitions.

## Build from source

```bash
# Install dependencies
npm install

# Build WASM artifacts (requires wasm-pack)
npm run build:wasm

# Build TypeScript package
npm run build
```
