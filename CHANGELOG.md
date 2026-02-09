# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/trevordcampbell/zpl-toolchain/compare/v0.1.1...v0.1.2) - 2026-02-09

### Added

- add print client — send ZPL to printers over TCP, USB, and serial

### Fixed

- resolve CI failures and add TypeScript print tests to CI

### Other

- update Cargo.lock dependencies

### Added

- **Print client** (`crates/print-client/`) — send ZPL to Zebra and ZPL-compatible printers over TCP (port 9100), USB (nusb), and serial/Bluetooth SPP (serialport); split `Printer`/`StatusQuery` trait design; `~HS` (24-field HostStatus) and `~HI` (PrinterInfo) parsing; STX/ETX frame parser; `RetryPrinter` wrapper with exponential backoff and jitter; `ReconnectRetryPrinter` wrapper with automatic reconnection between retry attempts via `Reconnectable` trait; `wait_for_completion()` generic polling; `PrintError` typed error hierarchy with `is_retryable()` classification; `#[non_exhaustive]` on all public enums for semver safety
- **CLI `zpl print` command** — `zpl print <FILES> --printer <ADDR>` with lint-then-print workflow, `--dry-run`, `--status`, `--info`, `--wait`, `--wait-timeout`, `--strict`, `--no-lint`, `--serial`, `--baud`; supports TCP (IP/hostname), USB (`usb` / `usb:VID:PID`), and serial/Bluetooth (`--serial`) transports; `--output json` consolidates all data into a single JSON envelope
- **TypeScript print package** (`@zpl-toolchain/print`) — pure TypeScript Node.js TCP print client using `node:net`; `TcpPrinter` class with persistent connections, `getStatus()`, `query()`, `isReachable()`; one-shot `print()` function; `printValidated()` with optional printer profile and strict mode; `parseHostStatus()` with full 24-field parity; HTTP proxy server (`createPrintProxy`) with SSRF protection and wildcard/glob allowlist patterns (e.g., `"192.168.1.*"`); WebSocket endpoint on the same port for persistent bidirectional communication with origin validation, `maxPayload` enforcement, and 30-second ping/pong keepalive; `_processValidationResult()` extracted for testability; 13 `printValidated` validation-logic unit tests; Zebra Browser Print SDK wrapper for browser environments
- **Python/C FFI bindings** — `print_zpl()` and `query_printer_status()` exposed in `bindings-common` (cfg-gated, not WASM)
- **Go print bindings** — `Print()` and `QueryStatus()` functions in `packages/go/zpltoolchain/` for sending ZPL over TCP and querying `~HS` printer status via the C FFI
- **.NET print bindings** — `Zpl.Print()` and `Zpl.QueryStatus()` in `packages/dotnet/ZplToolchain/` for sending ZPL over TCP and querying `~HS` printer status via P/Invoke
- **TypeScript batch API** — `printBatch()` standalone function and `TcpPrinter.printBatch(labels, opts?, onProgress?)` / `TcpPrinter.waitForCompletion()` methods for sending multiple labels with progress tracking; `BatchOptions`, `BatchProgress`, `BatchResult` types
- **Preflight diagnostics** — ZPL2308 (graphics bounds — `^GF` exceeds printable area), ZPL2309 (graphics memory — total `^GF` memory exceeds printer RAM), ZPL2310 (missing explicit `^PW`/`^LL` label dimension commands); 10 new validator tests
- **Zebra Browser Print tests** — 19 unit tests covering `isAvailable()`, `discover()`, `print()`, `getStatus()`, legacy device list parsing, and error handling
- **Rust batch/completion tests** — 8 new tests for `send_batch_with_status` (happy path, no interval, early abort, error propagation), `wait_for_completion` (immediate, polling, timeout, formats_in_buffer)

### Changed

