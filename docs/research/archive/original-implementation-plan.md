# `@zpl-toolchain` — Spec‑First Architecture & Implementation Plan (v1.3)

> This **complete** v1.3 document supersedes v1.2. It folds in robustness features to cover *all* ZPL II commands while preserving the original structure, goals, and tone. Major additions are marked where relevant but woven into the canonical text.

---

## 0. Executive Summary

Today’s ZPL ecosystem is fragmented: projects often skip a spec‑accurate validator, rely on cloud renderers, or tie themselves to a single runtime. We will build a **single, offline, spec‑first, composable toolchain** with a **Rust** core and **WASM** targets, with bindings for TS / JS, Python, Go, and .NET. The stack centers on two canonical data models — **AST (Abstract Syntax Tree)** and **ULIR (Unified Label Intermediate Representation)** — driven by **machine‑readable ZPL II spec tables** and **printer profiles**. Deliverables include parser, validator / linter, generator, renderer, editor / designer, print client, emulator, CLI, SDKs, and CI‑ready testing.

**PNG is the truth artifact** for behavior. Goldens, visual diffs, and barcode verification keep us honest.

**(optional):** an **SGD management plane**—typed, validated get/set/do of device variables and actions; profile‑declared expected defaults; a **Printer Twin** snapshot + drift check. This **never replaces** ZPL behavior; it complements it for operations.

**v1.3 additions**

* **Planes & scopes**: every command specifies its **plane** (**format | host | device**) and **scope** (**field | document | job | session**).
* **Lexer/parser mutations:** spec‑driven changes to **command prefix** and **language mode** (e.g., `^CC/~CC`, `^SZ`).
* **First‑class resource references:** `resourceRef` & `composite` arg types for device file paths and fixed extensions (`^XG`, `^A@`, `^DF/^XF`, `~DG`).
* **Profile‑gated enums & combo matrices:** model/firmware‑aware values and cross‑command validity (e.g., `^MM × ^MN × cutter`).
* **Query/response contracts:** typed schemas & parsers for tilde queries (`~HM`, `~HS`, `~HQES`).
* **State models:** deterministic serialization/counters (`^SN`) with ULIR snapshotting.
* **Render contract:** explicit rounding/quantization/pixel rules to guarantee Native/WASM parity.

---

## 0.1 Design Principles

1. **Spec‑compiled, not hand‑coded** — a spec compiler generates parser tables, validators, docs, completions, and quick‑fixes from machine‑readable ZPL tables (**plus** SGD registries). **v1.3:** also emits tokenizer config, resource helpers, combo matrices, query schemas, render contracts.
2. **Micro‑kernel core** — tiny tokenize → **AST** → **ULIR** engine; barcodes, fonts, transports, rulelets are plug‑ins; **SGD is a separate optional plug‑in**.
3. **ULIR (Unified Label Intermediate Representation)** — backend‑agnostic IR; ZPL first, future backends via capability matrix (EPL / TSPL / ESC‑POS / CPCL / PDF).
4. **Determinism over cleverness** — stable formatting / generation; explicit rounding/quantization; heuristics gated by flags.
5. **Fail early, fix fast** — strict checks with actionable auto‑fixes and doc hops.
6. **Portable by default** — defaults target `203‑dpi` portability; profiles can opt into richer behavior.
7. **Zero hidden network** — offline by default; any remote use is opt‑in and replaceable.
8. **Runtime‑agnostic UX** — same mental model across CLI, editor, CI, SDKs.
9. **Small sharp tools** — each sub‑tool is useful solo (parse, `zpl-format`, render, print, emulate) and composes cleanly.
10. **Test the truth** — **PNG** is the truth artifact; goldens + visual diffs + barcode verification. **SGD** uses record/replay fixtures and typed schemas.

**Licensing:** permissive (MIT / Apache‑2.0) across repos.

---

## 1. Goals, Non‑Goals, Versioning

### Goals

* **Spec‑first + offline**, **deterministic** outputs, **high performance** (Rust + WASM), **portable bindings**, **secure by default**, excellent **DX / UX**, and **permissive licensing** (MIT / Apache‑2.0).
* **ZPL** parsing, validation, rendering, generator, editor, print service, emulator, and SDKs with **complete command coverage** (prefix/language mutations, device/media policy, host queries, counters, resource references).
* **Optional SGD**: typed get/set/do, profile defaults, Printer Twin drift detection, CLI/LSP flows, emulator KV‑store — **without changing any ZPL behavior** when unused.

### Non‑Goals (MVP)

* Full WYSIWYG HTML → ZPL in core (later via adapters atop `^GF` encoders).
* Full USB transport coverage day one (start with network + BrowserPrint bridge).
* Full scalable font engine in default WASM (`^A@` optional initially).
* **For SGD:** browser raw sockets; requiring SGD to print; any background/device‑mutating calls by default.

### Versioning & Compatibility

* **Semver** across all packages / crates.
* **ULIR** is versioned; breaking changes bump major with migration notes.
* **Spec tables** are versioned independently; core declares a compatible spec range.
* VS Code extension pins a core/spec range and warns on drift.
* **SGD registries** (variables/actions) are versioned in the spec package with firmware ranges; diffs are surfaced by the spec compiler.

---

## 2. Architecture Overview

### 2.1 **Simple (ZPL data plane)**

```mermaid
flowchart LR
  subgraph pipeline["Core pipeline"]
    direction LR
    ZPL["ZPL text"] --> P["Parser (AST)"]
    P --> V["Validator / Linter"]
    V --> E["Executor (ULIR)"]
  end

  subgraph outputs["Outputs"]
    direction TB
    R["Renderer (PNG/SVG)"]
    PR["Print / Emulator"]
  end

  subgraph generator["Generator / Templates"]
    direction TB
    G["Generator / Templates"]
  end

  G -->|AST or ULIR| E
  E --> R
  E --> PR
```

**v1.3 note:** The tokenizer consumes **spec‑emitted lexer mutations** (prefix/language) ensuring identical token streams across Native/WASM. The emitter derives the published **Format:** string from `signature` (with **no space** after the opcode) and supports caret/tilde twins via `codes` + optional `signatureOverrides`.

### 2.2 **Comprehensive (ZPL surfaces)**

