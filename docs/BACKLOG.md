# ZPL Toolchain — Project Backlog

> Single source of truth for tactical work items. For the strategic roadmap (phases, priorities, and architectural decisions), see [ROADMAP.md](ROADMAP.md). Original plans archived at `docs/research/archive/`.
>
> Last updated: 2026-02-09

---

## Open Items — Prioritized

### Tier 1: Core Deepening (high value — improves correctness and completeness)

All Tier 1 items completed. See "Completed Work" below.

### Tier 2: Developer Experience & Tooling (unblocks first release)

- [x] **`LineIndex` utility** — byte-offset → line/column conversion in `diagnostics` crate; zero external dependencies; reusable by WASM/LSP; 8 unit tests
- [x] **CLI: pretty output formatting** — `ariadne` 0.6 for coloured source-annotated diagnostics; TTY detection via `std::io::IsTerminal`; `--output pretty|json` flag with auto-detection; severity-coloured summary; render module in `crates/cli/src/render.rs`
- [x] **CLI: embed tables via `build.rs`** — `build.rs` copies `generated/parser_tables.json` into binary at compile time; `--tables` flag retained as override; resolves ADR 0005; no more mandatory `--tables` for parse/lint/syntax-check
- [x] **`zpl format` command** — spec-driven ZPL auto-formatter in `crates/core/src/grammar/emit.rs`; one command per line, trailing-arg trimming, split-rule merging, spec-driven joiners (`,`, `:`, `.`, `""`), command prefix tracking (`^CC`); `--write` for in-place formatting, `--check` for CI (exit 1 if not formatted), `--indent none|label|field` for configurable indentation; field data and raw payloads preserved byte-for-byte; graceful degradation without tables (comma fallback); 23 round-trip tests including idempotency, USPS sample, prefix change, and all indent modes
- [x] **WASM bindings** — `crates/wasm/` with `wasm-bindgen` + `serde-wasm-bindgen`; 5 exported functions (parse, parseWithTables, validate, format, explain); embedded parser tables via `build.rs`; TypeScript wrapper at `packages/ts/core/` with full type definitions
- [x] **CI: expand matrix** — Linux/macOS/Windows build matrix for core tests and C FFI; `Swatinem/rust-cache@v2` for Cargo caching; `taiki-e/install-action@nextest` for nextest; `rustfmt --check` and `clippy -D warnings` gates; WASM size report via `$GITHUB_STEP_SUMMARY`
- [x] **CI: add spec-compiler check step** — dedicated `spec-check` job runs `zpl-spec-compiler check`; builds tables; runs CLI `coverage` command to report spec coverage in CI
- [x] **Draft release notes template** — `docs/RELEASE.md` with version scheme, crate publishing order, release checklist, release notes template with checksums; `CHANGELOG.md` (Keep a Changelog format); `.github/workflows/release.yml` with cross-platform CLI and FFI builds, artifact upload, and GitHub Release creation (manual `workflow_dispatch` fallback)

### Tier 3: Quality & Polish

- [x] **Golden snapshot expansion** — 5 new tests: unknown commands, multi-label, raw payload (`^GF`), tilde commands, cross-command state (11 total golden snapshots)
- [x] **Expand sample corpus** — 4 new ZPL labels: shipping, product, warehouse, compliance (5 total samples)
- [x] **Parser error recovery** — `skip_to_next_leader()` resync after invalid commands; stray content warning (`ZPL.PARSER.1301`); coalesced stray token diagnostics; new diagnostic code with explain() entry
- [x] **Community-contributed profiles** — 9 printer-specific profiles: GK420t, ZD420, ZD620, ZD621, ZT231, ZT410, ZT411, ZT610, ZQ520 (11 total with generics)
- [x] **Spec-compiler: strengthen cross-field validation** — signature↔args↔composites consistency; docs bundle has composites and enum value docs
- [x] **Spec-compiler: composite expansion** — enum value docs included in docs bundle generation
- [x] **Spec-compiler: `--strict` flag** — `--strict` on `build` command fails with `anyhow::bail!` on validation warnings
- [x] **Spec-compiler: fix silent walkdir errors** — proper `map_err` error propagation replaces `filter_map(|e| e.ok())`
- [x] **Spec-compiler: constraint kinds dedup** — documented canonical source (`ConstraintKind` enum in spec-tables) and sync requirements; wildcard arm documented
- [x] **Spec-compiler: schema version consistency** — `next_back()` documented as intentional (BTreeSet ascending → last = highest/latest); not arbitrary
- [x] **Spec-compiler: `missing_fields` declarative** — `REQUIRED_FIELDS` const table with `RequiredField` struct; adding a field = one entry
- [x] **Add concise `docs` strings** — all 216 spec files already have docs strings (100% coverage)
- [x] **Implement `range` and `note` constraint kinds at runtime** — `Note` already emits ZPL3001 info diagnostics; `Range` documented as future extension (range validation via `args[].range`); `Custom` remains escape-hatch

### Tier 4: Ecosystem & Bindings

- [x] **TypeScript** — `packages/ts/core/` wraps WASM with full TypeScript types; `@zpl-toolchain/core` npm package; 5 functions (parse, parseWithTables, validate, format, explain)
- [x] **Python** — `crates/python/` with PyO3 + maturin; `zpl-toolchain` on PyPI; 5 functions returning JSON strings; `abi3` for broad compat (Python 3.9+)
- [x] **C FFI** — `crates/ffi/` with cdylib+staticlib; `cbindgen.toml` for header generation; `zpl_parse`, `zpl_validate`, `zpl_format`, `zpl_explain`, `zpl_free`; foundation for Go + .NET
- [x] **.NET** — `packages/dotnet/ZplToolchain/` with P/Invoke wrapper; C# record types mirroring AST/Diagnostic; .NET Standard 2.0 for broad compat
- [x] **Go** — `packages/go/zpltoolchain/` with cgo wrapper; Go structs mirroring AST/Diagnostic types; JSON unmarshal from C FFI
- [x] **CI: ecosystem builds** — WASM build (wasm-pack), Python wheel (maturin), C FFI build (cargo), WASM size check (warn >500KB gzipped) in `.github/workflows/ci.yml`

### Tier C: Consolidation & Organization (cross-cutting improvements)

#### Tier C1 (High Impact)

- [x] **Diagnostic codes — single source of truth** — `spec/diagnostics.jsonc` is now the canonical source with `constName` field; `crates/diagnostics/build.rs` auto-generates `codes.rs` constants and `explain()` match arms at build time; fixed missing `ZPL.PARSER.1301`; `codes.rs` reduced from 148 lines to 7 lines (include!)
- [x] **Extract `bindings-common` crate** — `crates/bindings-common/` centralizes `embedded_tables()`, parse/validate/format/explain workflows, indent parsing, and `build.rs` table embedding; FFI/WASM/Python reduced to thin type-conversion wrappers; eliminated ~600 lines of duplication
- [x] **Test organization** — centralized helpers (`extract_codes`, `find_args`, `find_diag`, severity checks, profile fixtures) in `common/mod.rs`; standardized all test files to `common::TABLES`; split 111 validator tests from `parser.rs` into new `validator.rs`; `parser.rs` now has 61 focused parsing tests

#### Tier C2 (Medium Impact)

