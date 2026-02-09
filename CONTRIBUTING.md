# Contributing

Thanks for your interest in contributing to zpl-toolchain! This project is dual-licensed MIT/Apache-2.0. By contributing, you agree to license your contributions under the same terms.

## Development setup

1. Install Rust (2024 edition, 1.90.0+) and `cargo-nextest`:
   ```bash
   cargo install --locked cargo-nextest
   ```

2. Enable git hooks (enforces conventional commits, formatting, and linting):
   ```bash
   git config core.hooksPath .githooks
   ```
   > This is done automatically in the devcontainer. See [Conventional Commits](https://www.conventionalcommits.org) for the commit message format.

3. Build parser tables from spec files:
   ```bash
   cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated
   ```

4. Run tests:
   ```bash
   # Rust (excludes WASM/Python crates that need special toolchains)
   cargo nextest run --workspace --exclude zpl_toolchain_wasm --exclude zpl_toolchain_python

   # TypeScript print package
   (cd packages/ts/print && npm install && npm run build && npm test)
   ```

   > See [`docs/TESTING.md`](docs/TESTING.md) for the full testing guide, including
   > print-client TCP test quirks, USB/serial feature flags, and troubleshooting.

5. Check for warnings:
   ```bash
   cargo clippy --workspace --exclude zpl_toolchain_wasm --exclude zpl_toolchain_python -- -D warnings
   ```

## Adding or updating a ZPL command spec

1. Create or edit a JSONC file in `spec/commands/` (e.g., `^XX.jsonc`)
2. Follow the pattern of existing specs — see `spec/commands/^BC.jsonc` for a well-annotated example
3. Key rules:
   - Always set `"schemaVersion": "1.1.1"` and `"version": "0.1.0"`
   - Enum values must be **strings** (e.g., `["N","R","I","B"]`, not integers)
   - Add `"default"` values from the official ZPL Programming Guide PDF
   - Add `"defaultFrom": "^FW"` on orientation params, `"defaultFrom": "^BY"` on barcode height params
   - Mark params with `"optional": true` when they have defaults
   - For state-producing commands, add `"effects": { "sets": [...] }`
   - `scope`, `category`, and `stability` are typed enums (not free-form strings). Valid values — scope: `document`, `field`, `job`, `session`, `label`; category: `text`, `barcode`, `graphics`, `media`, `format`, `device`, `host`, `config`, `network`, `rfid`, `wireless`, `storage`, `kdu`, `misc`; stability: `stable`, `experimental`, `deprecated`
4. Rebuild tables and verify: `cargo run -p zpl_toolchain_spec_compiler -- build --spec-dir spec --out-dir generated`
5. Run tests to confirm nothing broke

## Code style

- Run `cargo fmt` and `cargo clippy --workspace` before submitting
- Keep changes deterministic and offline — avoid hidden I/O in libraries
- Add tests for new behavior
- Prefer small, composable modules
- Avoid `unwrap()` in production code — use proper error handling or `.expect("reason")`
- **Workspace lints** are configured in the root `Cargo.toml` under `[workspace.lints]` and inherited by all crates via `[lints] workspace = true`. Key lints enforced:
  - `missing_docs` — all new public items need doc comments
  - `unreachable_pub` — use `pub(crate)` for items not part of the public API
  - `clippy::manual_let_else` — prefer `let ... else { ... }` over `if let` / `match` for early returns
  - `clippy::clone_on_ref_ptr` — prefer `.clone()` on the inner type rather than the `Rc`/`Arc`

## Project structure

- `spec/commands/` — per-command JSONC spec files (single source of truth)
- `crates/core/` — parser, validator, emitter, AST
- `crates/diagnostics/` — shared diagnostic types, codes (auto-generated from `crates/diagnostics/spec/diagnostics.jsonc`), severity, spans
- `crates/spec-compiler/` — build pipeline (load/validate/generate)
- `crates/spec-tables/` — shared data structures (`CommandEntry`, `Arg`, etc.)
- `crates/profile/` — printer profile loading and validation (use `load_profile_from_str()` to get semantic validation, not raw `serde_json::from_str`)
- `crates/bindings-common/` — shared logic for all language bindings (embedded tables, parse/validate/format/explain)
- `crates/print-client/` — TCP/USB/serial print client (`zpl_toolchain_print_client`)
- `crates/cli/` — `zpl` command-line tool (parse, lint, format, print, etc.)
- `crates/wasm/` — WASM bindings (thin wrapper over bindings-common)
- `crates/python/` — Python bindings (thin wrapper over bindings-common)
- `crates/ffi/` — C FFI (thin wrapper over bindings-common, foundation for Go/.NET)
- `packages/ts/core/` — TypeScript wrapper for WASM (`@zpl-toolchain/core`)
- `packages/ts/print/` — TypeScript print client (`@zpl-toolchain/print`, pure TS, `node:net` + `ws`)
- `packages/go/zpltoolchain/` — Go wrapper (cgo over C FFI)
- `packages/dotnet/ZplToolchain/` — .NET wrapper (P/Invoke over C FFI)
- `profiles/` — printer profiles (e.g., `zebra-generic-203.json`)
- `docs/BACKLOG.md` — authoritative prioritized task list
- `docs/STATE_MAP.md` — cross-command state reference
- `docs/DIAGNOSTIC_CODES.md` — diagnostic codes reference
- `docs/BARCODE_DATA_RULES.md` — barcode field data validation rules
- `docs/RELEASE.md` — release process and checklist
- `docs/TESTING.md` — comprehensive testing guide (running tests, CI, troubleshooting)
- `docs/PROFILE_GUIDE.md` — printer profile system guide
- `docs/PRINT_CLIENT.md` — print client usage and API guide
- `docs/research/ZPL-PRINT-CLIENT-PLAN.md` — print client design plan
- `CHANGELOG.md` — project changelog

## Working with language bindings

All binding crates share common logic via `crates/bindings-common/` and wrap the same `core` crate. See [ADR 0008](docs/adr/0008-ecosystem-bindings.md) for the architecture.

### Building bindings

```bash
# WASM (requires wasm-pack)
wasm-pack build crates/wasm --target bundler

# Python (requires maturin)
maturin develop -m crates/python/Cargo.toml

# C FFI
cargo build -p zpl_toolchain_ffi --release
```

### Adding a new API function

1. Implement the core function in `crates/core/`
2. Add the shared workflow to `crates/bindings-common/src/lib.rs`
3. Add the thin wrapper to each binding crate (`crates/wasm/`, `crates/python/`, `crates/ffi/`)
4. Add the wrapper to each language package (`packages/ts/core/`, `packages/go/zpltoolchain/`, `packages/dotnet/ZplToolchain/`)
5. Update the README in each crate/package