```mermaid
flowchart TD
  classDef group fill:#f7f7f8,stroke:#c8ccd0,stroke-width:1px,color:#111;
  classDef core  fill:#eef6ff,stroke:#7baaf7,stroke-width:1px,color:#0b3d91;
  classDef out   fill:#eefcf3,stroke:#80c68b,stroke-width:1px,color:#1e5e2e;
  classDef plug  fill:#fff7e6,stroke:#f2b366,stroke-width:1px,color:#8a4b00;

  subgraph SURF["Surfaces"]
    direction LR
    CLI["CLI / CI"]:::group
    VS["VS Code (LSP)"]:::group
    SDK["SDKs TS/Py/Go/.NET"]:::group
  end

  subgraph SPECZ["Specs & Profiles"]
    direction TB
    SPEC["ZPL II Command Tables"]:::group
    COMP["Spec Compiler"]:::group
    PTBL["Parser Tables"]:::group
    RULES["Validation Rules"]:::group
    DOCS["LSP Docs / Completions"]:::group
    PRO["Printer Profiles"]:::group
    SPEC --> COMP
    COMP --> PTBL
    COMP --> RULES
    COMP --> DOCS
  end

  subgraph CORE["Core"]
    direction TB
    FMT["zpl-format (formatter)"]:::core
    PAR["Tokenizer + Parser → AST"]:::core
    VAL["Validator / Linter"]:::core
    EXE["Executor → ULIR"]:::core
    FMT --> PAR --> VAL --> EXE
  end

  subgraph OUTS["Outputs"]
    direction LR
    RND["Renderer (PNG/SVG)"]:::out
    PRN["Print Service"]:::out
    EMU["Emulator"]:::out
  end

  subgraph INPUTS["Inputs / Generation"]
    direction LR
    ZPLNODE["ZPL text"]:::group
    TPL["Templates + Data"]:::group
    GEN["Generator"]:::group
    TPL --> GEN
  end

  subgraph PLUGS["Plugin Surfaces"]
    direction LR
    BAR["Barcode Engines"]:::plug
    FON["Font Engine ^A@"]:::plug
    TRN["Transports 9100/LPR/BrowserPrint/…"]:::plug
  end

  ZPLNODE --> FMT
  GEN -->|AST| PAR
  GEN -->|ULIR| EXE
  EXE --> RND
  EXE --> PRN
  EXE --> EMU

  CLI --> FMT
  CLI --> VAL
  CLI --> EXE
  CLI --> RND
  CLI --> PRN

  VS --> PAR
  VS --> VAL
  VS --> RND

  SDK --> EXE
  SDK --> RND
  SDK --> PRN

  PTBL -.-> PAR
  RULES -.-> VAL
  DOCS -.-> VS
  PRO -.-> VAL
  PRO -.-> EXE
  PRO -.-> RND
  PRO -.-> PRN

  BAR -.-> RND
  FON -.-> RND
  TRN -.-> PRN
```

**v1.3 additions:**

* Compiler also emits **TokenizerConfig**, **resourceRef helpers**, **combo matrices**, **query schemas**, **Signature builder**, and **RenderContract**.
* Validator enforces **profile‑gated enums** and **combo matrices**.
* Executor implements **state models** (e.g., `^SN`) and embeds effective seeds into ULIR.

### 2.3 **Additive (optional) SGD management plane**

```mermaid
flowchart TD
  classDef mgmt  fill:#fff0f3,stroke:#f28ba8,stroke-width:1px,color:#7a1432;

  SGD["SGD Client (get/set/do)"]:::mgmt
  PRO["Printer Profiles (+ SGD expectations)"]
  PRN["Print Service"]
  EMU["Emulator"]
  CLI["CLI / LSP / SDKs"]

  SGD -. optional .-> PRN
  SGD -. optional .-> EMU
  CLI --> SGD
  PRO -. informs .-> SGD
```

> The SGD graph is separate and optional; it must not interfere with the ZPL pipeline.

---

## 3. Packages (Monorepo)

> **Repository:** `zpl-toolchain` — Rust core, WASM builds, CLI, editor, SDK bindings, transports, and tooling.

### 3.1 Naming & Packaging (by ecosystem)

**Rust (crates.io)**

* `zpl_toolchain_core` (AST / validator / ULIR / renderer)
* `zpl_toolchain_cli`
* `zpl_toolchain_spec`
* `zpl_toolchain_spec_compiler`
* **`zpl_toolchain_sgd` (optional)** — typed SGD client + wire parser + policy

**Node / TypeScript (npm)** — WASM‑backed, thin TS APIs

* `@zpl-toolchain/core`
* `@zpl-toolchain/renderer`
* `@zpl-toolchain/generator`
* `@zpl-toolchain/print` (+ transports)
* `@zpl-toolchain/cli`
* `@zpl-toolchain/editor`
* `@zpl-toolchain/spec`, `@zpl-toolchain/spec-compiler`
* **`@zpl-toolchain/sgd` (optional)** — wrapper for `sgd` APIs

**Python (PyPI)** — `zpl_toolchain` (maturin / pyo3 wheel; **sgd** submodule optional)

**Go (modules)** — `github.com/<org>/zpl-toolchain/go/zpltoolchain` (cgo wrapper; **sgd** pkg optional)

**.NET (NuGet)** — `ZplToolchain` (+ `ZplToolchain.Sgd` optional; RID‑specific native assets via P/Invoke)

> The **core is Rust**, compiled to native libraries and to **WASM**. npm packages wrap the WASM build; Python / Go / .NET bind to the native library. Same engine, many faces.

### 3.2 Targets & Language Bindings (Implementation Intent)

**Single engine, multiple delivery targets**

| Layer / User       | Artifact               | Default Target | Notes                                                                                         |
| ------------------ | ---------------------- | -------------- | --------------------------------------------------------------------------------------------- |
| Rust apps & CLI    | Native library         | **Native**     | Full threads / SIMD via Rayon; best throughput.                                               |
| Python / Go / .NET | Native library via FFI | **Native**     | Wheels / NuGet / modules ship prebuilt binaries; no toolchain required.                       |
| VS Code extension  | WASM module            | **WASM**       | Runs inside extension host safely; no native install.                                         |
| Browser apps       | WASM module            | **WASM**       | Offline‑capable; multiple Web Workers for parallelism.                                        |
| Node services / CI | WASM module (default)  | **WASM**       | Zero native deps; can scale via Worker Threads. Optional native addon later for maximum perf. |

**Determinism guarantee** — Goldens render identically regardless of target or concurrency level; CI compares PNGs across Native / WASM to guard regressions.