- [x] **Generated artifacts cleanup** — removed redundant standalone `opcode_trie.json` (was already embedded in `parser_tables.json`); documented `constraints_bundle.json` and `docs_bundle.json` as generated-but-not-consumed-at-runtime artifacts available for external tooling (IDE plugins, documentation generators)
- [x] **Core crate public API re-exports** — added top-level re-exports to `crates/core/src/lib.rs` for `parse_str`, `parse_with_tables`, `ParseResult`, `Ast`, `Label`, `Node`, `ArgSlot`, `Presence`, `emit_zpl`, `EmitConfig`, `Indent`, `Diagnostic`, `Span`, `Severity`, `codes`, `validate_with_profile`, `ValidationResult`; `bindings-common` updated to use flat imports
- [x] **Workspace dependency consistency** — `wasm-bindgen`, `serde-wasm-bindgen`, `pyo3` now use `{ workspace = true }`; removed unused `zpl_toolchain_spec` dep from `spec-compiler`
- [x] **Documentation consolidation** — root README slimmed from 327→225 lines: removed "Recent additions" (link to CHANGELOG), condensed spec authoring/profiles/bindings sections with links to dedicated docs; merged development/contributing sections; PROFILE_GUIDE diagnostic table replaced with cross-reference to DIAGNOSTIC_CODES.md

#### Tier C3 (Lower Impact / Polish)

- [x] **CI workflow deduplication** — extracted `.github/actions/setup-rust/` composite action (Rust toolchain + cache + parser tables); `ci.yml` and `release.yml` updated (8 uses); standardized cache keys with `ci-`/`release-` prefixes; fixed formatting bug in FFI verify step
- [x] **Production `unwrap()` cleanup** — `generate_docs_bundle()` and `generate_constraints_bundle()` now return `Result<Value>` with `?` propagation (4 `unwrap()` eliminated); `unreachable!()` in `parser.rs` now includes descriptive message
- [x] **Profile validation strengthening** — `id`, `schema_version`, `dpi` now required (non-`Option`); `load_profile_from_str()` validates DPI (100–600), speed range (1–14), darkness (0–30), page/memory positivity; validator simplified; 10 new tests

### Pre-Release Foundation Hardening

#### Completed (Tier A — Quick Wins)

- [x] **`ConstraintKind` Display impl** — added `Display` impl; replaced JSON-serialization-to-string hack in `pipeline.rs` with `.to_string()`
- [x] **Diagnostic span invariants in fuzz tests** — `assert_invariants()` now validates `span.start <= span.end` and `span.end <= input.len()` for all diagnostics
- [x] **Defensive `serde(rename_all = "camelCase")`** — added to 9 structs in `spec-tables`: `ParserTables`, `CommandEntry`, `Effects`, `OpcodeTrieNode`, `ProfileConstraint`, `ConditionalRange`, `RoundingPolicy`, `ConditionalRounding`, `Constraint`; tables regenerated
- [x] **Remove unused `anyhow` from core crate** — was listed as dependency but never used in source
- [x] **Make `Span` required (non-Option) on Node variants** — `span: Option<Span>` → `span: Span` on all 4 `Node` variants; `strip_spans()` refactored to use sentinel `Span::new(0, 0)`; removed `#[serde(skip_serializing_if)]` from span fields
- [x] **`#[non_exhaustive]` on `Node` enum** — future-proofs against adding new node variants
- [x] **String-typed fields to enums** — created `CommandScope`, `CommandCategory`, `Stability` enums with `Display` impls and serde support; replaced `Option<String>` fields on `CommandEntry`; gives exhaustiveness checking and IDE support

#### Completed (Tier B — Structural Improvements)

- [x] **Profile error type** — created `ProfileError` enum with `thiserror` derive (`InvalidJson`, `InvalidField`); replaced `anyhow` in profile crate; `load_profile_from_str()` returns typed errors
- [x] **`ValidationContext` struct** — created `ValidationContext` (immutable) + `CommandCtx` (per-command) structs; eliminated all 9 `#[allow(clippy::too_many_arguments)]` suppressions in validator
- [x] **Split `validate_semantic_state`** — 285-line function split into `validate_field_number`, `validate_position_bounds`, `validate_font_reference`, `validate_media_modes`, `validate_gf_data_length`; main function is now a thin dispatcher
- [x] **Split `validate_cross_field`** — 350-line function in spec-compiler split into `validate_duplicate_opcodes`, `validate_command_arity`, `validate_signature_linkage`, `validate_arg_hygiene`, `validate_command_constraints_spec`, `validate_composites_linkage`, `validate_effects`, `validate_profile_constraints_spec`; extracted `visit_args` helper
- [x] **Doc comments + `#![warn(missing_docs)]`** — added doc comments to all public types/functions in `core`, `spec-tables`, `diagnostics` crates; enabled `#![warn(missing_docs)]` with zero warnings

#### Deferred (Tier C — Lower Priority or High Effort)

- [ ] **Validator module split** (1,400 lines → 8 sub-modules) — high effort, medium benefit; do after ValidationContext/splits settle
- [ ] **`Cow<'a, str>` in AST nodes** — high churn (~100 sites), limited benefit since FFI serialization needs owned strings
- [ ] **JSON Schema validation in spec-compiler** — needs new dependency; serde + cross-field validation already catches issues. *Note:* A `crates/spec/` crate previously existed that embedded the schema via `include_str!("spec/schema/zpl-spec.schema.jsonc")` for this purpose. It was removed (unused, 9 lines) — if needed, the spec-compiler can `include_str!` the schema directly or read it from disk (it already reads spec files from disk). The schema file itself remains at `spec/schema/zpl-spec.schema.jsonc`.
- [ ] **Parser `parse_command()` sub-module split** — moderate benefit but parser is well-structured
- [ ] **Performance optimizations** — slice-based args, lazy `^FH` decode, raw/field streaming, fixed-point rounding; needs profiling data first
- [ ] **Test decoupling from generated files** — high effort (200+ test cases), low urgency; use in-memory fixtures or build-script dependency ordering

#### Print Client — Follow-up Items

