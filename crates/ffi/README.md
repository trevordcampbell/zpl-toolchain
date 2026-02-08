ffi
===

C-compatible FFI bindings for the ZPL toolchain. Produces a shared library (`libzpl_toolchain_ffi.so` / `.dylib` / `.dll`) and a static library for embedding.

This crate is the foundation for the Go and .NET language wrappers.

Build
-----
```bash
# Build parser tables first
cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated

# Build the shared + static library
cargo build -p zpl_toolchain_ffi --release

# Generate the C header (requires cbindgen)
cargo install --locked cbindgen
cbindgen --config crates/ffi/cbindgen.toml --crate zpl_toolchain_ffi -o zpl_toolchain.h
```

API
---

All functions accept null-terminated C strings and return heap-allocated C strings (JSON or plain text). **The caller MUST free returned pointers with `zpl_free()`.**

```c
// Parse ZPL → JSON { "ast": ..., "diagnostics": [...] }
char* zpl_parse(const char* input);

// Parse with explicit tables → JSON { "ast": ..., "diagnostics": [...] }
char* zpl_parse_with_tables(const char* input, const char* tables_json);

// Parse + validate → JSON { "ok": ..., "issues": [...] }
// profile_json may be NULL.
char* zpl_validate(const char* input, const char* profile_json);

// Format ZPL → formatted string
// indent may be NULL ("none"), "label", or "field".
char* zpl_format(const char* input, const char* indent);

// Explain a diagnostic code → string or NULL
char* zpl_explain(const char* id);

// Free a string returned by any zpl_* function. NULL-safe.
void zpl_free(char* ptr);
```

Usage from C
------------
```c
#include <stdio.h>

// Link with -lzpl_toolchain_ffi
extern char* zpl_parse(const char* input);
extern void  zpl_free(char* ptr);

int main() {
    char* result = zpl_parse("^XA^FDHello^FS^XZ");
    if (result) {
        printf("%s\n", result);
        zpl_free(result);
    }
    return 0;
}
```

Architecture
------------
- Thin wrapper over `crates/bindings-common/` which provides shared parse/validate/format/explain logic and embedded table management.
- `crate-type = ["cdylib", "staticlib"]` — shared library for dynamic linking, static for embedding.
- `cbindgen.toml` configures C header generation.
- All functions are `extern "C"` with `#[unsafe(no_mangle)]` (Rust 2024 edition).
- NULL inputs return NULL; invalid JSON inputs return an error JSON string.

See `packages/go/zpltoolchain/` and `packages/dotnet/ZplToolchain/` for language wrappers built on this FFI.