> **SGD APIs** follow the same delivery strategy as optional modules; enabling them does **not** change ZPL runtime behavior.

---

## 4. Components

### 4.0 `@zpl-toolchain/spec-compiler`

Build‑time generator that ingests `@zpl-toolchain/spec` tables and emits:

* tokenizer / grammar constants, arg parsers, validator rules / messages,
* LSP artifacts (hover docs, signature help, completions, code actions),
* Reference docs.

**v1.3 additional outputs:**

* **TokenizerConfig** (prefix/language mutations per spec).
* **resourceRef** & **composite** arg helpers (printer‑device paths, fixed extensions, name rules).
* **comboMatrix** + **profile‑gated enums** for validator tables.
* **Query/Response** schemas & sample parsers for tilde commands (`~HM`, `~HS`, `~HQES`).
* **Signature builder** to render exact **Format:** strings from `signature`/overrides (no space after opcode).
* **RenderContract** constants consumed by all renderer targets.

#### 4.0a Spec‑compiler codegen enhancements

* **Typed bindings**: discriminated unions for Rust/TS; generated JSON Schemas for configs and CLI I/O.
* **Executable examples**: spec entries embed runnable examples; compiler emits unit tests and VS Code hover snippets tied to those examples.
* **Failure gates**: docs fail CI if examples are broken; tables must hit coverage thresholds.

### 4.1 `@zpl-toolchain/grammar`

Rust tokenizer + parser (Logos / combinators). Comments/whitespace/raw regions. Emits **typed AST** with spans. **SGD is not parsed here.** **v1.3:** Tokenizer obeys spec‑emitted **prefix/language mutations** and serializes the effective mode into AST metadata for LSP banners.

### 4.2 `@zpl-toolchain/spec`

Versioned **ZPL** command & constraint tables (+ docs) as JSON / TOML. CI asserts schema and implementation **coverage**.

**v1.3 schema facets:** `plane`, `scope`, `lexerMutation`, `args[type=resourceRef|composite]`, `enumWhen`, `comboMatrix`, `response`, `stateModel`, `renderContract`, `defaultsMap`, `safety`, `gates`.

**(optional) registries:** `sgd.variables[]`, `sgd.actions[]` with firmware gates.

### 4.3 `@zpl-toolchain/core` (validator + ULIR executor)

* Validation: presence/order, arity, types, ranges, unit conversions.
* Cross‑command / state coupling (e.g., `^FW` vs field rotation; `^FB` effects; `^BY` defaults for `^BC` / `^BD` / `^BE`; `^GF` storage rules).
* **ULIR** executor: units / DPI scaling, `^FW`, `^FB`, `^FO` / `^FT` precedence, darkness / speed hints.
* Lint tiers: **error / warn / advisory**. Custom rules via config + sandboxed **WASM rulelets**.

**v1.3 additions:**

* **State models** for serialization/counters (`^SN`) with deterministic ULIR embedding.
* **Combo‑matrix** enforcement and **profile‑gated enum** resolution.
* New diagnostics: `prefix-changed-next-char` (info), `language-mode-changed` (info), `combo-invalid` (error), `rounding-applied` (info), `serialization-seed-missing` (warn).

**Real‑world notes:**

* **Fonts & code pages:** Ship device‑font metric packs per printer family (A..H/0 vary). Handle `^CI` code pages and provide a sanitizer for characters not representable under the active `^CI`.
* **`^A@` downloadable fonts:** Optional in WASM; prioritize native support early for labs needing Greek/non‑Latin or long IDs.
* **Barcode layout knobs:** Configurable quiet‑zone enforcement and module‑width rounding strategies per symbology and DPI.

#### 4.3a ULIR normalization & renderer contracts

* **Normalization passes:** deterministic unit normalization, layout resolution, barcode parameter canonicalization before rendering.
* **Structural hashing:** BLAKE3 of ULIR and rendered PNGs to dedupe caches/artifacts.
* **Pixel‑law contract:** documented rounding rules, module quantization, and anti‑alias policy identical in native & WASM.

### 4.4 (optional) `@zpl-toolchain/sgd`

Typed client to **get/set/do**; transport‑agnostic; policy & redaction; record/replay fixtures.

#### 4.4a SGD Transaction Model

* **Plan‑first:** `snapshot → diff(profile) → plan`; supports `--dry-run` and JSON output for CI.
* **Staged writes:** dependency‑ordered sets; verify with post‑`get`; rollback where possible.
* **Idempotency keys:** dedupe retries on flaky links.
* **Session scope:** optional auto‑restore of volatile changes after a print job.

### 4.5 `@zpl-toolchain/generator`

* Programmatic label builder: produce **AST** or **ULIR** → ZPL.
* **Schema‑checked templating** (JSON Schema / Zod); conditionals/loops; deterministic transforms (date, checksum, case).
* Layout helpers: grid / guides, baseline alignment, fit‑to‑PW / LL, multi‑DPI strategies, optical safe areas.
* Style presets: tubes / cryovials / plates / equipment.
* **Label ABI**: versioned template metadata (inputs, constraints, profile assumptions).

### 4.6 `@zpl-toolchain/editor`

* **VS Code extension** (LSP): semantic tokens, hovers, diagnostics + quick‑fixes, refactors, **live preview** (WASM), printer profile switcher, grids / safe areas.
* **Designer canvas**: IR‑layer editor; round‑trip to ZPL via generator; split view (code ↔ canvas).
* Snippets/code actions: `^XA` / `^XZ` scaffolds, `^FB` blocks, barcode wizards.
* Print commands, job history, artifact diffs (image & ZPL).

**Resource & media management:**

* **On‑device resources:** Resource Manager to sync/list/delete/checksum graphics/fonts (ZPL “file system”). Preflight fails if a referenced `R:*.GRF`/`E:*` is missing. Support `^DG`/`^DB`/`^DY` lifecycle and `^XG` placement.
* **Media policy checks:** Validate `^MN/^MT/^MM`, peel/cut/tear‑off combinations, `^PR` speed and `^MD` darkness against profile. Ensure `^LL` bounds, non‑zero top origin where required.
* **Batching & multi‑label:** Distinguish job vs page (e.g., `^PQ`). Provide `spooler.split()` for throughput. Implement deterministic chunking for large `^GF` blocks to avoid device buffer issues.

**Preflight & Twin:**

