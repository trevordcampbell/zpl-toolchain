# Testing Guide

Comprehensive guide for running the ZPL toolchain test suite locally and understanding
how tests work in CI.

## Quick start

```bash
# Run everything (except WASM/Python binding crates which need special toolchains)
cargo nextest run --workspace --exclude zpl_toolchain_wasm --exclude zpl_toolchain_python
```

This runs **395+ Rust tests** across 9 crates. Combined with the TypeScript tests (71+),
the project has **465+ tests** total.

## Rust tests

### Full workspace

```bash
cargo nextest run --workspace --exclude zpl_toolchain_wasm --exclude zpl_toolchain_python
```

The `--exclude` flags are required because:
- `zpl_toolchain_wasm` needs `wasm-pack` and the `wasm32-unknown-unknown` target
- `zpl_toolchain_python` needs `maturin` and a Python environment

### Individual crates

```bash
cargo nextest run -p zpl_toolchain_core           # parser, validator, emitter (255 tests)
cargo nextest run -p zpl_toolchain_print_client    # print client (71 tests)
cargo nextest run -p zpl_toolchain_profile         # printer profiles
cargo nextest run -p zpl_toolchain_diagnostics     # diagnostic types
cargo nextest run -p zpl_toolchain_spec_tables     # shared data structures
cargo nextest run -p zpl_toolchain_spec_compiler   # spec compiler pipeline
cargo nextest run -p zpl_toolchain_cli             # CLI integration tests
```

### Print-client tests (TCP sockets required)

The `zpl_toolchain_print_client` crate includes **integration tests** that spin up
real TCP listeners (using `TcpListener::bind("127.0.0.1:0")`). These tests:

- Bind to ephemeral ports on localhost
- Create mock printer servers that accept connections and verify data
- Test the full send/receive cycle including status queries

**These tests will fail in sandboxed environments** that block socket creation
(e.g., certain CI containers or restricted sandboxes). If you see
`PermissionDenied` errors on `TcpListener::bind`, the environment doesn't
allow TCP sockets.

To run the workspace tests without the print-client integration tests:

```bash
cargo nextest run --workspace \
  --exclude zpl_toolchain_wasm \
  --exclude zpl_toolchain_python \
  --exclude zpl_toolchain_print_client
```

Then run the print-client tests separately in an unrestricted environment:

```bash
cargo nextest run -p zpl_toolchain_print_client
```

> **Note:** There is 1 intentionally skipped test (`reconnect_after_server_restart`)
> which tests retry/reconnection behavior that requires precise timing control.
> It is tracked in the backlog for future stabilization.

### USB and serial transport features

The CLI and print-client support optional USB and serial/Bluetooth transports,
gated behind Cargo features. To test with all transports enabled:

```bash
cargo nextest run -p zpl_toolchain_cli -p zpl_toolchain_print_client \
  --features zpl_toolchain_cli/usb,zpl_toolchain_cli/serial
```

**Linux dependency:** USB support requires `libudev-dev`:

```bash
sudo apt-get install -y libudev-dev
```

These feature-gated tests are run in CI on Linux only (see `ci.yml`).

## TypeScript tests

### @zpl-toolchain/print

The TypeScript print package has **71 tests** covering TCP connections, batch printing,
status parsing, proxy (HTTP + WebSocket), browser API, `printValidated`, and error types.

```bash
cd packages/ts/print
npm install        # install dependencies (first time only)
npm run build      # compile TypeScript to dist/ (required before testing)
npm test           # runs: node --test dist/test/*.js
```

**Important:** `npm test` runs the compiled JavaScript in `dist/test/`, not the TypeScript
source files. Always run `npm run build` after changing source files.

The test files and what they cover:

| Test file | Tests | What it covers |
|-----------|-------|----------------|
| `client.test.ts` | 14 | TCP connection, send/receive, reconnection, error handling |
| `batch.test.ts` | 7 | Batch printing, inter-label delay, status polling, abort |
| `proxy.test.ts` | 15 | HTTP proxy, WebSocket proxy, allowlist, SSRF protection, wildcard matching |
| `status.test.ts` | 6 | `parseHostStatus` response parsing |
| `printValidated.test.ts` | 8 | Profile-based validation before printing (requires `@zpl-toolchain/core` peer dep) |
| `browser.test.ts` | 6 | Browser print API (`BrowserPrinter` via Zebra Browser Print) |
| `types.test.ts` | 4 | `PrintError` class, error codes |
| `preflight.test.ts` | 11 | Preflight checks (graphics bounds, memory estimation, label dimensions) |

Some proxy tests use real TCP connections to `127.0.0.1` (which immediately
return `ECONNREFUSED`). This is intentional — it validates the proxy's connection
forwarding without requiring an actual printer, and fails fast (~3ms).

### @zpl-toolchain/core

The core TypeScript package wraps WASM and requires a full WASM build to test:

```bash
wasm-pack build crates/wasm --target bundler --out-dir ../../packages/ts/core/wasm/pkg
cd packages/ts/core
npm install
npm run build
npm test
```

## Linting and formatting

```bash
# Rust formatting
cargo fmt --all -- --check     # check only
cargo fmt --all                # auto-fix

# Rust linting (strict — warnings are errors)
cargo clippy --workspace \
  --exclude zpl_toolchain_wasm \
  --exclude zpl_toolchain_python \
  -- -D warnings

# TypeScript type-checking (no emit)
cd packages/ts/print && npx tsc --noEmit
```

### Workspace lints

The root `Cargo.toml` configures workspace-level lints inherited by all crates:

- **`missing_docs`** (warn) — all public items need doc comments
- **`unreachable_pub`** (warn) — use `pub(crate)` for internal items
- **`clippy::manual_let_else`** (warn) — prefer `let ... else` for early returns
- **`clippy::clone_on_ref_ptr`** (warn) — avoid `.clone()` on `Rc`/`Arc` directly

These are promoted to errors in CI via `RUSTFLAGS="-D warnings"`.

## CI workflows

Tests run automatically on every push and PR via GitHub Actions:

| Job | What it does |
|-----|-------------|
| `Build & Test (ubuntu/macos/windows)` | `cargo fmt`, `cargo build`, `cargo clippy`, `cargo nextest run` across 3 OS |
| `Test CLI with all transport features` | Runs print-client + CLI tests with `--features usb,serial` (Linux only) |
| `TypeScript Print Tests` | `npm install` → `tsc --noEmit` → `npm run build` → `npm test` |
| `Spec Validation & Coverage` | `zpl-spec-compiler check` + `build` + coverage report |
| `WASM Build` | `wasm-pack build` + size check |
| `Python Wheel` | `maturin build` |
| `C FFI (ubuntu/macos/windows)` | Build + verify shared library exists |

See `.github/workflows/ci.yml` for the full configuration.

## Troubleshooting

### `PermissionDenied` on print-client tests

The print-client integration tests need real TCP sockets. If your environment
blocks `bind()` calls (e.g., sandboxed containers), exclude the crate:

```bash
cargo nextest run --workspace --exclude zpl_toolchain_print_client ...
```

### `ERR_MODULE_NOT_FOUND` in TypeScript tests

The TS tests run compiled JavaScript from `dist/`, not TypeScript source.
Run `npm run build` before `npm test`.

### `libudev-dev` not found

USB transport tests require the `libudev-dev` system package on Linux:

```bash
sudo apt-get update && sudo apt-get install -y libudev-dev
```

### Slow proxy test

If the proxy wildcard test takes >15 seconds, check that it's connecting to
`127.0.0.1` (instant `ECONNREFUSED`) rather than a remote IP (timeout-based failure).
The test at `packages/ts/print/src/test/proxy.test.ts` should use `127.0.0.1:9100`.
