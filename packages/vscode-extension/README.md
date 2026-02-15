# ZPL Toolchain for VS Code

Production-ready ZPL editing support powered by the wider
[`zpl-toolchain`](https://github.com/trevordcampbell/zpl-toolchain) project.

This extension brings parser, validator, formatter, and diagnostic documentation
into VS Code-family editors while staying aligned with the same core runtime used
by the CLI and language packages.

## What this extension does

- **Syntax highlighting** for ZPL command streams (`^` / `~` command leaders).
- **Live diagnostics** from the shared WASM core validator.
- **Format support** via the same formatter used across the toolchain.
- **Rich hovers** for commands and command parameters (types, ranges, enums, units).
- **Explain diagnostic** quick action/command for fast issue triage.
- **Command + enum completions** while authoring labels.

## Part of the wider toolchain

This extension is one piece of the full `zpl-toolchain` ecosystem:

- GitHub: https://github.com/trevordcampbell/zpl-toolchain
- Core TypeScript package: `@zpl-toolchain/core`
- Print package: `@zpl-toolchain/print`
- CLI wrapper: `@zpl-toolchain/cli`
- Release/process docs: `docs/RELEASE.md`, `docs/TESTING.md`, `docs/VSCODE_EXTENSION.md`

Extension identity:

- Extension ID: `trevordcampbell.zpl-toolchain`
- Package name (`package.json#name`): `zpl-toolchain`

## Settings

- `zplToolchain.profileJson` — optional inline printer profile JSON.
- `zplToolchain.diagnostics.debounceMs` — debounce delay for diagnostics refresh.
- `zplToolchain.format.indent` — formatter style (`none`, `label`, `field`).
- `zplToolchain.format.compaction` — optional compaction (`none`, `field`, default: `field`), independent of indentation mode.
  - `field` keeps printable field blocks (`^FO/^FT/^FM/^FN ... ^FS`) on one line while preserving expanded setup/global flow when `format.indent = none`.
- `zplToolchain.format.commentPlacement` — comment style (`inline`, `line`, default: `inline`).
  - `inline` keeps standalone semicolon comments attached to the preceding command where safe.
  - `line` preserves fully line-oriented formatter output.
- `zplToolchain.hover.enabled` — enable/disable command hovers.
- `zplToolchain.themePreset` — optional token-color preset (`custom`, `default`, `high-contrast`, `minimal`).

Formatter defaults contributed for `[zpl]`:

- `editor.defaultFormatter = trevordcampbell.zpl-toolchain`
- `editor.formatOnSave = true`
- `editor.suggest.showDetails = true`
- `editor.suggest.showInlineDetails = false`
- `editor.suggest.showStatusBar = true`

IntelliSense note:

- Command completion rows are intentionally compact (`opcode + name + [CATEGORY/SCOPE]`) and rich docs are provided through suggestion details/hover.
- Suggestion details are editor-controlled UI; this extension can provide details content and defaults, but cannot force side-vs-bottom placement.

Use Command Palette:

- `ZPL Toolchain: Apply Theme Preset`

## Local development

```bash
npm ci
npm run test:ci
npm run package:vsix
```

Note: `package:vsix` uses `npx @vscode/vsce@3.7.1`, so first run on a fresh machine
needs network access to fetch that pinned VSCE version.

Run in Extension Development Host from VS Code/Cursor with `F5`.

Integration test note (`linux/arm64`):

- The test runner now auto-detects a local VS Code-family CLI executable (`code`, `code-insiders`, `cursor`, `codium`) and uses it when available.
- You can still override explicitly with `VSCODE_EXECUTABLE_PATH=/path/to/code` (the executable must support `--extensionDevelopmentPath` and `--extensionTestsPath` flags).
- If no suitable executable is available, tests are skipped by default unless `FORCE_VSCODE_INTEGRATION=1`.

## Install from VSIX (pre-publish or internal testing)

1. Build the VSIX with `npm run package:vsix`
2. In VS Code/Cursor:
   - open Command Palette
   - run **Extensions: Install from VSIX...**
   - select the generated `zpl-toolchain-<version>.vsix`

## Distribution

- Visual Studio Marketplace (`vsce`)
- Open VSX (`ovsx`)