* **`preflight` subcommand:** `zpl preflight <label>` computes effective `^PW/^LL`, object bounds, text overflow, barcode quiet zones, total `^GF` bytes, media mode sanity, and returns pass/fail.
* **Printer Twin ops:** `zpl twin pull|push` to version device state (profiles; optional SGD snapshot).

### 4.9 SDKs — `@zpl-toolchain/sdk-{ts,py,go,dotnet}`

Thin bindings over the core; uniform APIs; **optional** `sgd.*` namespaces.

---

### Job vs page model

ULIR introduces a **Job** container including one or more **Pages**. This reflects ZPL constructs like `^PQ` within a single `^XA…^XZ` block and enables batching/splitting logic in the spooler.

### Deployment notes

* **Device storage:** Profiles capture storage letters (`R:`,`E:`,`B:`), capacities, and per‑model limits for graphics/font resources.
* **Provenance:** Embed firmware ranges and font‑metric origin; publish a profile fingerprint.
* **Self‑check:** Optional live validation using `~HQES/~HS` to compare a real printer’s snapshot with the profile and warn on drift.

### 4.10 Profiles — Capability Matrix & Verification Kit

* **Capability matrix:** declare ZPL command coverage, barcode constraints, and SGD vars/actions per profile (with firmware gates).
* **Downgrade hints:** validators propose safe alternatives when commands are unsupported.
* **Probe kit:** `probe run` sends `~HS/^HH` and tiny labels to auto‑fill capabilities; results can be signed and merged into profiles.

---

## 7. Configuration

### 7.1 Resolution & Precedence

Order of precedence (highest wins): CLI flags → env vars → nearest project `.zplrc.jsonc` → user config → embedded template metadata → built‑ins. `zpl config --show` prints effective config and source per key.

**Common env vars (preserved):** `ZPL_PROFILE`, `ZPL_DPI`, `ZPL_RENDER_DITHER`, `ZPL_LINT_STRICT=1`, `ZPL_CACHE_DIR`, `ZPL_RULELETS_DIR`, `ZPL_OFFLINE=1`.

**v1.3 additions:**

* `ZPL_TOLERANT_UNKNOWNS=1` (treat unknown codes as info with suggestions).
* `ZPL_ALLOW_DANGEROUS=1` (gate execution of commands marked `safety.class>=medium`).

**Formatting rule:** ZPL has **no space** between the opcode and its parameter list (e.g., `^JJa,b,c`, not `^JJ a,b,c`). The spec‑compiler enforces this via `signature.noSpaceAfterOpcode = true`.

### 7.2 File Conventions

* Ignore: `.zplignore` (gitignore syntax) for `zpl render --golden` and `zpl lint` discovery.
* Templates: `.zplt` (ZPL + placeholders) + optional adjacent `.schema.json`.
* Profiles: JSON files under `profiles/`, referenced by id in `.zplrc.jsonc`.

### 7.3 `.zplrc.jsonc` (full example)

```jsonc
{
  "$schema": "https://zpl.dev/schema/zplrc.json",
  "printerProfile": "zebra-zd620-203dpi",
  "units": "mm",
  "dpiStrategy": "portable-203",
  "paths": {
    "profiles": "./profiles",
    "rulelets": "./zpl-rulelets",
    "goldens": "./test/goldens",
    "cache": ".zpl-cache"
  },
  "template": {
    "delimiters": ["${", "}"],
    "schema": "./schemas/tube-label.schema.json",
    "strict": true,
    "coerce": true,
    "transforms": { "date": "yyyy-MM-dd", "uppercase": ["sampleId"] }
  },
  "lint": {
    "mode": "strict",
    "error": ["missing-terminator-xz","arg-out-of-range","unknown-command","barcode-quiet-zone-too-small"],
    "warn": ["no-explicit-pw-ll","text-overflow-in-fb","image-not-multiple-of-8"],
    "advice": ["merge-repeated-fields","prefer-mm-units"],
    "rules": { "max-darkness": 25, "min-font-height-mm": 1.7, "require-origin-command": true, "portable-dpi": true },
    "rulelets": [{ "id": "org.policy.example", "path": "zpl-rulelets/policy.wasm", "allow": ["readSpec"] }]
  },
  "render": {
    "output": "png",
    "dpi": 203,
    "antialias": true,
    "dither": "floyd-steinberg",
    "background": "transparent",
    "trim": true,
    "barcode": {
      "verify": true,
      "defaultStandard": "none",
      "standards": {
        "GS1": { "enabled": true, "aiAllowlist": ["01","10","17"], "enforceFNC1": true },
        "HIBC": { "enabled": true, "linkCharacter": "auto" }
      }
    }
  },
  "print": {
    "transport": "tcp9100",
    "target": { "host": "192.168.1.55", "port": 9100 },
    "retries": 3,
    "backoffMs": 1000,
    "preflight": { "policy": "fail", "autoScaleMaxPct": 5 },
    "statusTimeoutMs": 1500,
    "queue": { "enabled": true, "maxPending": 100 }
  },
  "emulator": {
    "enabled": false,
    "port": 9100,
    "profile": "zebra-zd621-203dpi",
    "faults": { "paperOut": false, "headOpen": false },
    "memoryLimitKb": 4096,
    "recordJobs": true,
    "artifactDir": ".zpl-emulator"
  },
  "security": { "offline": true, "redactLogs": true, "logLevel": "info" },
  "policy": { "tolerantUnknowns": false, "allowDangerous": false }
}
```

### 7.3a `.zplrc.jsonc` — **optional** SGD section

```jsonc
{
  "sgd": {
    "allowWrites": false,
    "allowlist": ["device.languages","ip.*","odometer.*"],
    "denylist": ["*password*","wlan.passphrase","tls.*"],
    "timeoutMs": 1500,
    "retries": 2,
    "backoffMs": 400,
    "dangerousRequireFlag": true,
    "snapshot": { "include": ["device.languages","media.*","print.*"] }
  }
}
```

### 7.4 CLI Flag Mapping (common)

`--profile`, `--dpi`, `--units`, `--dither`, `--verify-barcodes`, `--preflight`, `--transport`, `--host`, `--port`, `--retries`, `--strict`, `--fix`, `--check`, `--golden-dir`, `--rulelet <path>`.

**SGD CLI (optional):** `zpl sgd get|set|do|snapshot|diff|apply` with `--yes`, `--allow-risky`, `--dry-run`.

