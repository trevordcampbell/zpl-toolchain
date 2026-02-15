# Testing Guide

Comprehensive guide for running the ZPL toolchain test suite locally and understanding
how tests work in CI.

## Quick start

```bash
# Run all Rust workspace tests
cargo nextest run --workspace
```

This runs Rust tests across all workspace crates.
For TypeScript and wheel/runtime binding coverage, see the package-level sections below.

## Rust tests

### Full workspace

```bash
cargo nextest run --workspace
```

This is the project default and release-readiness Rust test path.
Python binding confidence is additionally validated via wheel/runtime tests:

```bash
bash scripts/test-python-wheel-local.sh
```

For local Python/.NET binding confidence, use the helper scripts:

```bash
bash scripts/test-python-wheel-local.sh
bash scripts/test-dotnet-local.sh
bash scripts/test-go-local.sh
```

`test-python-wheel-local.sh` builds the wheel and runs tests in a local venv
(`.venv-python-wheel-tests`) so it does not depend on system-site package writes.

### Individual crates

```bash
cargo nextest run -p zpl_toolchain_core           # parser, validator, emitter (300+ tests)
cargo nextest run -p zpl_toolchain_print_client    # print client tests
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

If your local environment blocks socket `bind()`, use a constrained fallback run:

```bash
cargo nextest run --workspace \
  --exclude zpl_toolchain_print_client
```

Then run print-client tests separately in an unrestricted environment:

```bash
cargo nextest run -p zpl_toolchain_print_client
```

> **Note:** There is 1 intentionally skipped test (`reconnect_after_server_restart`)
> which tests retry/reconnection behavior that requires precise timing control.
> It is tracked in the backlog for future stabilization.

### USB and serial transport features

All transports (TCP, USB, serial/Bluetooth) are **enabled by default** for the CLI.
No extra `--features` flags are needed:

```bash
cargo nextest run -p zpl_toolchain_cli -p zpl_toolchain_print_client
```

If you explicitly built with `--no-default-features`, you can re-enable them:

```bash
cargo nextest run -p zpl_toolchain_cli -p zpl_toolchain_print_client \
  --features zpl_toolchain_cli/usb,zpl_toolchain_cli/serial
```

> **Note:** The `serialport` dependency is built with `default-features = false` to
> avoid requiring `libudev-dev` on Linux. Port enumeration (`list_ports()`) still
> works via a sysfs fallback but may return less metadata than the libudev backend.
> The `nusb` crate (USB) is pure Rust and has no system dependencies.

## TypeScript tests

### @zpl-toolchain/print

The TypeScript print package has **89 test cases** across the core test files
covering TCP connections, batch printing, status parsing, proxy (HTTP + WebSocket),
browser API, `printValidated`, and error types.

```bash
cd packages/ts/print
npm ci             # install dependencies from lockfile
npm run build      # compile TypeScript to dist/ (required before testing)
npm test           # runs: node --test --test-timeout=30000 dist/test/*.js
```

**Important:** `npm test` runs the compiled JavaScript in `dist/test/`, not the TypeScript
source files. Always run `npm run build` after changing source files.

The test files and what they cover:

| Test file | Tests | What it covers |
|-----------|-------|----------------|
| `batch.test.ts` | 7 | Batch printing, inter-label delay, status polling, abort |
| `browser.test.ts` | 19 | Browser print API (`ZebraBrowserPrint`) behavior and error handling |
| `print.test.ts` | 8 | TCP print flows using a mock TCP server |
| `printValidated.test.ts` | 13 | Validation behavior before printing (strict/non-strict, issue handling) |
| `proxy.test.ts` | 32 | HTTP/WebSocket proxy behavior, security controls, and limits |
| `status.test.ts` | 6 | `parseHostStatus` response parsing |
| `types.test.ts` | 4 | `PrintError` class and error code contracts |

Some proxy tests use real TCP connections to `127.0.0.1` (which immediately
return `ECONNREFUSED`). This is intentional — it validates the proxy's connection
forwarding without requiring an actual printer, and fails fast (~3ms).

The test runner timeout (`--test-timeout=30000`) is a safety guard against
stuck tests, not a substitute for fixing failures. Network-dependent suites
use runtime network-availability checks; in CI we assert local TCP bind support
so these integration tests do not get silently skipped.

### @zpl-toolchain/core

The core TypeScript package wraps WASM and requires a full WASM build to test:

```bash
wasm-pack build crates/wasm --target bundler --out-dir ../../packages/ts/core/wasm/pkg
cd packages/ts/core
npm ci
npm run build
npm test
```

If you changed Rust sources in `crates/core`, `crates/wasm`, or
`crates/bindings-common` (or regenerated `generated/parser_tables.json`), rebuild
WASM before TypeScript core or extension validation.

### @zpl-toolchain/cli

The CLI wrapper package has lightweight runtime mapping tests:

```bash
cd packages/ts/cli
npm test
```

### VS Code extension

The VS Code extension package validates type safety, build integrity, and VSIX
packaging with:

```bash
cd packages/vscode-extension
npm ci
npm test
npm run test:integration
npm run package:vsix
```

`npm run build` (and therefore `test` / `test:ci` / `package:vsix`) now includes a
freshness guard (`check:core-runtime-freshness`) that fails fast when
`packages/ts/core/wasm/pkg` is stale relative to Rust/WASM sources or parser tables.

`npm run package:vsix` verifies that the extension can be packaged into a
distributable VSIX with bundled runtime assets.

`npm run test:integration` runs Extension Host tests via `@vscode/test-electron`
through `packages/vscode-extension/scripts/run-integration-tests.mjs`.
On Linux, this wrapper uses `xvfb-run` for headless execution.
On `linux/arm64` local environments, tests are skipped by default due upstream
launcher limitations unless you provide an explicit executable path override or
set `FORCE_VSCODE_INTEGRATION=1`.

More elegant local workaround on `linux/arm64`: provide a known-good local VS Code
or Cursor binary path and run with:

```bash
VSCODE_EXECUTABLE_PATH=/path/to/code npm run test:integration
```

Performance regression fixture:

- Integration suite includes a large-document diagnostics latency guard.
- Default budget is `8000ms`, override with:

```bash
ZPL_VSCODE_PERF_BUDGET_MS=6000 npm run test:integration
```

Manual runtime validation (Extension Development Host):

1. Open `packages/vscode-extension` in VS Code.
2. Press `F5` to launch an Extension Development Host.
3. Open a `.zpl` file and verify:
   - diagnostics update on edit
   - formatting works (`Format Document` or format-on-save)
   - hover docs resolve command metadata
   - diagnostic explain command opens useful details

Automated Extension Host integration tests are now part of the extension test
suite. Additional UI-heavy end-to-end scenarios can be layered in future phases.

## Linting and formatting

```bash
# Rust formatting
cargo fmt --all -- --check     # check only
cargo fmt --all                # auto-fix

