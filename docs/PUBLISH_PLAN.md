# Publish All Packages — Plan

> **Status (2026-02-14):** All phases complete. v0.1.11 published to crates.io, npm, and PyPI. All future releases are automated via release-plz (including Go module tagging).

## How releases work

### Automated (day-to-day) — `.github/workflows/release-plz.yml`

1. Push conventional commits to `main` (`feat:`, `fix:`, `docs:`, etc.)
2. release-plz opens a **Release PR** with version bumps + CHANGELOG updates
3. Review and merge the PR
4. release-plz automatically:
   - Publishes 7 crates to crates.io (dependency-ordered)
   - Creates a git tag (`v0.x.0`)
   - Creates a GitHub Release with changelog notes
   - Triggers npm, PyPI, binary build, and Go module tagging jobs
5. Done — all registries updated, artifacts uploaded, Go tag pushed, no manual steps

### Manual fallback — `scripts/publish.sh`

For emergencies or manual one-off publishes. Dry-run by default — pass `--live` to publish.

```bash
./scripts/publish.sh crates           # dry-run 7 crates to crates.io
./scripts/publish.sh crates --live    # actually publish to crates.io
./scripts/publish.sh npm              # dry-run WASM package to npm
./scripts/publish.sh npm --live       # actually publish to npm
./scripts/publish.sh pypi             # dry-run Python wheel to PyPI
./scripts/publish.sh pypi --live      # actually publish to PyPI
./scripts/publish.sh all              # dry-run all registries
./scripts/publish.sh all --live       # publish everything
```

Tokens loaded from `.env` at the project root (in `.gitignore`).
Required variables: `crates_api_key`, `npmjs_api_key`, `pypi_api_key`.

### Manual fallback — `.github/workflows/release.yml`

Emergency manual workflow triggered from the GitHub Actions UI (`workflow_dispatch`).
Builds CLI binaries + FFI libs and uploads them to an existing GitHub Release.
Use when the automated release-plz pipeline fails and you need to rebuild artifacts.

## Conventional commits (required going forward)

- `feat: ...` — minor bump (0.1.0 -> 0.2.0)
- `fix: ...` — patch bump (0.1.0 -> 0.1.1)
- `feat!: ...` or `BREAKING CHANGE:` footer — major bump
- `docs:`, `chore:`, `ci:`, `refactor:` — no version bump (no release)

Enforced locally by git hooks (`.githooks/commit-msg`). Hooks also run `cargo fmt --check` on commit and `cargo clippy` + tests on push. Enable with `git config core.hooksPath .githooks` (automatic in devcontainer).

## Required GitHub Secrets

| Secret | Purpose | Used by |
|---|---|---|
| `RELEASE_PLZ_TOKEN` | GitHub PAT (contents + PRs) | release-plz.yml |
| `CARGO_REGISTRY_TOKEN` | crates.io | release-plz.yml |
| `NPM_TOKEN` | npmjs.com | release-plz.yml |
| `PYPI_TOKEN` | pypi.org | release-plz.yml |

## Crates.io publish order (handled automatically by release-plz)

1. `zpl_toolchain_diagnostics` (leaf)
2. `zpl_toolchain_spec_tables` (leaf)
3. `zpl_toolchain_profile` (leaf)
4. `zpl_toolchain_print_client` (leaf)
5. `zpl_toolchain_core` (depends on 1, 2, 3)
6. `zpl_toolchain_spec_compiler` (depends on 2)
7. `zpl_toolchain_cli` (depends on 1, 3, 4, 5)

## Phase checklist

### Phase 1: Account Setup ✅

- [x] Create crates.io account + API token
- [x] Create npm account + API token
- [x] Create PyPI account + API token
- [x] Store tokens as GitHub Actions secrets

### Phase 2: Crates.io Prep ✅

- [x] Add `publish = false` to non-publishable crates
- [x] Add metadata to all 7 publishable crates (description, repository, keywords, categories)
- [x] Add version pins to inter-crate path dependencies
- [x] Move `spec/diagnostics.jsonc` into crate for packaging
- [x] Workspace inheritance for edition, license, repository
- [x] `cargo publish --dry-run` validated

### Phase 3–5: Registry Publishing (automated by Phase 6)

- [x] npm: WASM build + publish configured in release-plz.yml
- [x] PyPI: maturin publish configured in release-plz.yml
- [ ] NuGet: deferred (no account yet)

### Phase 6: CI Automation ✅

- [x] `release-plz.toml` — single-tag mode, workspace config
- [x] `.github/workflows/release-plz.yml` — unified release workflow
- [x] `.github/workflows/release.yml` — manual fallback annotated
- [x] `scripts/publish.sh` — manual/emergency publishing script