### 7.5 Rulelets (Sandboxed Lint Extensions)

* **Format**: WASM modules receiving `{ast, profile, spec, config}` and returning diagnostics/quick‑fixes.
* **Sandbox**: no network / FS; deterministic clock; CPU / mem / time caps; allowed host fns are whitelisted (e.g., `readSpec()`).
* **Packaging**: `.wasm` + `rule.json` manifest (id, version, description, permissions).
* **Distribution**: checked into repo or installed via private registry; enterprises can enforce an allowlist.

#### 7.5a Rulelets v2 ABI

* **Versioned ABI** with declarative capabilities (`readSpec`, `readProfile`), deterministic quotas, and a tiny SDK.
* **Policy:** enterprise allowlist maps capabilities; CI runs rulelets under quota to prevent nondeterminism.

### 7.6a Security & template determinism

* **Logging:** Redact label payloads by default; opt‑in verbose modes never dump full ZPL.
* **Template runtime:** Deterministic clock by default, no network transforms, and explicit **input size limits**.
* **Media policy:** Configurable checks for `^MN/^MT/^MM`, `^PR`, `^MD`, and `^LL` bounds with `fail|warn|off` modes.

### 7.7 Golden Tests & Verify

* `zpl render --golden` compares PNG fixtures in `paths.goldens`.
* `zpl verify --barcode` decodes barcodes from rendered PNGs; enforces quiet zones and (if enabled) selected standard payload rules.
* **v1.3:** Adds **lexer mutation**, **combo matrix**, **query parse**, **serialization determinism**, and **render‑contract** parity test suites.

### 7.8 Template ABI Metadata (example)

```jsonc
{
  "abiVersion": 1,
  "id": "tube.v1",
  "profile": "zebra-zd620-203dpi",
  "inputs": {
    "sampleId": { "type": "string", "required": true },
    "lot": { "type": "string" },
    "expires": { "type": "string", "format": "date" }
  },
  "constraints": [
    { "kind": "barcodeStandard", "symbology": "DataMatrix", "standard": "GS1", "required": false }
  ],
  "preview": { "width_mm": 25, "height_mm": 19 }
}
```

---

## 8. Public API Sketches

*(unchanged; representative snippets preserved)*

```ts
import { parse, validate } from "@zpl-toolchain/core";
import { renderPNG } from "@zpl-toolchain/renderer";
import { Printer, Emulator } from "@zpl-toolchain/print";
```

```python
import zpl_toolchain as zpl
```

```go
import zpl "github.com/<org>/zpl-toolchain/go/zpltoolchain"
```

```csharp
using ZplToolchain;
```

```ts
const ast = parse(zplText);
const result = validate(ast, { profile: "zebra-zd420-300dpi" });
if (!result.ok) console.error(result.issues);
```

```ts
import { compileTemplate } from "@zpl-toolchain/generator";
const tpl = compileTemplate(readFile("tube-label.zplt"), { schema: "tube.schema.json" });
const { zpl, ir } = tpl.render({ sampleId: "ABC123", lot: "L-42" });
```

```ts
const png = await renderPNG(ir, { dpi: 203 });
await fs.writeFile("preview.png", png);
```

```ts
const emu = new Emulator({ port: 9100, profile: "zebra-zd621-203dpi" });
emu.start();
const p = await Printer.connect({ host: "127.0.0.1", port: 9100 });
await p.print(zpl);
const status = await p.status();
```

*(Optional) SGD*

```ts
import { sgd } from "@zpl-toolchain/sgd";
const snap = await sgd.snapshot({ include: ["device.languages","media.*"] });
const diff = sgd.diff(snap, profileExpected);
await sgd.setVar("device.languages", "zpl", { allowRisky: false });
```

---

## 9. High‑Throughput Mode (Batch & Concurrency)

*(unchanged; adds enforcement of combo matrices during preflight and rendering quantization from render contract)*

---

## 11. VS Code Extension (MVP)

* Syntax + **semantic** highlighting; hovers from spec tables.
* Diagnostics with **quick‑fixes** (insert `^XZ`, add `^PW` / `^LL`, fix barcode module width/ratios).
* Live preview (WASM) with DPI / profile toggles, grids, safe areas.
* **Designer**: drag / drop primitives; round‑trip via generator; split view (code ↔ canvas).
* Templates: placeholder definitions + **Data Preview** panel (sample JSON).
* **Preflight wizard**; **visual diffs**; print commands & job console.
* Config UI; Rulelets manager; **Printer Twin** import/export.

**v1.3:** shows **plane/scope chips**, **mode banners** (prefix/language flips), resourceRef search‑order hovers, and **combo‑matrix** quick‑fix suggestions.

---

## 12. Print Service & Emulator (Ops‑grade)

*(unchanged capabilities; v1.3 adds typed query responses & combo‑matrix preflight enforcement)*

---

## 13. Security & Telemetry

*(unchanged; logs respect redaction; queries are parsed with schemas and never raw‑logged)*

---

## 14. Phased Delivery Plan

*(as in v1.2 with v1.3 mid‑phase items noted earlier: tokenizer mutations, render contract, combo matrices, query schemas, state models)*

---

## 15. Testing Strategy

* **Golden corpus** PNG diffs in CI (sharded & parallel).
* **Differential** vs devices (where feasible) and known engines (license permitting).
* **Fuzzing**: grammar fuzz + property tests (no panics, bounded memory/time).
* **Barcode verification**: decode rendered bitmaps; validate against selected standard(s) and quiet zones.
* **Performance gates**: render time/memory budgets; incremental parse/validate in editor.
* **Round‑trip**: `format(parse(zpl)) == zpl` under normalization.
* **ULIR** fixtures: parity across WASM/native.
* **Profile conformance**: per‑profile snapshots for `^PW` / `^LL` and font metrics.

**v1.3 expansions**

* Lexer‑mutation goldens (prefix/language change mid‑stream).
* Combo‑matrix suites per profile.
* Query parse goldens for `~HM`, `~HS`, `~HQES`.
* Serialization determinism (`^SN`) across preview/emulator/print.
* RenderContract parity (pixel rounding, module quantization, byte alignment) across targets.

---

## 16. Risks & Mitigations

