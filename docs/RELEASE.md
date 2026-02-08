# Release Process

## How releases work

Releases are fully automated via [release-plz](https://release-plz.ieni.dev/) and GitHub Actions.

### Day-to-day flow

1. Push conventional commits to `main` (enforced by git hooks — see below)
2. release-plz opens a **Release PR** with version bumps + CHANGELOG updates
3. Review and merge the PR
4. release-plz automatically:
   - Publishes crates to crates.io (dependency-ordered)
   - Creates a git tag (`v0.x.y`)
   - Creates a GitHub Release with changelog notes
   - Triggers npm, PyPI, and binary build jobs
5. Done — all registries updated, artifacts uploaded

### Manual fallback

For first-time publishing or emergencies, use `scripts/publish.sh`:

```bash
./scripts/publish.sh crates           # dry-run crates to crates.io
./scripts/publish.sh crates --live    # publish to crates.io
./scripts/publish.sh npm --live       # publish to npm
./scripts/publish.sh pypi --live      # publish to PyPI
./scripts/publish.sh all --live       # publish everything
```

Or push a tag manually (`git tag v0.3.0 && git push origin v0.3.0`) to trigger
the `.github/workflows/release.yml` workflow for CLI binaries and FFI artifacts.

## Version scheme

All crates share the same version number (`0.x.y`). We follow semver:

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
| 4 | `zpl_toolchain_core` | diagnostics, spec-tables, profile |
| 5 | `zpl_toolchain_spec_compiler` | spec-tables |
| 6 | `zpl_toolchain_cli` | core, diagnostics, profile |

Binding crates (`bindings-common`, `wasm`, `python`, `ffi`) have `publish = false` —
they are distributed through npm, PyPI, and binary downloads instead.

## Required GitHub secrets

| Secret | Purpose | Used by |
|--------|---------|---------|
| `RELEASE_PLZ_TOKEN` | GitHub PAT (contents + PRs) | release-plz.yml |
| `CARGO_REGISTRY_TOKEN` | crates.io | release-plz.yml |
| `NPM_TOKEN` | npmjs.com | release-plz.yml |
| `PYPI_TOKEN` | pypi.org | release-plz.yml |

## Git hooks

The `.githooks/` directory contains three hooks, activated via `git config core.hooksPath .githooks`:

| Hook | Trigger | Checks |
|------|---------|--------|
| `commit-msg` | Every commit | Conventional Commits format |
| `pre-commit` | Every commit | `cargo fmt --check` |
| `pre-push` | Every push | `cargo clippy -D warnings` + full test suite |

Skip any hook when needed: `git commit --no-verify` or `git push --no-verify`.

## CI workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push / PR | Build, test, clippy, fmt |
| `release-plz.yml` | Push to main | Release PR + automated publish |
| `release.yml` | Tag push `v*` | Manual fallback: build binaries + GitHub Release |

## Configuration files

| File | Purpose |
|------|---------|
| `release-plz.toml` | release-plz workspace config (single-tag mode, changelog) |
| `scripts/publish.sh` | Manual publishing script (dry-run by default) |
| `.githooks/*` | Git hooks (conventional commits, fmt, clippy, tests) |
| `.env` | Local API tokens for manual publishing (gitignored) |