- [x] **Integration tests with mock TCP server** — 10 tests: connect, send, multi-label, raw bytes, ~HS query/parse, ~HI query/parse, reconnect, connection error, empty send, large payload (100KB)
- [x] **TypeScript unit tests** — 71 tests across 6 suites: browser (19), proxy (22), printValidated (13), batch (7), status (6), types (4)
- [x] **CLI print tests** — 11 tests via `assert_cmd`: help output, required args, dry-run (pretty + JSON for TCP/USB/serial), serial-USB conflict, missing file, validation with/without tables
- [x] **Proxy wildcard allowlist** — `isPrinterAllowed` now supports `*` glob patterns (e.g., `"192.168.1.*"`); exact match fast path, regex conversion for patterns
- [x] **`RetryPrinter` reconnection** — added `Reconnectable` trait, `ReconnectRetryPrinter<P>` wrapper that calls `reconnect()` between retry attempts; `TcpPrinter` implements `Reconnectable`; 4 tests
- [x] **WebSocket proxy endpoint** — `createPrintProxy` accepts WebSocket connections on the same port for persistent bidirectional communication; JSON message protocol with `print` and `status` message types
- [x] **WebSocket security hardening** — `wsSend()` readyState guard prevents crashes from sends to closed connections; `maxPayload` enforcement matches HTTP limits; `verifyClient` origin validation; 30s ping/pong keepalive with `terminate()` for idle connections
- [x] **`printValidated` unit tests** — `_processValidationResult()` extracted for testability; 13 validation-logic unit tests
- [x] **Go print bindings** — `Print()` and `QueryStatus()` functions in `packages/go/zpltoolchain/` for sending ZPL over TCP and querying `~HS` printer status via the C FFI
- [x] **.NET print bindings** — `Zpl.Print()` and `Zpl.QueryStatus()` in `packages/dotnet/ZplToolchain/` for sending ZPL over TCP and querying `~HS` printer status via P/Invoke
- [x] **TypeScript batch API** — `printBatch()` standalone function and `TcpPrinter.printBatch(labels, opts?, onProgress?)` / `TcpPrinter.waitForCompletion()` methods; `BatchOptions`, `BatchProgress`, `BatchResult` types; 7 unit tests
- [x] **Preflight diagnostics** — ZPL2308 (graphics bounds — `^GF` exceeds printable area), ZPL2309 (graphics memory — total `^GF` memory exceeds printer RAM), ZPL2310 (missing explicit `^PW`/`^LL` label dimension commands); 10 new validator tests
- [x] **TcpPrinter write queue** — `print()` serializes concurrent writes through an internal promise-chain mutex; `close()` drains the queue before teardown
- [x] **Proxy `allowedPorts`** — new `ProxyConfig.allowedPorts` option (default `[9100]`) restricts destination ports; empty array = deny all
- [x] **Proxy `maxConnections`** — new `ProxyConfig.maxConnections` option (default `50`) limits concurrent WebSocket connections; rejected with 503 at capacity
- [x] **Proxy CORS single-string fix** — `verifyClient` and `setCorsHeaders` now correctly handle single-string CORS configurations
- [x] **Proxy allowlist pre-compilation** — glob-to-regex patterns compiled once at startup; `[^.]*` prevents ReDoS
- [x] **`^MU` unit normalization** — `^FO`/`^FT` positions and `^PW`/`^LL` dimensions normalized to dots for correct ZPL2302/ZPL2308 bounds checks with non-dot units
- [x] **`#[non_exhaustive]` on structs** — `HostStatus`, `PrinterInfo`, `BatchProgress`, `BatchResult` for semver safety
- [x] **`SockRef` keepalive** — replaced `Socket::from(stream.try_clone()?)` with `SockRef::from()` for robustness
- [x] **Socket resource cleanup** — `tcpQuery` socket leak fix; `TcpPrinter.close()` 2s force-destroy; `isReachable()` 1s force-destroy
- [x] **`^GF` math hardening** — `saturating_mul` for `bytes_per_row * 8`; `div_ceil` for `graphic_field_count / bytes_per_row`
- [x] **Zebra Browser Print tests** — 19 unit tests covering `isAvailable()`, `discover()`, `print()`, `getStatus()`
- [x] **Rust batch/completion tests** — 8 tests for `send_batch_with_status` and `wait_for_completion` (including timeout and formats_in_buffer)

#### Print Client — Future Items

- [ ] **mDNS/Bonjour printer discovery** (Phase 5c) — discover network printers via mDNS/Bonjour service advertising; zero-configuration printer finding for local networks
- [ ] **Virtual printer emulator** (Phase 5c) — listen on port 9100, accept ZPL, render via the native renderer; useful for testing and development without physical hardware
- [ ] **BLE transport** (deferred) — Bluetooth Low Energy is designed for status monitoring, not bulk data transfer; low throughput makes it unsuitable for label printing; SPP (serial) remains the recommended Bluetooth transport
- [x] **`RetryPrinter` accessor methods** — added `inner()` / `inner_mut()` for symmetry with `ReconnectRetryPrinter`
- [x] **CLI `--timeout 0` rejection** — `value_parser` range constraint rejects zero-value timeout at the argument level
- [x] **TS input validation** — `resolveConfig()` validates `host`, `port` (1–65535), and `timeout` for clear error messages
- [x] **Proxy HTTP connection limits** — `server.maxConnections` set to match the WebSocket `maxConnections` limit
- [x] **Proxy error message sanitization** — HTTP and WebSocket error paths return generic messages, not internal TCP error details
- [x] **Proxy body-read timeout** — 30s deadline on `readBody()` to mitigate slow-loris attacks
- [x] **C# `NodeJsonConverter.Write` fix** — serializes via `NodeDto` to prevent `StackOverflowException` from infinite recursion
- [ ] **FFI `catch_unwind` guard** — wrap all `extern "C"` FFI functions in `std::panic::catch_unwind` to prevent undefined behavior if Rust code panics across the FFI boundary
- [ ] **FFI configurable timeouts** — add optional `timeout_ms` / `config_json` parameters to `print_zpl` / `query_printer_status` for Go/C#/.NET consumers who need to tune connection settings
- [ ] **FFI `query_info` exposure** — expose `~HI` (printer identification) query in bindings-common, FFI, Go, and C# (currently only `~HS` is exposed)
- [ ] **Go/C# typed `HostStatus`** — `QueryStatus()` currently returns raw JSON string; add typed struct matching the 24-field Rust `HostStatus`
- [ ] **Proxy body-read timeout** — add a deadline to `readBody()` to mitigate slow-loris attacks; configure `server.requestTimeout` / `server.headersTimeout`
- [ ] **Proxy HTTP connection limits** — set `server.maxConnections` to match the WebSocket `maxConnections` limit
- [ ] **Proxy per-client WS rate limiting** — add per-connection concurrency cap to prevent a single WebSocket client from flooding the proxy with requests
- [ ] **Proxy WS correlation IDs** — accept optional `id` field in WS messages and echo it back for request-response correlation on concurrent WS usage
- [ ] **Proxy error message sanitization** — avoid leaking internal IP addresses and error details to HTTP/WS clients; return generic messages on TCP errors
- [ ] **TS `AbortSignal` support** — accept `AbortSignal` on `print()`, `printBatch()`, `waitForCompletion()` for cooperative cancellation (Node.js 18+)
- [ ] **TS `BatchResult` with partial error** — when `printBatch` fails mid-batch, include `{ sent, total, error }` so callers know which labels were sent
- [ ] **TS input validation** — validate `host`, `port`, `timeout` in `resolveConfig()` for clear error messages instead of raw Node.js socket errors
- [ ] **CLI stdin support** — accept `-` as a file path to read ZPL from stdin (common CLI convention)
- [ ] **CLI `--timeout 0` rejection** — reject zero-value timeout at the argument level to prevent confusing immediate-failure behavior
- [ ] **CLI JSON error envelope consistency** — ensure all error paths (`anyhow::bail!`, file-read `?`, tables loading) produce JSON error envelopes when `--output json` is active
- [ ] **Preflight `^MU` + bounds tests** — add tests for `^MUI`/`^MUM` interaction with ZPL2302 position bounds and ZPL2308 graphic bounds checks
- [ ] **Preflight `^FT` bounds test** — add test for `^FT` position bounds (currently only `^FO` is tested)
- [ ] **Preflight boundary comparison** — evaluate changing ZPL2302 from `>` to `>=` at exact boundary (position == label dimension)
- [ ] **Feature-gated `serde`** — make `serde` an optional feature on print-client for users who only need the transport layer
- [ ] **Mock TCP server for TS tests** — create a mock TCP server to enable happy-path testing of `print()`, `TcpPrinter`, retry logic, and proxy forwarding
- [ ] **Proxy security test coverage** — add tests for `allowedPorts`, `maxConnections`, CORS origin filtering, `maxPayloadSize`, and `/status` HTTP endpoint

### Tier 5: Tech Debt & Future Considerations