* **Spec drift** → Separate spec package, PR review, coverage report, behavioral‑changes log.
* **WASM memory ceilings** → Chunked / streaming pipelines; documented complexity guidance.
* **Font metric mismatches** → Device‑font test suite; `^A@` optional in WASM; native harfbuzz feature.
* **Barcode edge cases** → Verify mode; strict lint rules; team‑specific rulelets.
* **USB complexity** → Prioritize network; add USB transports later per‑platform.
* **Rulelet sandboxing** → No network / FS, deterministic clock, CPU / memory guards; enterprise allowlists.
* **SVG fidelity** → Strict policy; outline text only on request; warn on non‑orthogonal image transforms.

**v1.3‑specific:**

* **Mode churn** → TokenizerConfig is content‑addressed; LSP mode banners reduce confusion.
* **Model‑dependent combos** → Enforced by combo matrices; downgrade hints in diagnostics.

**SGD‑specific:** dangerous ops require explicit flags; default read‑only; transport interleaving avoided; retries with jitter; secrets redacted; audit events for set/do.

---

## 17. MVP Definition

* **Core**: micro‑kernel, **Spec Compiler**, validator, renderer (PNG), `zplfmt`, CLI, WASM build.
* **Commands**: `^XA`/`^XZ`, `^PW`/`^LL`/`^LH`, `^FO`/`^FT`, `^FB`, `^FW`, `^A` (device fonts), `^BY`, `^BC`, `^BQN`, `^BX`, `^GB`, `^GF` (decode+encode).
* **Profiles**: ZD420 / ZD421 / ZD620 / ZD621 @ `203` & `300` dpi (portable‑203 strategy).
* **Editor**: LSP diagnostics + hovers + live preview; minimal designer; quick‑fixes; preflight.
* **Print**: TCP 9100 + emulator (job capture, faults) + Printer Twin.

---

## 18. Open Questions (pre‑Phase 2)

* **Barcode engine choice**: Which Rust crates meet compliance? Which symbologies need Zint fallback initially?
* **SVG text fidelity**: Criteria for outline vs. device font mapping in SVG.
* **Template rulelets API**: Minimal sandbox surface (I/O restrictions, caps).
* **Profile authoring**: Crowd‑source / verify profiles safely? Sign / verify process?
* **ULIR stability**: Versioning policy and guarantees for editor / generator / renderer integrations.

**Open Questions (SGD)**

* Minimal high‑value variable/action subset per model for MVP.
* Session apply/restore semantics after temporary changes.
* Browser transport bridge choice (BrowserPrint vs local helper) and packaging.
* Typed coercion rules for ints/floats/bools (canonical formatting) to avoid vendor quirks.

---

## 19. Appendix A — Command Registry Schema (v1.1.0, JSONC)

The canonical schema is stored as `zpl-spec.schema.jsonc` and **permits comments**. The spec‑compiler strips comments and **validates** against this schema. Keys beginning with `x-` are allowed anywhere for forward‑compatible experimentation.