- **TcpPrinter write queue** — `print()` now serializes concurrent writes through an internal promise-chain mutex, preventing interleaved data on concurrent calls; `close()` drains the write queue before teardown
- **Proxy `allowedPorts`** — new `ProxyConfig.allowedPorts` option (default `[9100]`) restricts which TCP ports the proxy can connect to; empty array = deny all (consistent with `allowedPrinters`)
- **Proxy `maxConnections`** — new `ProxyConfig.maxConnections` option (default `50`) limits concurrent WebSocket connections; excess connections rejected with 503
- **Proxy CORS single-string fix** — `verifyClient` and `setCorsHeaders` now correctly handle single-string CORS configurations (was silently allowing all origins)
- **Proxy allowlist hardening** — glob-to-regex patterns are pre-compiled once at startup (no per-request regex compilation); uses `[^.]*` instead of `.*` to prevent ReDoS
- **`readBody` double-rejection guard** — `readBody` includes a `settled` guard to prevent double-rejection when `req.destroy()` is called on oversized payloads
- **`^MU` unit normalization** — `^FO`/`^FT` positions and `^PW`/`^LL` dimensions are now normalized to dots via `convert_to_dots()` when recorded in `LabelState`, fixing ZPL2302 and ZPL2308 bounds checks when `^MU` sets inches or millimeters
- **`^GF` math hardening** — `bytes_per_row * 8` uses `saturating_mul` to prevent overflow; `graphic_field_count / bytes_per_row` uses `div_ceil` for correct ceiling division
- **`#[non_exhaustive]` on structs** — added to `HostStatus`, `PrinterInfo`, `BatchProgress`, `BatchResult`, `PrinterConfig`, `PrinterTimeouts`, `RetryConfig`, `BatchOptions` for semver safety (already on all public enums)
- **`SockRef` keepalive** — `configure_keepalive` uses `SockRef::from()` instead of `Socket::from(stream.try_clone()?)`, avoiding unnecessary file descriptor cloning
- **Socket leak fixes** — `tcpQuery` `finish()` now calls `socket.destroy()`; `TcpPrinter.close()` has a 2-second force-destroy timer; `isReachable()` has a 1-second force-destroy timer after `sock.end()`
- **TcpPrinter idle timeout fix** — `sock.setTimeout(0)` called after connection to disable idle timeout; TCP keepalive handles liveness for persistent connections
- **Cleanup timer `.unref()`** — force-close timers in `tcpSend`, `isReachable`, and `close()` now call `.unref()` to prevent Node.js process hang on exit
- **Batch status polling best-effort** — `getStatus()` in `printBatch` wrapped in try/catch; status query failures no longer abort the batch
- **`serde_json` → dev-dependency** — moved from runtime to dev-dependencies in print-client (only used in tests)
- **`CompletionTimeout` includes `formats_in_buffer`** — error now reports both formats in buffer and labels remaining for better diagnostics
- **`RetryPrinter` accessor methods** — added `inner()` / `inner_mut()` for symmetry with `ReconnectRetryPrinter`
- **CLI `--timeout` minimum** — rejects `--timeout 0` via clap value parser range constraint (minimum 1)
- **CLI JSON mode quieter** — progress messages (`sent: ...`, `connected to ...`, `waiting for printer...`) only shown in `--output pretty`, not JSON
- **TS input validation** — `resolveConfig()` validates host (non-empty), port (1–65535), and timeout for clear `PrintError` messages
- **Proxy glob `*` matches IPs** — bare `*` in `allowedPrinters` now matches dotted IPs/hostnames (special-cased as match-all)
- **Proxy `readBody` doesn't destroy socket** — uses `removeAllListeners` + `resume` instead of `req.destroy()` so 413 responses are delivered
- **Proxy body-read timeout** — 30-second deadline on `readBody()` mitigates slow-loris attacks (408 on timeout)
- **Proxy error message sanitization** — HTTP and WebSocket error responses return generic messages instead of leaking internal TCP error details
- **Proxy HTTP connection limit** — `server.maxConnections` set to match the WebSocket `maxConnections` limit
- **Proxy Content-Type case-insensitive** — `Content-Type` header check now uses `.toLowerCase()` per RFC 2045
- **C# `NodeJsonConverter.Write` fix** — serializes via `NodeDto` to avoid `StackOverflowException` from infinite recursion
- **C# `Validate` profileJson normalization** — empty string converted to null before FFI call, matching `Print()` behavior
- **.NET README** — updated compatibility claim to .NET Core 3.1+ / .NET 5+ (LPUTF8Str not supported on .NET Framework)
- **TypeScript `prepublishOnly`** — added to both `@zpl-toolchain/core` and `@zpl-toolchain/print` as a safety net
- **CLI transport features** — USB (`--features usb`) and serial (`--features serial`) are now opt-in cargo features; TCP is default and always available; `cargo install zpl_toolchain_cli` works without system dependencies; pre-built release binaries include all transports
- **CI: TypeScript print tests** — new CI job runs type-check, build, and all 71 tests for `@zpl-toolchain/print`
- **CI: libudev-dev** — Ubuntu CI runners now install `libudev-dev` for USB transport compilation
- **CI: all-features test** — Linux CI additionally runs tests with USB and serial features enabled to exercise all transport code paths

