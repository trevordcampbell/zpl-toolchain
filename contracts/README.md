# Contracts

Shared contract fixtures for cross-surface conformance tests.

## Structure

- `fixtures/bindings-parity.v1.json`
  - canonical parse/validate fixture corpus for parity tests
  - consumed by `crates/bindings-common/tests/cross_target_parity.rs`
- `fixtures/print-status-framing.v1.json`
  - canonical `~HS`/`~HI` status and STX/ETX framing fixtures
  - consumed by `crates/print-client/tests/contract_status_framing.rs`
  - consumed by `packages/ts/print/src/test/status.test.ts` and `packages/ts/print/src/test/print.test.ts`
- `fixtures/print-job-lifecycle.v1.json`
  - minimal print job lifecycle semantics (phases, IDs, deterministic completion)
  - consumed by `crates/print-client` and `packages/ts/print` for job-aware batch/completion APIs

## Intent

Keep behavior aligned across language surfaces by asserting equivalent outcomes from a single shared fixture set instead of duplicating inline test cases per package.

## CI Validation

Fixture schema/version invariants are enforced by `scripts/validate-contract-fixtures.mjs`, which runs in CI `fast-gate`. The `conformance` job runs contract fixture validation plus Rust contract consumer tests (bindings parity, print status/framing). See [docs/COMPATIBILITY_POLICY.md](../docs/COMPATIBILITY_POLICY.md) for the full governance policy.

## Adding A New Fixture

1. Add the fixture JSON under `contracts/fixtures/` and name it `name.v<version>.json`.
2. Set the top-level `"version"` field to match the filename suffix.
3. Register a schema validator in `scripts/validate-contract-fixtures.mjs`:
   - add a `validate<Name>Fixture(...)` function with required-field checks
   - add a `case "<name>.v<version>.json"` branch in `validateFixture(...)`
4. Wire at least one consumer test (Rust/TS/etc.) to load the fixture from `contracts/fixtures/`.
5. Run `node scripts/validate-contract-fixtures.mjs` locally before opening a PR.

This guard is strict by design: any fixture without a registered validator fails CI to prevent silent contract drift.