```jsonc
{
  // ──────────────────────────────────────────────────────────────────────────────
  // Schema metadata
  // ──────────────────────────────────────────────────────────────────────────────
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://zpltoolchain.com/schema/zpl-spec.schema.jsonc",
  "title": "ZPL Command Registry (commented JSONC)",
  // NOTE: This file is JSONC; the spec-compiler strips comments before validation.

  "type": "object",
  "properties": {
    // Content version of this registry (not the schema version)
    "version": { "type": "string", "description": "Semver of this spec data set" },

    // Schema version bump for the new signature/codes/composites features
    "schemaVersion": { "type": "string", "const": "1.1.0" },

    // Optional extra metadata for docs/provenance
    "meta": { "type": "object", "additionalProperties": true },

    // The full list of ZPL/host commands
    "commands": {
      "type": "array",
      "description": "All ZPL/host commands in this registry",
      "items": { "$ref": "#/$defs/command" }
    }
  },
  "required": ["commands", "schemaVersion"],
  "additionalProperties": false,

  // Allow vendor/experimental fields anywhere
  "patternProperties": { "^x-": {} },

  "$defs": {
    // ────────────────────────────────────────────────────────────────────────────
    // Signature & composites (NEW in 1.1.0)
    // ────────────────────────────────────────────────────────────────────────────
    "signature": {
      "type": "object",
      "properties": {
        // Ordered short keys or composite names that appear after the opcode
        // Example: ["a","b","c"] for ^JJa,b,c  — or ["d:o.x","mx","my"] for ^XG
        "params": {
          "type": "array",
          "items": { "type": "string", "minLength": 1 },
          "minItems": 0
        },
        // How params are joined (ZPL default is ",")
        "joiner": { "type": "string", "default": "," },
        // ZPL format has no space after opcode (keep true for correctness)
        "noSpaceAfterOpcode": { "type": "boolean", "default": true },
        // If true, allow trailing empty params to preserve position (common in ZPL)
        "allowEmptyTrailing": { "type": "boolean", "default": true }
      },
      "required": ["params"],
      "additionalProperties": false
    },

    "composite": {
      "type": "object",
      "properties": {
        // Name used inside signature.params, e.g., "d:o.x"
        "name": { "type": "string", "minLength": 1 },
        // Template that references underlying args by key, e.g., "{d}:{o}.{x}"
        "template": { "type": "string", "minLength": 1 },
        // Which args (by key) are composed here (helps docs & validation)
        "exposesArgs": {
          "type": "array",
          "items": { "type": "string", "minLength": 1 },
          "minItems": 1
        },
        "doc": { "type": "string" }
      },
      "required": ["name", "template", "exposesArgs"],
      "additionalProperties": false
    },

    // ────────────────────────────────────────────────────────────────────────────
    // Command
    // ────────────────────────────────────────────────────────────────────────────
    "command": {
      "type": "object",
      "properties": {
        // NEW: list of opcodes that invoke this logical command (first = canonical)
        "codes": {
          "type": "array",
          "items": { "type": "string", "pattern": "^[\\^~][A-Z0-9]{1,3}$" },
          "minItems": 1
        },

        // BACK-COMPAT (deprecated): single code + aliases
        // If present, the compiler will normalize to `codes: [code, ...aliases]`.
        "code": { "type": "string", "pattern": "^[\\^~][A-Z0-9]{1,3}$" },
        "aliases": {
          "type": "array",
          "items": { "type": "string", "pattern": "^[\\^~][A-Z0-9]{1,3}$" }
        },

        "name": { "type": "string" },
        "category": { "type": "string" },                 // text, barcode, graphics, media, format, device
        "plane": {
          "enum": ["format", "device", "host"],
          "description": "format (label content), device (settings), or host (status)"
        },
        "scope": {
          "enum": ["document", "field", "job", "session"],
          "default": "field",
          "description": "Effect lifetime; e.g., field-scoped like ^A, or document (^PW)"
        },
        "effects": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Freeform side-effect tags (e.g., sets-current-font)"
        },
        "since": { "type": "string" },
        "deprecated": { "type": "boolean", "default": false },
        "deprecatedSince": { "type": "string" },
        "stability": { "enum": ["stable", "experimental", "deprecated"], "default": "stable" },

        // Arguments in Zebra's positional order
        "arity": { "type": "integer", "minimum": 0 },
        "args": {
          "type": "array",
          "items": { "$ref": "#/$defs/argUnion" },
          "description": "Positional arguments in Zebra’s published order"
        },

        // Command-level defaults (freeform bag; compiler understands common patterns)
        "defaults": { "type": "object", "additionalProperties": true },

        // Command-level unit context; args may override
        "units": { "type": "string" },

        // Validation/lint constraints
        "constraints": { "type": "array", "items": { "$ref": "#/$defs/constraint" } },

        // Capability gates matched by printer profiles
        "printerGates": { "type": "array", "items": { "type": "string" } },

        // Executable/doc examples
        "examples": { "type": "array", "items": { "$ref": "#/$defs/example" } },

        "docs": { "type": "string" },
        "extras": { "type": "object", "additionalProperties": true },

        // NEW: canonical parameterization for building the "Format:" string
        "signature": { "$ref": "#/$defs/signature" },

        // NEW: per-opcode overrides when twins differ slightly (rare)
        "signatureOverrides": {
          "type": "object",
          "additionalProperties": { "$ref": "#/$defs/signature" }
        },

        // NEW: path-like composites (e.g., d:o.x for ^XG, or d:f.x for ^A@)
        "composites": { "type": "array", "items": { "$ref": "#/$defs/composite" } }
      },

      // At least one of {codes} or {code} must be provided
      "anyOf": [
        { "required": ["codes"] },
        { "required": ["code"] }
      ],

      // Keep existing requirement for arity
      "required": ["arity"],
      "additionalProperties": false
    },

    // ────────────────────────────────────────────────────────────────────────────
    // Arg or Union of Args
    // ────────────────────────────────────────────────────────────────────────────
    "argUnion": {
      "description": "Either a single arg or a union of mutually exclusive arg shapes",
      "oneOf": [
        { "$ref": "#/$defs/arg" },
        {
          "type": "object",
          "properties": {
            "oneOf": {
              "type": "array",
              "minItems": 2,
              "items": { "$ref": "#/$defs/arg" }
            }
          },
          "required": ["oneOf"],
          "additionalProperties": false
        }
      ]
    },

    // ────────────────────────────────────────────────────────────────────────────
    // Arg
    // ────────────────────────────────────────────────────────────────────────────
    "arg": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },                     // Long name (e.g., "height")
        "key": { "type": "string" },                      // Short positional key (e.g., "h")

        // Primitive type
        "type": { "enum": ["int", "float", "enum", "bool", "string", "resourceRef", "char"] },

        // Unit/range for numeric args
        "unit": { "type": "string" },
        "range": {
          "type": "array",
          "minItems": 2,
          "maxItems": 2,
          "items": { "type": "number" }
        },

        // Enum values can be strings or objects with per-value gates
        "enum": {
          "type": "array",
          "items": {
            "oneOf": [
              { "type": "string" },
              {
                "type": "object",
                "properties": {
                  "value": { "type": "string" },
                  "printerGates": { "type": "array", "items": { "type": "string" } },
                  "extras": { "type": "object", "additionalProperties": true }
                },
                "required": ["value"],
                "additionalProperties": false
              }
            ]
          }
        },

        // Presence semantics (tri-state-ish)
        "optional": { "type": "boolean", "default": false },
        "presence": {
          "enum": ["unset", "empty", "value", "valueOrDefault", "emptyMeansUseDefault"],
          "description": "Clarifies how empty args are interpreted by firmware"
        },

        // Defaults: literal or dependency-based
        "default": {},
        "defaultFrom": { "type": "string", "description": "Dependency command or state (e.g., ^FW, ^CF)" },

        // Conditional numeric rules (handled by validator)
        "rangeWhen": { "type": "array", "items": { "$ref": "#/$defs/conditionalRange" } },

        // Rounding policy (global or conditional)
        "roundingPolicy": { "$ref": "#/$defs/roundingPolicy" },
        "roundingPolicyWhen": { "type": "array", "items": { "$ref": "#/$defs/conditionalRounding" } },

        // Resource ref specialization
        "resource": { "enum": ["graphic", "font", "any"] },

        // Extra gates & metadata
        "printerGates": { "type": "array", "items": { "type": "string" } },
        "extras": { "type": "object", "additionalProperties": true }
      },
      "required": ["name", "type"],
      "additionalProperties": false
    },

    // ────────────────────────────────────────────────────────────────────────────
    // Conditional range / rounding
    // ────────────────────────────────────────────────────────────────────────────
    "conditionalRange": {
      "type": "object",
      "properties": {
        "when": { "type": "string", "description": "DSL predicate label (e.g., 'fontIsBitmap')" },
        "range": {
          "type": "array",
          "minItems": 2,
          "maxItems": 2,
          "items": { "type": "number" }
        }
      },
      "required": ["when", "range"],
      "additionalProperties": false
    },

    "roundingPolicy": {
      "type": "object",
      "properties": {
        "unit": { "type": "string" },
        "mode": {
          "enum": [
            "nearest",
            "floor",
            "ceil",
            "ties-to-even",
            "toMultiple",
            "toNearestMultipleOfBaseHeight",
            "toNearestMultipleOfBaseWidth"
          ]
        },
        "multiple": { "type": "number" }
      },
      "required": ["mode"],
      "additionalProperties": false
    },

    "conditionalRounding": {
      "type": "object",
      "properties": {
        "when": { "type": "string" },
        "mode": { "type": "string" },
        "multiple": { "type": "number" }
      },
      "required": ["when", "mode"],
      "additionalProperties": false
    },

    // ────────────────────────────────────────────────────────────────────────────
    // Constraint (validation/lint)
    // ────────────────────────────────────────────────────────────────────────────
    "constraint": {
      "type": "object",
      "properties": {
        "kind": { "enum": ["order", "incompatible", "requires", "range", "note", "custom"] },
        "expr": { "type": "string", "description": "Small DSL condition; optional for kind=note/custom" },
        "message": { "type": "string" },
        "severity": { "enum": ["error", "warn", "info"], "default": "warn" },

        // For sandboxed WASM rulelets
        "wasmRuleletId": { "type": "string" },

        "extras": { "type": "object", "additionalProperties": true }
      },
      "required": ["kind", "message"],
      "additionalProperties": false
    },

    // ────────────────────────────────────────────────────────────────────────────
    // Example (docs + tests + snippets)
    // ────────────────────────────────────────────────────────────────────────────
    "example": {
      "type": "object",
      "properties": {
        "title": { "type": "string" },
        "zpl": { "type": "string" },
        "pngHash": { "type": "string", "description": "Optional BLAKE3 of rendered PNG" },
        "notes": { "type": "string" },
        "since": { "type": "string" },
        "profiles": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["zpl"],
      "additionalProperties": false
    }
  }
}
```

