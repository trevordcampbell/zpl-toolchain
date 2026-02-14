# Release Process

## How releases work

Releases are fully automated via [release-plz](https://release-plz.ieni.dev/) and GitHub Actions.

### Day-to-day flow

1. Push conventional commits to a new feature branch (enforced by git hooks — see below)
2. Open a PR for merging these changes into `main`.
3. Review and merge the PR
4. release-plz opens a **Release PR** with version bumps + CHANGELOG updates
5. Review and merge the Release PR
6. release-plz automatically:
   - Publishes crates to crates.io (dependency-ordered)
   - Creates a git tag (`v0.x.y`)
   - Creates a GitHub Release with changelog notes
   - Triggers npm, PyPI, binary build, and Go module tagging jobs
7. Done — all registries updated, artifacts uploaded, Go tag pushed

> **Self-healing releases:** `release_always = true` in `release-plz.toml` means
> the release step runs on every push to main, not only when the commit comes from
> a release PR. If a publish fails partway through (e.g. a missing system dep),
> the next push to main automatically retries the unpublished crates. This is safe
> because release-plz is idempotent — it skips versions already on the registry.

### Install channels

| Channel | Command |
|---------|---------|
| Build from source | `cargo install zpl_toolchain_cli` |
| Pre-built binary (binstall) | `cargo binstall zpl_toolchain_cli` |
| Pre-built binary (download) | [GitHub Releases](https://github.com/trevordcampbell/zpl-toolchain/releases) |
| npx wrapper (downloads binary on first run) | `npx @zpl-toolchain/cli --help` |

`cargo binstall` metadata is configured in the CLI's `Cargo.toml` so that
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) can download
pre-built binaries directly from GitHub Releases instead of compiling from source.

### Parser tables

The compiled parser tables are committed at `crates/cli/data/parser_tables.json`
and embedded into the CLI binary at build time via `build.rs`. This means
`cargo install` (and `cargo binstall`) produce a fully working binary without
requiring users to run the spec-compiler first.

CI includes a **freshness check** that regenerates the tables from specs and
verifies the committed copy matches. If specs change without updating the
committed tables, CI will fail.

### Manual fallback

For emergencies or manual one-off publishes, use `scripts/publish.sh`:

```bash
./scripts/publish.sh crates           # dry-run crates to crates.io
./scripts/publish.sh crates --live    # publish to crates.io
./scripts/publish.sh npm --live       # publish to npm
./scripts/publish.sh pypi --live      # publish to PyPI
./scripts/publish.sh all --live       # publish everything
```

> When publishing npm packages manually, ensure the GitHub Release for the target version already exists and includes uploaded CLI binaries. The `@zpl-toolchain/cli` package downloads those release assets at runtime.

For manual publishing, create a repo-root `.env` with:

```bash
crates_api_key=...
npmjs_api_key=...
pypi_api_key=...
```

To rebuild CLI/FFI binaries and upload them to an existing GitHub Release (e.g. if
the automated upload failed), trigger the manual workflow from the GitHub Actions UI:
**Actions → Release (manual) → Run workflow → enter the tag (e.g. `v0.3.0`)**.

> **Why not a tag trigger?** The automated `release-plz.yml` workflow uses a PAT
> (needed so CI runs on release PRs), which means tags it creates bypass GitHub's
> anti-recursion protection and trigger other workflows. A tag-triggered manual
> workflow would run in parallel with the automated one, causing double builds,
> race conditions, and API rate limiting. Using `workflow_dispatch` keeps the
> manual workflow as an explicit, intentional action only.

## Pre-release verification

Before creating a release (or when debugging CI failures), run the full test suite locally.
See [`docs/TESTING.md`](TESTING.md) for the complete guide, including platform-specific
quirks and the print-client TCP tests.

Quick smoke test:

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo nextest run --workspace --exclude zpl_toolchain_wasm --exclude zpl_toolchain_python
(cd packages/ts/print && npm ci && npm run build && npm test)
(cd packages/ts/cli && npm test)
bash scripts/test-python-wheel-local.sh
bash scripts/test-dotnet-local.sh
```

## Version scheme

Crates are versioned independently by release-plz based on which files changed. We follow semver:

- **Patch** (`0.1.x`): bug fixes, doc improvements, spec corrections, new command specs
- **Minor** (`0.x.0`): new features (new CLI commands, new API functions, new binding targets)
- **Major** (`x.0.0`): breaking API changes (deferred until post-1.0)

Pre-1.0, minor versions may contain breaking changes with notice in the changelog.

Version bumps are handled automatically by release-plz based on commit types:
- `feat:` → minor bump
- `fix:` → patch bump
- `feat!:` or `BREAKING CHANGE:` footer → major bump
- `docs:`, `chore:`, `ci:`, `refactor:` → no release

## Conventional commits

All commits must follow the [Conventional Commits](https://www.conventionalcommits.org) format:

```
type(scope): description
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

This is enforced locally by the `commit-msg` git hook (`.githooks/commit-msg`).
Hooks are activated automatically in the devcontainer, or manually:

```bash
git config core.hooksPath .githooks
```

## Crate publishing order

release-plz handles ordering automatically. For manual publishing, follow this order:

| # | Crate | Dependencies |
|---|-------|-------------|
| 1 | `zpl_toolchain_diagnostics` | — |
| 2 | `zpl_toolchain_spec_tables` | — |
| 3 | `zpl_toolchain_profile` | — |
| 4 | `zpl_toolchain_print_client` | — |
| 5 | `zpl_toolchain_core` | diagnostics, spec-tables, profile |
| 6 | `zpl_toolchain_spec_compiler` | spec-tables |
| 7 | `zpl_toolchain_cli` | core, diagnostics, profile, print-client |

Binding crates (`bindings-common`, `wasm`, `python`, `ffi`) have `publish = false` —
they are distributed through npm, PyPI, and binary downloads instead.

> **Version syncing**: release-plz only bumps `Cargo.toml` versions. After creating
> the Release PR, a workflow step automatically syncs `package.json` (npm) and
> `pyproject.toml` (PyPI) versions to match, so all ecosystems stay in lockstep
> and the repo always reflects the true version at every commit.

## Required GitHub secrets

| Secret | Purpose | Used by |
|--------|---------|---------|
| `RELEASE_PLZ_TOKEN` | GitHub PAT (contents + PRs) | release-plz.yml |
| `CARGO_REGISTRY_TOKEN` | crates.io | release-plz.yml |
| `NPM_TOKEN` | npmjs.com (`@zpl-toolchain/core`, `@zpl-toolchain/print`, `@zpl-toolchain/cli`) | release-plz.yml |
| `PYPI_TOKEN` | pypi.org | release-plz.yml |

## Git hooks

The `.githooks/` directory contains three hooks, activated via `git config core.hooksPath .githooks`:

| Hook | Trigger | Checks |
|------|---------|--------|
| `commit-msg` | Every commit | Conventional Commits format |
| `pre-commit` | Every commit | Parser tables sync + `cargo fmt --check` |
| `pre-push` | Every push | `cargo clippy -D warnings` + full test suite |

Skip any hook when needed: `git commit --no-verify` or `git push --no-verify`.

## CI workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push / PR | Build, test, clippy, fmt |
| `release-plz.yml` | Push to main | Release PR + automated publish (crates.io, npm, PyPI, binaries, Go tag) |
| `release.yml` | `workflow_dispatch` (manual) | Emergency fallback: rebuild binaries + upload to GitHub Release |

## Configuration files

| File | Purpose |
|------|---------|
| `release-plz.toml` | release-plz workspace config (single-tag mode, changelog) |
| `scripts/publish.sh` | Manual publishing script (dry-run by default) |
| `.githooks/*` | Git hooks (conventional commits, fmt, clippy, tests) |
| `.env` | Local API tokens for manual publishing (gitignored) |