## [0.1.1](https://github.com/trevordcampbell/zpl-toolchain/compare/v0.1.0...v0.1.1) - 2026-02-08

### Other

- add crate-level doc comments to all published crates

## [0.1.0] — 2026-02-06

### Changed

- **Constraint kinds single source of truth** — `ConstraintKind::ALL` constant in `spec-tables` is the canonical list of valid constraint kinds; a `constraint_kinds_match_schema` test in `spec-compiler` validates the JSONC schema stays in sync
- **Diagnostic codes consolidation** — `spec/diagnostics.jsonc` is now the single source of truth; `crates/diagnostics/build.rs` auto-generates `codes.rs` constants and `explain()` match arms at build time; fixed missing `ZPL.PARSER.1301` entry
- **Extract `bindings-common` crate** — shared parse/validate/format/explain logic and embedded table management centralized in `crates/bindings-common/`; FFI/WASM/Python reduced to thin type-conversion wrappers (~600 lines of duplication eliminated)
- **Test organization** — validator tests (111) split from `parser.rs` into dedicated `validator.rs`; helpers centralized in `common/mod.rs`; all test files standardized on shared table loading
- **Spec directory reorganization** — `spec/schema/` now contains both `zpl-spec.schema.jsonc` and `profile.schema.jsonc` (schemas separated from data files); `crates/spec/schema/` removed (single canonical location)
- **Generated artifacts cleanup** — removed redundant standalone `opcode_trie.json` (already embedded in `parser_tables.json`); documented `docs_bundle.json` and `constraints_bundle.json` as external-tooling artifacts
- **Core crate public API re-exports** — added convenience re-exports at `zpl_toolchain_core::` root for parser, AST, emitter, diagnostics, validator, and table types (shorter import paths for consumers)
- **Workspace dependency consistency** — `wasm-bindgen`, `serde-wasm-bindgen`, `pyo3` now use workspace-level version management; removed unused `zpl_toolchain_spec` dep from spec-compiler
- **Documentation consolidation** — root README slimmed from ~327 to ~225 lines; removed duplicated sections in favour of cross-references to CONTRIBUTING.md, PROFILE_GUIDE.md, CHANGELOG.md, and crate READMEs
- **CI workflow deduplication** — extracted `.github/actions/setup-rust/` composite action; reduced 7 toolchain installs and 6 table-build blocks to a single reusable action; standardized cache key naming
- **Production `unwrap()` cleanup** — `generate_docs_bundle()` and `generate_constraints_bundle()` return `Result` with proper `?` propagation; `unreachable!()` now includes descriptive message
- **Profile validation strengthening** — `id`, `schema_version`, `dpi` are now required fields (non-`Option`); `load_profile_from_str()` enforces DPI range (100–600), speed range (1–14 ips), darkness range (0–30), page/memory positivity; validator code simplified (no more `.unwrap_or("unknown")` for profile ID); 10 new tests
- **Pre-release foundation hardening (Tier A)** — `ConstraintKind` Display impl; diagnostic span invariants in fuzz tests; defensive `serde(rename_all = "camelCase")` on 9 spec-tables structs; removed unused `anyhow` from core; `Span` now required (non-Option) on all `Node` variants; `#[non_exhaustive]` on `Node` enum; `CommandScope`/`CommandCategory`/`Stability` enums replace `Option<String>` fields
- **Pre-release foundation hardening (Tier B)** — `ProfileError` enum with `thiserror` (replaces `anyhow` in profile crate); `ValidationContext`/`CommandCtx` structs eliminate all `too_many_arguments` suppressions; `validate_semantic_state` split into 5 focused functions; `validate_cross_field` split into 8 focused functions with `visit_args` helper; doc comments on all public types with `#![warn(missing_docs)]` enabled on core, spec-tables, diagnostics crates
- **Post-hardening review fixes** — `bindings-common` now uses `load_profile_from_str` for proper validation (was bypassing semantic checks); profile loading rejects empty `id`/`schema_version`; added `Copy` to `ComparisonOp`/`ConstraintKind`/`ConstraintSeverity`/`RoundingMode` enums; orphaned `^FS` detection moved to early return in `validate_field_close`; parse diagnostics now prepend validation diagnostics in bindings output (source-order)
- **Final quality review fixes** — 26 changes across code, data, CI, and docs: FFI error JSON properly escaped via serde_json; CLI `syntax-check` `ok` semantics fixed (true unless Error-severity); CLI `--tables` errors reported instead of silently swallowed; validator detects unclosed fields at end-of-label; parser validates ASCII for `^CC`/`^CT`/`^CD` prefix changes (`PARSER_NON_ASCII_ARG`); `char_in_set` handles reversed ranges; `strip_jsonc` fixed for UTF-8; `arg_keys()` OneOf false-positive fix; diagnostics `build.rs` rewrite with proper JSONC stripping, duplicate/validity checks, newline escaping; `dump.rs`/`render.rs` use `expect()` over silent fallbacks; `bindings-common` `embedded_tables()` fails loudly on invalid embedded tables; pipeline uses explicit match arms for `ConstraintKind`; WASM `validate` exported with consistent `js_name`; removed unused `serde_json`/`serde` deps; TypeScript import path and API call corrected; data fixes (schema scope adds `label`, ZT610-300 speed corrected to 12 ips, QR/EAN sample data fixed, generic profile `lcd=false`); CI gets `--locked` on all spec-check commands and `RUSTFLAGS` in release workflow; test codes use `codes::` constants; exhaustive diagnostic test updated
- **Bindings hardening** — Go/NET `ParseWithTables`/`Validate` now detect FFI error responses instead of silently returning empty results; Python functions renamed for cross-binding consistency (`validate_zpl` → `validate`, `format_zpl` → `format`, `parse_with_tables_json` → `parse_with_tables`); `span` made required (non-optional) on all AST node types across TypeScript, Go, and .NET bindings to match Rust serialization; FFI `zpl_parse`/`zpl_parse_with_tables` use direct `ParseResult` serialization instead of manual JSON construction; TypeScript package adds `tsconfig.json` for declaration generation, WASM type stub for pre-build IDE support