**Schema v1.1.0 highlights:** replaces `code`/`aliases` with `codes`, adds `signature` and `signatureOverrides` for exact **Format:** emission (**no space** after opcode), and introduces `composites` for path‑like segments (e.g., `{d}:{o}.{x}`).

### Modeling caret/tilde twins & path‑like params (authoring guidance)

* Use **`codes`** to keep twins in a single entry: `codes: ["^CC","~CC"]`.
* Provide one canonical **`signature`**; if one twin differs, put a per‑opcode **`signatureOverrides`** entry (rare).
* For path‑style segments like `^XG d:o.x` or `^A@ d:f.x`, declare a **`composites`** item with `template: "{d}:{o}.{x}"` and include the composite **name** in `signature.params`.

> **Migration 1.0 → 1.1.0**: replace `code`+`aliases` with `codes`; add `signature` (`params` ordered by Zebra docs, `joiner:","`, `noSpaceAfterOpcode:true`); introduce `composites` where applicable; use `signatureOverrides` only when twins differ.

---

## 20. Appendix B — ULIR Types (sketch)

```ts
export type IR = {
  page: { width_mm: number; height_mm: number; dpi: number };
  objects: IRObject[];
};

export type IRObject =
  | { kind: "text"; x_mm: number; y_mm: number; rotation: 0|90|180|270; font: DeviceFont; height_mm: number; text: string }
  | { kind: "barcode"; x_mm: number; y_mm: number; rotation: 0|90|180|270; symbology: Symb; params: Record<string, number|string|boolean>; data: string }
  | { kind: "line"; x1_mm: number; y1_mm: number; x2_mm: number; y2_mm: number; thickness_mm: number }
  | { kind: "rect"; x_mm: number; y_mm: number; w_mm: number; h_mm: number; radius_mm?: number; stroke_mm?: number; fill?: boolean }
  | { kind: "bitmap"; x_mm: number; y_mm: number; w_px: number; h_px: number; data: Uint8Array };
```

---

## 21. Open‑Source Survey → Design Mapping — preserved

* **zebrash / zpl‑renderer‑js / zpl‑js** → Offline rendering & editor UX are feasible; we add **formal validation**, **profiles**, and **spec tables**.
* **lprint / zpl‑print** → Discovery / spooling ideas; we keep a thin ZPL‑first client + integrated emulator.
* **PDF / Image → ZPL** → Adapter on top of our `^GF` encoder.
* **Editors / Designers** → Fold into VS Code + small web designer sharing **AST / ULIR / generator**.
* **VS Code labelary preview** → Replace with **WASM renderer** + profiles.

---

## 22. Suggested Repo Layout

```
zpl-toolchain/
  crates/
    core/                 # Rust: AST, validator, ULIR, renderer
    spec/                 # ZPL spec tables (JSON/TOML)
    spec-compiler/        # Generates validators, LSP artifacts, docs
    cli/                  # Rust CLI (zpl, zpl-format)
    sgd/                  # optional crate for SGD client & policy
  bindings/
    node/                 # WASM build + npm packages (@zpl-toolchain/*)
    python/               # maturin/pyo3 wheel (zpl_toolchain)
    dotnet/               # NuGet packaging, P/Invoke glue
    go/                   # cgo wrapper
  packages/               # TS utilities (editor, transports, etc.)
    sgd/                  # optional @zpl-toolchain/sgd
  tools/                  # release & build scripts
```

---

## 23. Checklist / Next Actions

* Finalize Rust MSRV & CI matrix (Linux / macOS / Windows; Node+browser WASM).
* Lock spec table schema; implement **Spec Compiler** scaffolding.
* Implement tokenizer / parser → **AST** with spans & comments.
* Implement validator passes (arity / type / range / unit; key cross‑checks).
* Define **ULIR** v0; implement minimal renderer (PNG; text / lines / Code128 / QR; `^GF` decode).
* Ship CLI v0 incl. `zpl-format` + `.zplrc.jsonc` schema.
* VS Code extension skeleton: LSP ping, diagnostics, preview panel using WASM renderer; wire completions from spec compiler.
* Curate golden corpus (~50 samples) for CI image diffs.
* Draft **Label ABI** schema + sample templates.

**SGD add‑ons:**

* Lock SGD registry schema; extend spec‑compiler to emit `sgd_bundle.json` + `sgd_rules.json`.
* Implement `sgd` wire parser & client with policy/allowlist and redaction.
* Update profiles with expected SGD defaults + drift severities.
* Add CLI `sgd` subcommands and LSP console; emulator KV‑store; CI record/replay.

---

## 24. End‑to‑End Journeys (DX/UX validation)

**Solo lab user**

* `npm i -g @zpl-toolchain/cli` → `zpl init`
* `zpl design` with live preview (WASM)
* `zpl print --profile zd620@203 --to 192.168.1.55 label.zpl`
* `zpl emulate` for at‑home testing

**Team CI (labels as code)**

* Commit templates + schema; `.zplrc.jsonc` enforces rules
* `zpl lint --strict && zpl render --golden`
* PR shows diagnostics, quick‑fixes, and PNG diffs

**Enterprise service (multi‑site)**

* Embed `@zpl-toolchain/sdk-*`; printer discovery + queues
* Central profiles + shareable **Printer Twin** JSONs
* Observability via job events; payload redaction by policy
* *(Optional)* SGD snapshot & drift policy tooling (read‑only by default)