- [x] `ConstraintKind` Display impl — completed in Pre-Release Foundation Hardening (Tier A)
- [x] Defensive `#[serde(rename_all = "camelCase")]` on leaf structs — completed in Pre-Release Foundation Hardening (Tier A)
- [x] Make `Option<Span>` non-optional on `Node` variants — completed in Pre-Release Foundation Hardening (Tier A)
- [x] Add diagnostic span invariant checks to fuzz `assert_invariants` — completed in Pre-Release Foundation Hardening (Tier A)
- [x] Add `#[non_exhaustive]` on `Severity` — future-proofs against adding variants; wildcard arms added to CLI match sites
- [x] Extract `valid_kinds` list from shared constant — `ConstraintKind::ALL` is now the single source of truth; a `constraint_kinds_match_schema` test in spec-compiler validates the JSONC schema stays in sync
- [x] `LazyLock` for `load_tables()` in snapshot tests — already done (uses `std::sync::LazyLock` in `tests/common/mod.rs`)
- [ ] Cross-target parity test — native vs WASM parse/validate outputs
- [x] Document `^CI` variable-length remap limitation — documented in `^CI.jsonc` constraint note
- [ ] Schema v2 refactor — normalized param schemas, command maps, structured effects, DSL-based constraints (see `docs/research/schema-v2-proposal.md`). *Deferred in ROADMAP.md — revisit when current schema becomes a bottleneck.*
- [ ] Schema parity gaps — `argUnion` examples, richer constraints
- ~~Query/response schemas for tilde commands (`~HM`, `~HS`, `~HQES`)~~ — *Dropped per ROADMAP.md. The print client (Phase 5a) can send `~HS` and parse the response without a formal schema system.*
- [ ] Signature builder for exact `Format:` string emission
- ~~RenderContract constants (stub)~~ — *Archived. Vestige of original plan; superseded by the renderer's direct AST consumption approach (ROADMAP Phase 3).*

---

## Completed Work (detailed history)

### Architecture / Elegance / Maintainability

