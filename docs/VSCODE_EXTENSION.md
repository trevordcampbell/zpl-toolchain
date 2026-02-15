# VS Code Extension

The `zpl-toolchain` repository includes a VS Code-family extension at
`packages/vscode-extension` for in-editor ZPL diagnostics and formatting.

## Current Status

- Extension package exists in-repo and is wired into CI/release workflows.
- Version follows toolchain release versioning (`0.1.13` currently).
- Marketplace/Open VSX publishing is automated in `release-plz.yml` once release
  secrets are configured.

## Supported Editors

- Visual Studio Code
- VS Code forks that support standard VSIX extensions (including Cursor)

Distribution channels:

- Visual Studio Marketplace
- Open VSX
- Manual VSIX install (release artifact)

## MVP Features

- ZPL language registration and syntax highlighting (`.zpl`)
- Live diagnostics (`validate`) from `@zpl-toolchain/core` (WASM)
- Document formatting (`format`)
- Hover documentation for command opcodes from `generated/docs_bundle.json`
  including parameter metadata at cursor (type, optionality, ranges, enum values, units)
- Contextual note routing: explanatory notes are shown in hover details instead of
  polluting the Problems panel
- Completion items for command opcodes and enum-like argument values
- Diagnostic explain command + quick action

## Settings

- `zplToolchain.profileJson` — optional printer profile JSON used for validation
- `zplToolchain.diagnostics.debounceMs` — debounce before re-validating on edits
- `zplToolchain.format.indent` — formatter indent mode (`none`, `label`, `field`)
- `zplToolchain.format.compaction` — optional formatter compaction (`none`, `field`, default: `field`)
  - `field` keeps printable field blocks (`^FO/^FT/^FM/^FN ... ^FS`) on one line while preserving setup/global flow
- `zplToolchain.format.commentPlacement` — semicolon comment placement (`inline`, `line`, default: `inline`)
  - `inline` keeps standalone semicolon comments attached to the preceding command where safe.
  - `line` preserves fully line-oriented formatter output.
- `zplToolchain.hover.enabled` — enable/disable opcode hover docs
- `zplToolchain.themePreset` — optional highlight preset (`custom`, `default`, `high-contrast`, `minimal`)
- contributed defaults for `[zpl]`:
  - `editor.defaultFormatter = trevordcampbell.zpl-toolchain-vscode`
  - `editor.formatOnSave = true`
  - `editor.suggest.showDetails = true`
  - `editor.suggest.showInlineDetails = false`
  - `editor.suggest.showStatusBar = true`

Theme preset command:

- `ZPL Toolchain: Apply Theme Preset` (Command Palette)

### IntelliSense completion UX defaults

- Completion rows are intentionally compact and high-signal: `opcode + command name + [CATEGORY/SCOPE]`.
- Rich command context lives in suggestion details and hover docs (summary, aliases, arguments, format template).
- Details-pane placement (side vs bottom) is controlled by the editor UI and is not configurable by extension APIs.

## Theme / Highlight Customization

Yes. ZPL highlighting uses TextMate scopes, so you can override colors in your
editor theme settings (`settings.json`) without changing extension code.

Example:

```json
{
  "editor.tokenColorCustomizations": {
    "textMateRules": [
      {
        "scope": "keyword.control.command.zpl",
        "settings": { "foreground": "#C586C0", "fontStyle": "bold" }
      },
      {
        "scope": "string.unquoted.field-data.zpl",
        "settings": { "foreground": "#9CDCFE" }
      },
      {
        "scope": "constant.numeric.zpl",
        "settings": { "foreground": "#B5CEA8" }
      }
    ]
  }
}
```

Useful ZPL scopes:

- `keyword.control.command.zpl`
- `string.unquoted.field-data.zpl`
- `punctuation.separator.parameter.zpl`
- `constant.numeric.zpl`
- `comment.line.semicolon.zpl`

### `zplToolchain.profileJson` format

This setting is an inline JSON string (not a path). Example:

```json
{
  "id": "zd421-203dpi",
  "schema_version": "1.0.0",
  "dpi": 203,
  "page": {
    "width_dots": 832,
    "height_dots": 1218
  }
}
```

For profile schema details, see `docs/PROFILE_GUIDE.md`.

## Local Development

```bash
cd packages/vscode-extension
npm ci
npm run test
npm run test:integration
```

To run the extension locally:

1. Open `packages/vscode-extension` in VS Code.
2. Press `F5` to launch an Extension Development Host.
3. Open a `.zpl` file and validate hover/diagnostics/format behavior.

Manual runtime checks in Extension Development Host:

- Type invalid ZPL and confirm diagnostics update while typing.
- Run `Format Document` and confirm formatter output.
- Hover command opcodes (for example `^XA`, `^FO`) and confirm docs appear.
- Hover command parameters and confirm parameter-level metadata appears.
- Trigger completions after `^` / `~` and inside enum-like args.
- Trigger diagnostic quick action (`Explain <CODE>`) and confirm details.

