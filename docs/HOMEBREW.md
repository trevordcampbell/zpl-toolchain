# Homebrew Installation

Pre-built zpl-toolchain CLI binaries are available via Homebrew.

## In-repo formula

Install directly from this repository (no tap required):

```bash
brew install --formula Formula/zpl-toolchain.rb
```

## Using a tap

For `brew install trevordcampbell/zpl-toolchain/zpl-toolchain`, create a tap repo
`homebrew-zpl-toolchain` with one of these layouts:

```
homebrew-zpl-toolchain/
  zpl-toolchain.rb      # supported (repo root)
```

or

```
homebrew-zpl-toolchain/
  Formula/
    zpl-toolchain.rb   # Homebrew standard layout
```

Then add the tap and install:

```bash
brew tap trevordcampbell/zpl-toolchain https://github.com/trevordcampbell/homebrew-zpl-toolchain
brew install trevordcampbell/zpl-toolchain/zpl-toolchain
```

## Automated tap updates via CI

The release workflow (`.github/workflows/release-plz.yml`) can automatically update
the tap formula on each tagged release created by release-plz.

### Required GitHub configuration (this repo)

1. Add repository variable:
   - `HOMEBREW_TAP_REPO` = `<owner>/<homebrew-tap-repo>`
     - Example: `trevordcampbell/homebrew-zpl-toolchain`
2. (Optional) Add repository variable:
   - `HOMEBREW_TAP_FORMULA_PATH` = path to formula file inside tap repo
     - Examples:
       - `Formula/zpl-toolchain.rb` (Homebrew standard layout)
       - `zpl-toolchain.rb` (repo root)
     - Default when unset: `Formula/zpl-toolchain.rb`
3. Add repository secret:
   - `HOMEBREW_TAP_TOKEN` = fine-grained PAT with:
     - **Repository access**: tap repository only
     - **Permissions**:
       - Contents: Read and write
       - Metadata: Read

### What automation does

When a new release tag (for example `v0.1.13`) is created and artifacts are uploaded,
CI runs `scripts/update-homebrew-tap.sh` to:

1. Download release `.sha256` files for Linux x86_64 and macOS ARM64 artifacts.
2. Regenerate the formula at the configured tap path.
   - Uses `HOMEBREW_TAP_FORMULA_PATH` when configured.
3. Commit and push only when the formula content actually changed.

If the formula is already up-to-date, the job exits cleanly without creating a commit.

### Dry-run validation workflow

Use the manual workflow before a release (or when debugging automation):

1. Open **Actions → Homebrew Tap Validate**.
2. Click **Run workflow** and provide a release tag (for example `v0.1.13`).
3. The job runs `scripts/update-homebrew-tap.sh` with `HOMEBREW_TAP_DRY_RUN=true`.
4. If a change is needed, the workflow prints the staged formula diff but does not push.

## Supported platforms

| OS    | Architecture | Binary                         | Status     |
|-------|--------------|--------------------------------|------------|
| macOS | ARM64        | aarch64-apple-darwin           | Supported  |
| Linux | x86_64       | x86_64-unknown-linux-gnu      | Supported  |
| macOS | Intel        | —                              | Unsupported |
| Linux | ARM64        | —                              | Unsupported |

Unsupported platforms: use `cargo install zpl_toolchain_cli` from source.

## Updating the formula manually

If CI automation is not configured, update your formula file manually on each release:

1. Set `version` to the new release (e.g. `"0.1.13"`).
2. Update `url` and `sha256` for each platform. Fetch hashes from the GitHub release:
   ```bash
   V=v0.1.13
   curl -sL "https://github.com/trevordcampbell/zpl-toolchain/releases/download/$V/zpl-x86_64-unknown-linux-gnu.tar.gz.sha256"
   curl -sL "https://github.com/trevordcampbell/zpl-toolchain/releases/download/$V/zpl-aarch64-apple-darwin.tar.gz.sha256"
   ```
3. Extract the first field (64-char hex) as the `sha256` value.

Artifact naming and platform mapping are defined in `packages/ts/cli/lib/platform.mjs`.