- [x] **`effects` schema normalization** — removed `oneOf` array variant from JSONC schema; `effects` now only accepts the object format (`{ "sets": [...] }`) consistent with Rust struct and all 40+ spec files
- [x] **`^A` spec-driven split rule** — added `SplitRule` struct and `splitRule` field to `Signature` schema/struct; parser now uses generic split logic instead of hardcoded `if code == "^A"`; `^A.jsonc` uses `splitRule: { paramIndex: 0, charCounts: [1, 1] }`
- [x] Remove unused workspace dependencies (`logos`, `chumsky`, `blake3`, `schemars`)
- [x] Factor spec-compiler `build()` into separate pure functions (`load`, `validate`, `generate_tables`, `generate_docs_bundle`, `generate_constraints_bundle`, `generate_coverage`)
- [x] Type the spec-compiler pipeline with `SourceCommand` struct instead of raw `serde_json::Value`
- [x] Generalize profile gate checks (`^PW`/`^LL`) into data-driven `profileConstraint` on arg specs
- [x] Formalize constraint `expr` DSL with parsed/validated grammar in spec-compiler
- [x] Remove hardcoded `^FD`/`^FV` empty-data warning; express through spec instead
- [x] Remove unused dependencies (`thiserror` from core, `jsonschema` from spec-compiler)
- [x] Fix `strip_jsonc` to handle escaped quotes (`\"`) and prevent JSON corruption
- [x] Fix `chars().next().unwrap()` panic on multi-byte UTF-8 in `^A` parser handling
- [x] Fix fragile `std::ptr::eq` node index lookup in validator — use `enumerate()`
- [x] Remove dead code (`_saw_xa`, trivial `predicate_when_mode_matches` alias, redundant `.clone()` on `Copy` type)
- [x] Box `ArgUnion::Single(Arg)` → `Single(Box<Arg>)` to resolve clippy `large_enum_variant` warning
- [x] Apply all clippy auto-fixes (collapsible `if` blocks, `is_none_or`, merged parser branches)
- [x] Add duplicate opcode detection across spec files in pipeline validation
- [x] Add warning output for `load_master_codes` file failures (was silently returning empty)
- [x] Remove redundant `docs_bundle` embedding from `parser_tables.json` (separate `docs_bundle.json` already exists)
- [x] Fix master codes file path (`docs/public/schema/zpl-commands.jsonc`)
- [x] Fix coverage counting to register all codes from multi-code specs (not just canonical)
- [x] Build `HashMap`/`HashSet` lookup once in `Parser::new()` instead of per-command O(n) scan
- [x] Reset `fh_active` on field data interruption (was only reset on `^FS`, not on interrupted exits)
- [x] Separate diagnostic code `ZPL.PARSER.1001` into `1001` (invalid syntax) and `1002` (unknown command)
- [x] Add `DiagSpan` to the shared `Diagnostic` type in `diagnostics` crate
- [x] Propagate spans from AST through validator diagnostics and CLI merge
- [x] Fix `emptyData` constraint to check following `FieldData` nodes (not just Command args)
- [x] Interruption diagnostic now shows full interrupting command code (was showing only `^` or `~`)
- [x] Add `debug_assert!(end >= start)` to `Span::new()` for safety
- [x] Add ASCII safety comment to `recognize_opcode` byte-as-char casting
- [x] Add `field_data` and `raw_payload` to JSONC spec schema
- [x] Fix `^A` — add `before:^FD|^FV` order constraint
- [x] Fix `^BQ` — rename `field_orientation` to `orientation` for consistency
- [x] **[CRITICAL]** `generate_tables()` builds `CommandEntry` directly — refactored from ad-hoc `serde_json::json!` to typed `Vec<CommandEntry>` construction with compile-time field guarantees; added missing fields (`name`, `category`, `since`, `deprecated`, etc.) to `CommandEntry`
- [x] **[CRITICAL]** Structural flags added to JSONC schema — all six flags (`opens_field`, `closes_field`, `hex_escape_modifier`, `field_number`, `serialization`, `requires_field`) now defined in `zpl-spec.schema.jsonc`; `effects` schema updated to use `sets` (matching actual spec files)
- [x] **[CRITICAL]** Parser uses spec-driven flags — `code == "^FS"` replaced with `ce.closes_field`, `code == "^FH"` replaced with `ce.hex_escape_modifier`; consistent with data-driven philosophy
- [x] `Constraint.severity` uses `ConstraintSeverity` enum — replaces `Option<String>` with typed enum; `map_sev()` now takes `Option<&ConstraintSeverity>`
- [x] `CommandEntry.plane` uses `Plane` enum — replaces `Option<String>` with typed `Plane` enum with `Display` impl; validator uses enum matching for scope validation
- [x] `Diagnostic.id` type safety — added `all_diagnostic_ids_have_explanations` test that validates all 37 diagnostic codes have matching `explain()` entries; catches typos at test time
- [x] **Type 4 `SourceCommand` fields** — `signature`, `args`, `constraints`, `effects` now use typed `Signature`, `Vec<ArgUnion>`, `Vec<Constraint>`, `Effects` structs; catches schema mismatches at deserialization; fixed silent `allowEmptyTrailing` bug via `#[serde(rename_all = "camelCase")]` on `Signature` and `Arg`
- [x] **Type remaining `SourceCommand` fields** — `signatureOverrides` → `HashMap<String, Signature>`, `composites` → `Vec<Composite>`, `examples` → `Vec<Example>`; `defaults` and `extras` remain as `Value` (freeform/vendor data, no schema-defined structure)
- [x] **Make `Constraint.kind` and `ProfileConstraint.op` enums** — `ConstraintKind`, `ComparisonOp`, and `RoundingMode` enums with serde support; enables exhaustiveness checking at compile time
- [x] Unify diagnostic type — parser and validator now both use `Diagnostic` from `zpl_toolchain_diagnostics`; `SyntaxDiag` removed; `PartialEq` derived on `Severity`/`Diagnostic`; convenience constructors `Diagnostic::error/warn/info()`; dead `DiagSpan` alias removed; validator uses `crate::grammar::diag` re-export consistently
- [x] Deduplicate unknown-command diagnostics — removed `ZPL1301`, parser `ZPL.PARSER.1002` handles it; validator now skips unknown commands entirely
- [x] Add type validation for `int`/`float` args — new codes `ZPL1107` (non-integer) and `ZPL1108` (non-numeric)
- [x] Add structural validation — `ZPL2201` (missing field origin), `ZPL2202` (empty label), `ZPL2203` (overlapping fields), `ZPL2204` (orphaned ^FS), `ZPL2205` (scope violation)
- [x] Emit `note` constraints as `ZPL3001` Info diagnostics — 84 previously silent constraints now visible
- [x] Add `plane`/`scope` fields to `CommandEntry` — enables scope validation at runtime
- [x] **Extract `FieldTracker` struct** — field-tracking state machine (`open`, `has_fh`, `has_fn`, `has_serial`, `start_idx`) extracted into a struct with `process_command()` and `validate_field_close()` methods; main loop reduced by ~90 lines
- [x] **Extract `enum_contains` helper** — broke up dense 180+ char single-line condition in `select_effective_arg`; reused in enum validation
- [x] **Reformat long single-line statements** — min/max length checks reformatted to multi-line for readability
- [x] **Add "why" comments** — rounding epsilon logic now explains both conditions for floating-point imprecision handling
- [x] **DRY: extract `first_arg_f64(args)` helper** — shared by `^PW` and `^LL` tracking blocks
- [x] **Unify `Span`/`DiagSpan` types** — canonical `Span` now lives in `diagnostics` crate; `ast.rs` re-exports it; removed `to_diag_span()` conversion function; `DiagSpan` kept as backward-compatible alias
- [x] **Add `Eq` derive to `Diagnostic`** — all fields (`String`, `Severity`, `Option<Span>`) are `Eq`; enables use in `HashSet`/exhaustive equality contexts
- [x] **Add `#[derive(Debug)]` to `Token<'a>`** — was missing despite `TokKind` having `Debug`; essential for test assertions and debugging
- [x] **Add UTF-8 safety comment to lexer `tokenize()`** — documents why `b[i] as char` is safe for delimiter detection (all delimiters are ASCII, continuation bytes are >= 0x80)
- [x] **Remove redundant `self.nodes = Vec::new()` in parser** — `std::mem::take` already leaves an empty `Vec`
- [x] **Eliminate `.clone()` in `parse_raw_data()`** — refactored to use `std::mem::replace` like the EOF cleanup path; mode state moved out instead of borrowed+cloned
- [x] **Eliminate `.clone()` in EOF raw data cleanup** — reordered to emit diagnostic first (borrows `&command`), then move `command` into node
- [x] **Fix `truncate()` UTF-8 panic in fuzz tests** — `&s[..max]` could slice mid-character; now finds largest `is_char_boundary` offset <= max
- [x] **Optimize `code.clone()` in `parse_command()`** — restructured to move `code` directly into `Node::Command` on the common (non-raw-payload) path; `.clone()` only occurs for raw payload commands that need `code` in both the node and `Mode::RawData`
- [x] **Upgrade `Span::new` to full `assert!`** — replaced `debug_assert!` with `assert!` so inverted spans (`end < start`) panic in release builds too, preventing silent corruption downstream
- [x] **Add diagnostics crate unit tests** — 15 in-crate tests covering Span construction (valid, empty, inverted-panics), Severity Display, Diagnostic constructors (error/warn/info), Diagnostic Display formatting, explain() method (known/unknown), exhaustiveness (all codes have explanations), PartialEq/Eq behavior, and serde round-trip with skip_serializing_if verification
- [x] **Consolidate split `#[derive]` on `Presence` enum** — merged two `#[derive(...)]` blocks into one
- [x] **Zero-allocation lexer: `Token<'a>` with `&'a str`** — `Token.text` now borrows directly from input instead of heap-allocating per token; eliminates O(n) `String` allocations in the tokenizer; `Parser` already had `'a` lifetime so the change was contained to `lexer.rs` (Token struct + tokenize fn) and 5 usage sites in `parser.rs`; fuzz tests confirmed no regressions
- [x] **Cache `code_set()` and `cmd_lookup` on `ParserTables`** — added `OnceLock`-based lazy caching; `code_set()` returns `&HashSet<String>`, new `cmd_by_code()` returns `Option<&CommandEntry>` in O(1); parser and validator updated to use cached lookups; per-call `HashMap` rebuilds eliminated
- [x] **Extract validator into focused sub-functions** — `validate_command_args()`, `validate_command_constraints()`, `validate_semantic_state()`; main function reduced from ~550 to ~170 lines
- [x] **Consolidate validator label passes** — reduced from 4 passes (main, empty-check, structural, scope) to 1 unified pass per label
- [x] **Standardize validation patterns** — replaced `skip_value_checks` boolean flag with `type_valid` match pattern for cleaner type → range → constraint flow
- [x] **Make validator structural checks spec-driven** — replaced hardcoded `match code.as_str()` structural block with spec-driven flags (`opens_field`, `closes_field`, `hex_escape_modifier`, `field_number`, `serialization`, `requires_field`) on `CommandEntry`; flags flow from JSONC spec → spec-compiler → generated tables → validator. Removed `zpl_commands.rs` constants module (superseded by spec-driven approach)
- [x] **Performance: O(n) order constraint checking** — replaced O(n²) preceding-nodes scan with incremental `seen_codes: HashSet<&str>`; each order check is now O(1) lookup
- [x] **Performance: reduce String cloning** — `record_producer` no longer clones arg values; `LabelState::producers_seen` simplified from `HashMap<String, Vec<Option<String>>>` to `HashSet<String>`
- [x] **Extract `SCHEMA_VERSION` constant** — shared `pub const SCHEMA_VERSION` in spec-compiler crate; `check` and `build` commands use it
- [x] Replace `resolve_profile_field` hardcoded match with `OnceLock` registry — declarative `PROFILE_FIELD_REGISTRY` with O(1) lookup; single line per field
- [x] **Add `Diagnostic` ID constants module** — `diagnostics::codes` with 31 constants; `explain()`, parser, and validator all use constants instead of string literals; compile-time typo detection and IDE autocomplete
- [x] **Pipeline `Plane` conversion** — typed `SourceCommand.plane` as `Option<Plane>` directly; eliminated JSON roundtrip in `pipeline.rs`; invalid plane values now cause proper serde errors; deduplicated schema version check
- [x] **Add `Display` impl for `Severity` and `Diagnostic`** — `Severity` outputs lowercase; `Diagnostic` formats as `severity[id]: message`
- [x] **Add `Diagnostic::explain()` convenience method** — wraps `explain(&self.id)` for more discoverable API
- [x] `Diagnostic.id` changed to `Cow<'static, str>` — all `codes::` constants now create `Cow::Borrowed` (zero allocation); `impl Into<Cow<'static, str>>` on constructors maintains backward compat
- [x] **Tests: migrate string literal diagnostic IDs to `codes::` constants** — ~150+ replacements across 31 diagnostic ID patterns; compile-time typo detection now covers all test assertions
- [x] **Tests: extract shared `load_tables()` into `tests/common/mod.rs` with `LazyLock`** — eliminated 3 duplicate `load_tables()` functions; tables loaded once per test binary via `std::sync::LazyLock`
- [x] **Remove unused `serde_json` dependency from diagnostics crate** — no code in the crate uses it
- [x] **Remove dead `TokKind::Other` variant** — lexer never produces it; removed from enum and parser match arm
- [x] **Deduplicate `Span` re-export** — consolidated to single canonical re-export through `diag.rs`; `ast.rs` now uses private import
- [x] **Trie key type optimization** — `OpcodeTrieNode.children` now uses `HashMap<char, …>` with custom serde; eliminates `ch.to_string()` allocation per trie lookup
- [x] **Add `skip_serializing_if = "Option::is_none"` to `ArgSlot::value`** — now consistent with `key` field; golden snapshots regenerated
- [x] **Deduplicate schema version check in spec-compiler main.rs** — extracted `warn_schema_versions()` helper
- [x] **Simplify `validate_command_args` length checks** — removed redundant `slot.value.as_ref()` rebinds; uses existing `val` binding
- [x] **Decompose `validate_command_args` into focused sub-functions** — extracted `validate_arg_value`, `validate_arg_range`, `validate_arg_length`, `validate_arg_rounding`, `validate_arg_profile_constraint`, `validate_arg_enum_gates`; main function reduced from ~200 to ~40 lines
- [x] **Precompute label-wide code set for `Requires`/`Incompatible` constraints** — O(1) per constraint via precomputed `HashSet<&str>`; removed `any_target_matches()` O(n) helper
- [x] **Reformat `EnumValue::Object` variant** — expanded single dense line to multi-line for readability
- [x] **Add `^GF` positional index comments** — documented arg layout and added inline comments on each positional reference
- [x] **Add `type_valid` design comment** — documents why invalid enums intentionally keep `type_valid = true`
- [x] **Create `docs/DIAGNOSTIC_CODES.md`** — comprehensive reference of all 40 diagnostic codes with descriptions, severities, examples, and fix guidance
- [x] **Create `spec/diagnostics.jsonc`** — machine-readable diagnostic spec for auto-doc generation and coverage checking
- [x] Fix `^LL` vs profile height validation — now handled by generic `profileConstraint` (ZPL1401)
- [x] Fix parser `unreachable!()` in `parse_field_data()` — replaced with informative `unreachable!()` message
- [x] Add spans to parser diagnostics that had `span: None` — missing ^XZ now points to end of input; no-labels spans entire input