Automated integration coverage:

- `src/test/suite/extension.integration.test.ts` verifies:
  - diagnostics race/convergence under rapid edits
  - explain command wiring via code actions
  - hover docs resolution for command opcodes
  - formatter idempotency on `samples/*.zpl`
  - large-document diagnostics latency within configurable budget
- Test runner: `@vscode/test-electron` via `scripts/run-integration-tests.mjs`
  (Linux uses `xvfb-run` automatically for headless execution)

Note: on `linux/arm64` local setups, the runner first tries to auto-detect a
local VS Code-family CLI executable (`code`, `code-insiders`, `cursor`,
`codium`) and use it for Extension Host integration tests.
If none is available, integration tests are skipped by default unless
`FORCE_VSCODE_INTEGRATION=1` is set (or `VSCODE_EXECUTABLE_PATH` points to a
known-good local editor executable).

Preferred `linux/arm64` workaround: point tests at a known-good local executable:

```bash
VSCODE_EXECUTABLE_PATH=/path/to/code npm run test:integration
```

The configured executable must support Extension Host flags
(`--extensionDevelopmentPath`, `--extensionTestsPath`).

Performance budget override:

```bash
ZPL_VSCODE_PERF_BUDGET_MS=6000 npm run test:integration
```

## Packaging

```bash
cd packages/vscode-extension
npm run package:vsix
```

This emits `zpl-toolchain-vscode-<version>.vsix` in the package directory.

Packaging/publish scripts use an explicit VSCE version (`npx @vscode/vsce@3.7.1 ...`)
for deterministic behavior. On a fresh machine this requires network access once to
resolve/fetch that exact package version into the npm cache.

Core runtime freshness is now validated during extension builds/packaging:

- `npm run build` runs `check:core-runtime-freshness` before copying vendored runtime
- this fails fast if `packages/ts/core/wasm/pkg` is stale relative to Rust core/WASM sources
- rebuild command when stale:

```bash
wasm-pack build crates/wasm --target bundler --out-dir ../../packages/ts/core/wasm/pkg
cd packages/ts/core
npm run build
```

## Architecture Notes

- Extension host entrypoint: `packages/vscode-extension/src/extension.ts`
- Core runtime bridge: `packages/vscode-extension/src/coreApi.ts`
- Byte-span to VS Code range mapping: `packages/vscode-extension/src/utf8LineIndex.ts`
- Command docs provider: `generated/docs_bundle.json` copied into
  `packages/vscode-extension/resources/docs_bundle.json`
- Core WASM runtime assets copied into `packages/vscode-extension/vendor/core`
  at build/package time

Diagnostics pipeline is debounced and race-safe:

- per-document generation counters prevent stale async validate writes
- document-close cleanup disposes timers and cached indices
- config updates (`zplToolchain.*`) trigger re-validation of open ZPL documents

### Diagnostics audiences

The spec supports note audience targeting for `kind=note` constraints:

- `audience: "problem"` (default behavior) keeps notes in standard diagnostics
  streams (CLI lint JSON, VS Code Problems, etc.).
- `audience: "contextual"` marks explanatory notes for contextual help surfaces.
  In VS Code this currently means they appear in hover as **Additional Notes**
  and are omitted from Problems.

This is intentionally editor-agnostic naming (`contextual`, not `hover`) so
other consumers can map it to their own UX surfaces.

## Renderer-Ready Architecture (Next Phases)

The extension ships with a `RendererBridge` stub so live preview can be
added later without re-architecting diagnostics/formatting/hover flows. Preview
work will layer in a Webview panel with strict CSP and typed messaging.

### Planned preview integration

- Replace `NoopRendererBridge` with a renderer-backed implementation once the
  native renderer has stable WASM entrypoints.
- Add a preview Webview panel command (`openPreview`) with:
  - strict CSP + nonce
  - typed extension-host ↔ webview request/response protocol
  - profile/DPI controls shared with diagnostics context
  - incremental refresh scheduling compatible with diagnostics debounce strategy

### Planned designer/label-maker integration

- Introduce a split view (editor + visual panel) in a later phase.
- Keep code as source of truth; visual edits emit deterministic ZPL updates.
- Reuse renderer service for canvas preview and collision/bounds overlays.
- Connect template/builder workflows to future label builder API work.

### Planned image/PDF import capabilities

- Image -> ZPL (`^GF`) flow:
  - command-level entrypoint in extension
  - image preprocessing + thresholding options
  - insertion at cursor or selected field context
- PDF -> ZPL flow (later, built on image pipeline):
  - page selection and rasterization controls
  - optional batch conversion for multi-page labels
  - deterministic output suitable for version control and review

These are intentionally deferred from MVP, but the current service boundaries
are designed so these can be added without rewriting diagnostics/formatting
foundations.
