# @zpl-toolchain/cli

`npx` wrapper for the `zpl` CLI binary.

This package downloads the matching pre-built binary from GitHub Releases on first run, caches it locally, and then executes it.

## Usage

```bash
npx @zpl-toolchain/cli --help
npx @zpl-toolchain/cli doctor --output json
npx @zpl-toolchain/cli lint label.zpl
```

## How it works

- Detects runtime platform/arch (`linux/x64`, `darwin/arm64`, `win32/x64`)
- Downloads the release binary archive for this package version
- Optionally verifies `sha256` when checksum files are available
- Extracts and caches the binary under:
  - Linux/macOS: `$XDG_CACHE_HOME/zpl-toolchain/cli` or `~/.cache/zpl-toolchain/cli`
  - Windows: `%LOCALAPPDATA%/zpl-toolchain/cli`

## Environment variables

- `ZPL_CLI_BASE_URL`: override release base URL (default GitHub Releases)
  - Only set this when using a trusted internal mirror.

## Notes

- First run requires network access to GitHub Releases.
- Supported runtime targets: `linux/x64`, `darwin/arm64`, `win32/x64`.
- If your platform is unsupported (for example Intel Mac or Linux ARM64), build from source via:

```bash
cargo install zpl_toolchain_cli
```