### Data Accuracy (spec files vs PDF)

- [x] Audit all 25 original spec files against the Zebra Programming Guide PDF
- [x] Fix `^BY` ratio `r`: should be `float` range `[2.0, 3.0]`, not `int` range `[1,3]`
- [x] Fix `^BC` mode `m`: should be `enum ["N","U","A","D"]`, not `int`
- [x] Fix `^A` font `f`: constrain to `A-Z, 0-9`; height range `[10, 32000]` for scalable; add `defaultFrom` for `^CF` and `^FW`
- [x] Fix `^MN`: enum values are incomplete (need `N`, `Y`, `W`, `M`, etc.)
- [x] Fix `^FO`: add justification param `z` (arity 3) for newer firmware
- [x] Fix `^CI`: needs specific valid code page values
- [x] Add `"default"` values from the PDF for all parameters across all spec files
- [x] Add `"defaultFrom"` where the PDF says "default: last accepted ^XX" or "value set by ^XX"
- [x] Fix `^BY` scope from `"field"` to `"label"` — state persists until next `^BY`
- [x] Fix `^BB` — add `defaultFrom: "^FW"` for orientation, `defaultFrom: "^BY"` for height
- [x] Fix `^BR` — add `defaultFrom: "^FW"` for orientation, `defaultFrom: "^BY"` for height
- [x] Fix `^BT` — add `defaultFrom: "^BY"` for code39_height, micropdf417_row_height, code39_width, code39_ratio, micropdf417_width
- [x] Fix `^BR` — add `requires: ^BY` constraint
- [x] Fix `^BT` — add `requires: ^BY` constraint
- [x] Fix `^GS` — add `defaultFrom: "^FW"` for orientation, `defaultFrom: "^CF"` for height/width
- [x] Fix `^CI` — add `effects: { "sets": ["encoding.characterSet"] }`
- [x] Fix `^MU` — expand DPI enum values to `["150","200","300","600"]` for both params
- [x] `^BQ`/`^B0` magnification defaults are DPI-dependent (1/2/3/6 for 150/200/300/600 dpi) — implemented via `defaultByDpi` on both spec args

### Cross-Command State Model

- [x] Create `docs/STATE_MAP.md` documenting all state-setting commands and their consumers
- [x] Design schema/validator support for cross-command state (`effects.sets`, `defaultFrom`, state accumulator)
- [x] Implement cross-command state validation incrementally: `^BY`->barcodes, `^CF`->`^A`, `^FW`->orientations
- [x] Add `effects.sets` to `^CI` for encoding state
- [x] Add `effects.sets` to `^CC`/`^CD`/`^CT` for parser prefix state
- [x] Add `effects.sets` to `^MD`, `^MM`, `^MT`, `^PR`, `^PM`, `^MU`, `^CW`, `^CM`, `^FR` for device/session/field state
- [x] Add spec-compiler validation: warn when `defaultFrom` references a command without `effects`

### Device & Unit State

- [x] **Device-level state tracking** — added `DeviceState` struct with `Units` enum persisting across labels; session-scoped commands (`^MU`) update device state; `validate_with_profile()` creates `DeviceState` before label loop
- [x] **`^MU` unit conversion** — `convert_to_dots()` function converts inches/mm to dots using DPI; `validate_arg_range()` converts user values before range comparison when `unit: "dots"` args have non-dot units active; 3 new tests

### Semantic Validation

- [x] **Barcode `^FD` data format validation (ZPL2401, ZPL2402)** — spec-driven `fieldDataRules` on 29 barcode spec files, each audited against the ZPL II Programming Guide PDF; `FieldTracker` tracks active barcode command and its rules; `validate_barcode_field_data()` validates character set (`ZPL2401`) and length/parity (`ZPL2402`) at `^FS`; `char_in_set()` parses compact charset notation (ASCII-only); full-ASCII symbologies (`^BA`, `^B4`, `^BD`) correctly omit character set restriction; type-dependent barcodes (`^BR`, `^BZ`, `^BX`) use notes-only rules; `maxLength` added for capacity-limited symbologies (`^B7`, `^BF`, `^BX`, `^BZ`, `^BT`, `^BD`); `^BK` characterSet corrected (A-D are start/stop params, not data); 8 new tests
- [x] Duplicate `^FN` field number detection (ZPL2301) — warn when same number reused in a label
- [x] Position bounds checking (ZPL2302) — `^FO`/`^FT` positions vs `^PW`/`^LL`/profile bounds
- [x] Font reference validation (ZPL2303) — `^A` font vs built-in (A-Z, 0-9) + `^CW`-loaded fonts
- [x] `^FH` hex escape validation (ZPL2304) — validate `_XX` sequences have valid hex digits in field data
- [x] Redundant state-setting detection (ZPL2305) — warn when producer overrides without consumer
- [x] `^SN`/`^SF` serialization without `^FN` (ZPL2306) — warn on serialization without field number
- [x] `^GF` data length validation (ZPL2307) — compare actual data chars to declared `binary_byte_count` for ASCII hex (×2) and binary (×1) formats; compressed (C) skipped

### Raw/Field Data Handling

- [x] Write ADR for raw/field data handling (`^GF` payloads, `^FD` escapes, trivia preservation)
- [x] Implement `RawData` node creation in parser for `^GF`/`~DG`/`~DY`/`~DB`
- [x] Implement field data mode in parser for `^FD`/`^FV` (content until `^FS`, `^FH` hex escapes)
- [x] Implement `Trivia` node creation for comments and whitespace (round-tripping support)

### Command Coverage Expansion