# Rust linting (strict — warnings are errors)
cargo clippy --workspace -- -D warnings

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
| `Build & Test (ubuntu/macos/windows)` | `cargo fmt`, `cargo build`, `cargo clippy`, `cargo nextest run` across 3 OS (all transports are default) |
| `TypeScript Core Tests` | `wasm-pack build crates/wasm --target bundler --out-dir ../../packages/ts/core/wasm/pkg` → `npm ci` → `tsc --noEmit` → `npm run build` → `node --test dist/test/*.js` |
| `TypeScript Print Tests` | `npm ci` → `tsc --noEmit` → `npm run build` → artifact assertions + local TCP bind precheck → `npm test` |
| `TypeScript CLI Wrapper Tests` | `npm test` in `packages/ts/cli` (platform mapping + unsupported-runtime guard coverage) |
| `VS Code Extension Build & Test` | `npm ci` → `npm run test:ci` (typecheck/build + extension-host integration where supported) → `npx @vscode/vsce package` + VSIX integrity check |
| `Spec Validation & Coverage` | `zpl-spec-compiler check` + `build` + enforced `note-audit` + coverage report |
| `WASM Build` | `wasm-pack build` + size check |
| `Python Wheel` | `maturin build` |
| `Python Runtime Tests (py3.9–3.13)` | Build wheel + install wheel + `python -m unittest discover -s crates/python/tests -v`. **PRs:** reduced subset (3.9, 3.12, 3.13). **Push to main:** full matrix (3.9–3.13). |
| `Go Bindings Runtime Tests` | Build FFI release library + `go test -v ./...` for Go wrapper runtime behavior |
| `.NET Bindings Runtime Tests` | Build FFI release library + `dotnet test` for .NET wrapper runtime behavior |
| `C FFI (ubuntu/macos/windows)` | Build + verify shared library exists |

See `.github/workflows/ci.yml` for the full configuration.

## Note audience validation

Contextual note routing is spec-driven and test-covered:

- `zpl-spec-compiler note-audit --spec-dir spec --format json`
  reports note constraints that likely need conditional expressions or
  `audience` refinement, and findings fail CI.
- CLI diagnostics support `--note-audience` on `lint` and `print`:
  - `--note-audience all` (default): includes contextual notes.
  - `--note-audience problem`: excludes contextual notes from diagnostic output.

## Troubleshooting

### `PermissionDenied` on print-client tests

The print-client integration tests need real TCP sockets. If your environment
blocks `bind()` calls (e.g., sandboxed containers), use a constrained fallback:

```bash
cargo nextest run --workspace --exclude zpl_toolchain_print_client
```

### `ERR_MODULE_NOT_FOUND` in TypeScript tests

The TS tests run compiled JavaScript from `dist/`, not TypeScript source.
Run `npm run build` before `npm test`.

### `libudev-dev` not found (historical)

Previous versions required `libudev-dev` on Linux for serial port support.
With the current configuration (`serialport` built with `default-features = false`),
`libudev-dev` is **no longer required** for building. If you see this error from an
older build, update to the latest version.

### Slow proxy test

If the proxy wildcard test takes >15 seconds, check that it's connecting to
`127.0.0.1` (instant `ECONNREFUSED`) rather than a remote IP (timeout-based failure).
The test at `packages/ts/print/src/test/proxy.test.ts` should use `127.0.0.1:9100`.

### `dotnet` not found when running local .NET tests

If `bash scripts/test-dotnet-local.sh` reports `dotnet is required but not found on PATH`,
rebuild the devcontainer after enabling the .NET feature in
`.devcontainer/devcontainer.json`.

### `go` not found when running local Go tests

If `bash scripts/test-go-local.sh` reports `go is required but not found on PATH`,
rebuild the devcontainer after enabling the Go feature in
`.devcontainer/devcontainer.json`.

### PyO3 / Python toolchain mismatches

If Python-related builds fail after changing local Python versions, re-run:

```bash
bash scripts/setup-pyo3-env.sh
```

The devcontainer runs this automatically in `postCreateCommand`.
