# Compatibility & Conformance Governance

> **Scope:** CLI, bindings (Rust/TS/Python/Go/.NET), and cross-surface contracts.

## 1. Compatibility and Deprecation Policy

- **CLI and bindings APIs** follow semantic versioning. Minor versions add features; patch versions are bug fixes and non-breaking changes.
- **Deprecation lifecycle:** Breaking changes require at least one minor release of deprecation warnings before removal. Document deprecated APIs in release notes and `CHANGELOG.md`.
- **Support windows:** We support the last two minor versions of Node (TS), Python 3.9–3.13, and the current Go/.NET LTS versions used in CI.

## 2. Conformance CI Gates

The following checks **must pass** on all protected branches:

| Gate | What |
|------|------|
| Contract fixture validation | `node scripts/validate-contract-fixtures.mjs` — schema/version invariants for `contracts/fixtures/*` |
| Bindings parity | `crates/bindings-common/tests/cross_target_parity.rs` — parse/validate parity vs shared fixtures |
| Print status/framing | `crates/print-client/tests/contract_status_framing.rs` — STX/ETX framing and `~HS`/`~HI` parsing vs shared fixtures |
| TypeScript contract tests | `packages/ts/print` contract fixture conformance step (`status.test`, `print.test`, `batch.test`) — TS parsing/framing/lifecycle aligned to shared fixtures |
| Spec validation | Spec-compiler `check` + `build --strict` — no mixed schema versions |
| Validator benchmark guardrail | `node scripts/check-validator-benchmark.mjs` — validate-phase performance regression gate for benchmark scenarios (ubuntu/macos/windows with OS-specific tolerances) |

These run in CI via the `fast-gate` and `conformance` matrix jobs (`rust`, `ts-print`). See [contracts/README.md](../contracts/README.md) for adding fixtures.

## 3. Schema Migration Policy

- **Single-version invariant:** Spec pipeline fails on mixed or unexpected schema versions by default.
- No compatibility escape hatch on protected branches unless an explicit override is documented and intentional.

## 4. Drift-Prevention Checklist

When adding or changing behavior that touches **multiple surfaces** (e.g., Rust + TS, or parser + bindings):

- [ ] Add or update shared fixtures under `contracts/fixtures/` (`name.v<version>.json`)
- [ ] Register fixture schema in `scripts/validate-contract-fixtures.mjs`
- [ ] Wire at least one consumer test per affected surface to load from `contracts/fixtures/`
- [ ] Run `node scripts/validate-contract-fixtures.mjs` and relevant tests locally before opening a PR

## 5. Release Note Discipline

- Surface contract-impacting changes under a dedicated **"Compatibility / Contracts"** section in release notes.
- Include fixture schema bumps, framing/status parsing changes, and cross-binding parity fixes.