- [x] Add specs for `^CF` (font defaults) and `^FW` (field orientation) with `effects`
- [x] Add specs for all barcode commands: `^B0`–`^BZ` (26 commands)
- [x] Add specs for all print/label/media control: `^PQ`, `^PR`, `^MM`, `^MT`, `^MD`, `^MU`, `^PM`, `^LT`, `^LS`, `^ML`
- [x] Add specs for text/font/field commands: `^A@`, `^FR`, `^FP`, `^FN`, `^FX`, `^TB`, `^PA`
- [x] Add specs for prefix/config commands: `^CC`/`~CC`, `^CD`/`~CD`, `^CT`/`~CT`, `^CW`, `^CM`
- [x] Add specs for graphics commands: `^GC`, `^GD`, `^GE`, `^GS`
- [x] Add specs for file/resource commands: `^DF`, `^XF`, `^XG`, `^ID`, `^IL`, `^IM`, `^IS`, `~DG`, `~DY`
- [x] Add specs for host/diagnostic commands: `^HF`–`^HZ`, `~HB`–`~HU`
- [x] Add specs for job control commands: `^JB`–`^JW`, `~JA`–`~JX`
- [x] Add specs for download commands: `~DB`, `~DE`, `~DN`, `~DS`, `~DT`, `~DU`, `~EG`
- [x] Add specs for keyboard/KDU commands: `^KC`–`^KV`
- [x] Add specs for network/wireless commands: `^NB`–`^NW`, `~NC`, `~NR`, `~NT`
- [x] Add specs for RFID commands: `^RB`, `^RF`, `^RL`, `^RS`, `^RU`, `^RW`
- [x] Add specs for wireless commands: `^WA`–`^WX`, `~WC`, `~WL`, `~WQ`, `~WR`
- [x] Add specs for serialization commands: `^SF`, `^SN`
- [x] Add specs for settings/config commands: `^SI`–`^SZ`
- [x] Add specs for misc commands: `^CN`, `^CO`, `^CP`, `^FC`, `^FE`, `^FL`, `^FM`, `^MA`, `^MI`, `^MP`, `^MW`, `^TO`, `^XB`, `^XS`, `^ZZ`, `~KB`, `~RO`, `~SD`, `~TA`
- [x] Add print control: `^PF`, `^PH`/`~PH`, `^PN`, `^PP`/`~PP`, `~PL`, `~PM`, `~PS`
- [x] **100% coverage achieved: 223/223 commands (216 spec files)**
- [x] Audit all 45 arity-0 stubs against PDF — 44 confirmed truly parameterless, 1 updated (`^FX` now has `comment` arg)
- [x] Add `char` type validation (`ZPL1109`) — rejects multi-character values for `type: "char"` args (10 args across 8 commands)

### Parser Enhancements ("God Mode")

- [x] **Parser prefix/delimiter state tracking** — `^CC`/`~CC`/`^CT`/`~CT` prefix changes handled early in `parse_command()` with single-char extraction and re-tokenization of remaining input; `^CD`/`~CD` delimiter changes tracked in `Parser.delimiter`; `tokenize_with_config()` supports configurable prefix and delimiter characters (lexer correctly tokenizes new delimiter as `TokKind::Comma`); commands with non-comma signature joiners (`:`, `.`) correctly preserved under delimiter change; canonical leader mapping ensures consistent opcode lookup; 7 new tests
- [x] Fix `^FV` field tracking bug (parser only checked `^FD`, not `^FV`)
- [x] Implement field data mode for `^FD`/`^FV` — content until `^FS` preserved as single arg, commas not split
- [x] Add byte offset/span tracking to all AST nodes (`Span { start, end }`) for precise diagnostics
- [x] Implement `Trivia` node creation — comments (`;`) preserved as Trivia nodes
- [x] Refactor parser from boolean flags to explicit state machine (`Mode::Normal`, `Mode::FieldData`)
- [x] Extract mode-specific parsing into separate methods (`parse_normal`, `parse_command`, `parse_field_data`, etc.)
- [x] Add `Span` to `SyntaxDiag` for location-aware diagnostics
- [x] Lexer now emits `TokKind::Comment` tokens instead of silently stripping them
- [x] **Implement raw payload mode** — `Mode::RawData` state machine variant for `^GF`/`~DG`/`~DY`/`~DB`; collects multi-line data until next command leader; emits `Node::RawData` with data and spans; no spurious empty nodes for inline-only data; EOF diagnostic for unterminated raw data; refactored EOF cleanup to `match` for mutual exclusivity; 9 tests

### Testing

- [x] Add parser/lexer unit tests — 35 comprehensive tests covering all scenarios
- [x] Test edge cases: empty label `^XA^XZ`, nested `^XA`, all-empty args `^BC,,,,,`, UTF-8 in `^FD`, comments, field data with commas, span tracking, diagnostics
- [x] Tighten all test assertions to exact values (no `>=`, no `contains`)
- [x] Test field data interruption (`ZPL.PARSER.1203`) and EOF-without-FS (`ZPL.PARSER.1202`)
- [x] Test empty field data (`^FD^FS`)
- [x] Test known-set fallback (no trie)
- [x] Test validator ignores Trivia/FieldData nodes
- [x] Test distinct diagnostic codes (`1001` vs `1002`)
- [x] Test validator diagnostics include spans
- [x] Add negative/error-path tests for each diagnostic code — 29 dedicated tests covering ZPL1101, ZPL1103, ZPL1107, ZPL1108, ZPL1201, ZPL1401, ZPL1501, ZPL1502, ZPL2101, ZPL2103, ZPL2201, ZPL2202, ZPL2203, ZPL2204, ZPL2205, ZPL2301–ZPL2307, ZPL3001, plus all parser codes and false-positive guards
- [x] Add tests for untested diagnostic codes — `ZPL1104` and `ZPL2304` tested; `ZPL1105`, `ZPL1106`, `ZPL2102`, `ZPL2104` deferred (no spec commands currently use those constraint types)
- [x] Add `all_diagnostic_ids_have_explanations` test — validates all 37 codes have `explain()` entries
- [x] Update README.md — updated test count to ~90, removed completed items from "What's next"
- [x] Remove stale "subagent" comments in `parser.rs`
- [x] **Add negative test pairs** — 12 new tests covering `ZPL1101`, `ZPL1103`, `ZPL1107`, `ZPL1108`, `ZPL1201`, `ZPL2101`, `ZPL2103`, `ZPL2201`, `ZPL2202`, `ZPL2203`, `ZPL2204`, `ZPL2205`; verified no false positives for each code
- [x] **Add golden AST/diagnostic snapshot tests** — 6 snapshot tests in `crates/core/tests/snapshots.rs` with golden files in `tests/golden/`; `UPDATE_GOLDEN=1` to regenerate
- [x] **Add tokenizer/parser fuzz smoke tests** — 26 tests in `fuzz_smoke.rs`: random bytes, adversarial leaders, pathological repetition, edge-case strings, unicode, invariant checks, tokenizer coverage; found and fixed a real UTF-8 multi-byte panic bug