### Added

- **Full ZPL II command coverage** — 216 spec files covering 223/223 commands (100%), each audited against the official Zebra Programming Guide PDF
- **Spec-first architecture** — per-command JSONC specs in `spec/commands/` compiled to parser tables, docs bundles, constraints bundles, and coverage reports
- **Hand-written parser** — zero-allocation lexer, opcode trie longest-match, signature-driven argument parsing, field/raw data modes, prefix/delimiter state tracking, trivia preservation
- **Table-driven validator** — presence/arity, type validation (int/float/char), enums, ranges, rounding, constraints DSL (requires/incompatible/order/emptyData/note), cross-command state tracking, profile constraints, printerGates, media validation, barcode field data validation, unit conversion
- **`zpl format` command** — spec-driven auto-formatter with configurable indentation (none/label/field), trailing-arg trimming, split-rule merging, field/raw data preservation
- **CLI** — `parse`, `syntax-check`, `lint`, `format`, `coverage`, `explain` commands with `--output pretty|json` auto-detection, `ariadne`-powered coloured diagnostics, embedded parser tables
- **Printer profiles** — 11 shipped profiles (GK420t, ZD420, ZD620, ZD621, ZT231, ZT410, ZT411, ZT610, ZQ520, plus generics) with page bounds, speed/darkness ranges, 10 hardware feature gates, media capabilities, DPI-dependent defaults
- **Ecosystem bindings** — WASM (wasm-bindgen), TypeScript (@zpl-toolchain/core), Python (PyO3/maturin), C FFI (cdylib/staticlib), Go (cgo), .NET (P/Invoke); all expose unified 5-function API
- **Diagnostic system** — 45 diagnostic codes with structured context, severity levels, byte-offset spans, `explain()` for every code
- **Spec-compiler** — typed pipeline with cross-field validation, constraint DSL parsing, schema version enforcement
- **CI** — multi-OS matrix (Linux/macOS/Windows), cargo cache, rustfmt/clippy checks, spec validation, coverage report, WASM size check, Python wheel build, C FFI cross-platform build