### Tooling (CLI, CI, Profiles)
- [x] Profiles: add `page.height_dots` to `zebra-generic-203.json` — completed (Phase 1)
- [x] Profiles: add a second profile (`profiles/zebra-generic-300.json`) — 300 dpi, 1218x1800 dot page, speed 2-6 ips (Phase 5)
- [x] Profiles: extend schema with `speed_range`, `darkness_range`, `schema_version` — added to `Profile` struct and `spec/schema/profile.schema.jsonc` (Phase 4)
- [x] Profiles: make `dpi` field resolvable — added `dpi` match arm to `resolve_profile_field` (Phase 4); DPI-dependent defaults (`^BQ`/`^B0` magnification) still TODO
- [x] Profiles: create `spec/schema/profile.schema.jsonc` — formal profile schema as single source of truth; cross-validated by spec-compiler (Phase 2)
- [x] Profiles: fix field types — `width_dots`/`height_dots` changed from `Option<i32>` to `Option<u32>` (Phase 1)
- [x] Profiles: resolve ZPL1401 redundancy — removed hardcoded height check; `profileConstraint` (ZPL1401) is now sole mechanism (Phase 1)
- [x] Profiles: `resolve_profile_field` coverage safety — added `all_profile_constraint_fields_are_resolvable` test as compile-time safety net (Phase 3)
- [x] Profiles: replace hardcoded `resolve_profile_field` match with registry — field resolution now covers all numeric profile fields; `all_profile_constraint_fields_are_resolvable` test enforces coverage
- [x] Profiles: add `profileConstraint` to the JSON Schema — added to `arg` definition in `zpl-spec.schema.jsonc` (Phase 2)
- [x] Profiles: add missing test scenarios — `profile_none_page_skips_constraints`, `profile_missing_height_skips_ll_constraint`, `profile_height_used_for_position_bounds`, `profile_300dpi_width_constraint`, `all_profile_constraint_fields_are_resolvable`, `diag_profile_constraint_sd_exceeds_darkness`, `profile_all_none_skips_all_constraints` (Phases 1/3/5 + review)
- [x] Profiles: document profile system — comprehensive `docs/PROFILE_GUIDE.md` covering schema, printerGates, profileConstraint, DPI-dependent defaults, and custom profile creation
- [x] Profiles: add `profileConstraint` to `~SD.jsonc` for darkness bounds against `darkness_range.max` (Phase 4)
- [x] Profiles: add `is_finite()` guards in `check_profile_op` and effective height/width tracking — prevents NaN/infinity from corrupting validation (review)
- [x] Profiles: add doc comments and `PartialEq`/`Eq` derives to `Profile`, `Page`, `Range` structs (review)
- [x] Profiles: add `load_profile_from_str` validation — rejects `min > max` in speed/darkness ranges (review)
- [x] Profiles: add 6 unit tests to profile crate — load/validate, minimal profile, invalid ranges, default, equality (review)
- [x] Profiles: add warning logging in `load_profile_field_paths` for missing/malformed schema (review)
- [x] Profiles: add `printerGates` diagnostic infrastructure — `ZPL1402` code, explain text, enforcement logic (Phase 5)
- [x] Profiles: implement full `printerGates` enforcement — `Features` struct with `Option<bool>` three-state semantics; command-level and enum-value-level gate resolution; ZPL1402 diagnostics
- [x] Profiles: add media capability fields — `Media` struct with `print_method`, `supported_modes`, `supported_tracking`
- [x] Profiles: add memory/firmware fields — `Memory` struct with `ram_kb`, `flash_kb`, `firmware_version`; `Features` struct with `cutter`, `peel`, `rewinder`, `applicator`, `rfid`, `rtc`, `battery`, `zbi`, `lcd`, `kiosk`
- [x] Profiles: profile-aware defaults — `defaultByDpi` support for DPI-dependent defaults
- [x] Profiles: DPI-dependent barcode magnification — `defaultByDpi` on `^BQ`/`^B0` magnification args (1/2/3/6 for 150/200/300/600)
- [x] Profiles: replace hardcoded `resolve_profile_field` match with `OnceLock` registry — declarative `PROFILE_FIELD_REGISTRY` + `OnceLock<HashMap>` for O(1) lookup; adding a field requires one line
- [x] Profiles: fix shipped profile consistency — removed peel mode `"P"` from `supported_modes` (contradicted `peel: false`); corrected `ram_kb` from 512 to 32768 (32 MB)
- [x] Profiles: add `Range::new()` / `Range::try_new()` constructors — enforces `min <= max` invariant at construction time
- [x] Profiles: fix missing `printerGates` — added gates to `~JI` (zbi), `^KV` (kiosk), `^SL`/`^KD` (rtc), `^JJ` (applicator), `^KP`/`^KL`/`^JH` (lcd)
- [x] Profiles: fix invalid `scope: "format"` — corrected `^RU` and `^RB` to `scope: "document"` (valid schema value)
- [x] Profiles: fix validator case-sensitivity mismatch — enum-value gate check now uses exact match consistent with `enum_contains`
- [x] Profiles: fix `f64::EPSILON` in `check_profile_op` — replaced with `0.5` tolerance for integer-valued profile fields
- [x] Profiles: fix `PROFILE_GUIDE.md` Features table — removed `^CN` from cutter (belongs to kiosk); added `^MM F` to rfid; synced all gate lists with actual specs
- [x] Profiles: fix `Features` struct doc comments — synced with actual spec gate annotations
- [x] Profiles: add P0 test coverage — 6 new integration tests (command-level gate positive/negative, enum-value gate fire/skip/unknown, `defaultByDpi` loaded); 4 new profile unit tests (serde round-trip, all 10 gates, malformed JSON, min==max range)
- [x] Profiles: validate `^MM` mode against `media.supported_modes` — emit warning when selected mode is not in the profile's supported list
- [x] Profiles: validate `^MN` tracking against `media.supported_tracking` — emit warning when selected tracking mode is not supported
- [x] Profiles: validate `^MT` print method against `media.print_method` — warn when thermal transfer is selected but profile only supports direct thermal
- [x] Profiles: type `Media.print_method` as `PrintMethod` enum — `DirectThermal`, `ThermalTransfer`, `Both` with `#[serde(rename_all = "snake_case")]`; replaces stringly-typed `Option<String>`
- [x] Profiles: add media validation diagnostic `ZPL1403` (MEDIA_MODE_UNSUPPORTED) — 6 tests covering `^MM`, `^MN`, `^MT` positive/negative cases
- [x] **Diagnostic structured context** — added `Option<BTreeMap<String, String>>` `context` field to `Diagnostic` struct with `with_context()` builder and `ctx!` macro; context populated at all 42 emission sites (+ 2 intentional exceptions) across validator and parser; 11 new context assertion tests; `spec/diagnostics.jsonc` updated to v1.1.0 with `contextKeys`; docs updated (`DIAGNOSTIC_CODES.md`, `PROFILE_GUIDE.md`)
- [x] **Field-scoped state tracking via `requires_field`** — added `requires_field: true` to `^FH`, `^FR`, `^SN`, `^SF` spec files; validator now enforces that field-scoped commands appear within `^FO`…`^FS` blocks (ZPL2201); 7 new integration tests
- [x] **`^FH` hex escape module** — new `hex_escape.rs` module with `decode_hex_escapes()` and `validate_hex_escapes()` functions; configurable indicator character (default `_`, set via `^FH` arg); `FieldTracker` now captures `fh_indicator` from `^FH` argument; validator refactored to use module instead of inline byte scanning; 14 unit tests + 2 integration tests (custom indicator, indicator reset between fields)
- [x] **`^GF` raw data continuation** — `validate_semantic_state()` now receives `label_nodes` and accumulates data from `Node::RawData` continuation nodes when checking `^GF` data length (ZPL2307); whitespace stripped for ASCII hex format; 3 new tests (multi-line correct, multi-line mismatch, single-line regression)

### Spec-Compiler

- [x] **Align spec-compiler with improved spec** — `generate_tables()` properly propagates all 25+ fields; `validate_cross_field()` enhanced with 8 new validations (defaultFrom linkage, constraint target existence, signatureOverrides key validity, enum/range consistency, effects.sets validation); all 216 commands pass
- [x] **Strengthen `check` command** — now runs full `load_spec_files` + `validate_cross_field` pipeline; reports structured JSON with `commands_loaded`, `validation_issues`; uses shared `SCHEMA_VERSION` constant; exits code 1 on issues
- [x] **Validate constraint target opcodes** — `extract_constraint_targets()` parses targets from constraint expressions and validates they exist in the command set
