# @zpl-toolchain — <component> — Working Doc (vx.x)

> **Doc purpose:** Describe *what this component does*, *how it behaves*, *how to use it*, and *how we ship/test it*. Keep it deterministic, offline-first, and profile-aware.

## 0) Purpose & Scope

- One-paragraph summary of responsibilities and value.

- **Out of scope (MVP):** bullet list of intentional deferments.

- **Primary consumers:** CLI | VS Code | SDKs | services.

## 1) Goals, Non-Goals, Success Criteria

**Goals**

- <goal 1 tied to user value>

- <goal 2>

**Non-Goals (MVP)**

- <deferment 1>

**Done means true**

- <testable acceptance check 1>

- <testable acceptance check 2>

## 2) Interfaces & Contracts

**Inputs** (shape/schema + tiny example)

- <input 1>

**Outputs** (artifacts/APIs + tiny example)

- <output 1>

**Invariants**

- Deterministic, idempotent, profile-aware; no hidden network.

**Error model**

- Stable diagnostic IDs, severity (error|warn|info), retryability; quick-fix hooks if editor-facing.

**Inter-component dependencies**

- Upstream: <e.g., spec, profiles>

- Downstream: <e.g., renderer, CLI>

- Boundary rules: what’s generated vs. consumed; data → code lines.

## 3) Data Models & Schemas

- Core types (AST/ULIR pieces if relevant).

- Schema snippets + **versioning policy** (semver; breaking vs. additive).

- **Arg presence tri-state** (`unset | empty | value`) if applicable.

- **Profile & spec contract**: which profile/spec fields affect behavior.

## 4) Public API (per language) & CLI

> Provide **real, minimal, copy-runnable** examples for each language that this component ships for. If a language is not supported by this component, keep the header and add a one-liner: “Not applicable for this component.”

### Rust (library)

`// minimal example`

### TypeScript / Node (WASM or N-API)

`// minimal example`

### Python (pyo3 / maturin)

`# minimal example`

### Go (cgo)

`// minimal example`

### .NET (P/Invoke)

`// minimal example`

### CLI

`# minimal example`

**Conventions to include in every component:**

- Show how to **load spec/profile artifacts** if applicable (e.g., parser tables).

- Include **env var overrides** that affect this component (with sane values).

- Demonstrate both a **strict/CI** invocation and a **developer-friendly** invocation.

- If outputs write to disk, show the **outDir** and the location of key artifacts.

## 5) Behavior & Algorithms

`flowchart LR   Input --> Parse --> Validate --> Execute --> Output`

- Key algorithms; rounding/units policy; ordering rules.

- **Profile integration:** which profile fields change behavior.

- **Offline guarantee:** networking disallowed here.

- **Determinism controls:** canonical ordering, stable float formatting, content addressing.

## 6) Configuration

- **Precedence:** flags > env > project config > user config > defaults.

- Project config (file name & example).

- **Environment variables** (prefix `ZPL_<COMP>_...`).

- **Safe defaults:** strict linting, portable-203 DPI, no network.

## 7) Performance Targets & Limits

- Throughput/latency targets (dev warm cache & CI cold cache).

- Memory ceilings (WASM vs native) and concurrency model.

- Size budgets (e.g., bundles/pages).

- Complexity guardrails (e.g., max `^GF` bytes, objects/label, expression depth).

## 8) Security, Privacy, Compliance

- Logging redaction (no ZPL payloads by default).

- Sandbox constraints (no FS/network for WASM/rulelets).

- Supply chain: content-addressed manifest; optional signing; provenance block.

- PII/PHI: codepage/UTF-8 sanitization strategy if relevant.

## 9) UX & Developer Experience

- Stable diagnostic IDs + actionable messages + doc hops.

- Quick-fixes with preview metadata (if editor-facing).

- CLI ergonomics (`--json` for machines, human-first diffs).

- VS Code hooks (hovers, completions, live preview) if applicable.

## 10) Observability

- Tracing spans & counters; log levels; timing metrics.

- Event schemas for lifecycle (if emitted).

- Size/latency gates logged (for CI enforcement).

## 11) Testing Strategy

- Unit, property, fuzz.

- Golden corpus & (if rendering) image diffs.

- Native↔WASM parity where relevant.

- Real-device smoke tests when profiles/printers apply.

- **Determinism tests:** same inputs ⇒ same hashes.

## 12) Risks & Mitigations

- <top risk → mitigation, owner>

- <risk → mitigation, owner>

## 13) Implementation Stack

- **Primary language(s):** rationale.

- **Crates/libraries:** CLI, parsing, DSL, codegen, watch, hash/sign, diffs, docs, testing.

- **Custom modules:** responsibilities and boundaries.

- **Naming conventions:** binaries use descriptive `zpl-*` (e.g., `zpl-format`, `zpl-spec-compiler`); env vars `ZPL_<COMP>_*`.

## 14) File Layout (loose)

```
zpl-toolchain/
  crates/<component>/
    Cargo.toml
    src/
      ...
  tests/
  templates/           # if emitting code/docs
  generated/           # gitignored dev outputs
```

## 15) Build, CI, Release

- Build flags & features.

- CI matrix, cache keys, time budgets.

- Artifact manifest & version tagging; optional signing.

- Back-compat checks (API, diagnostics, bundle size caps).

## 16) Roadmap & Phases

- M1 (MVP), M2, M3 — scope for each.

- Deferments & feature flags.

## 17) Acceptance Criteria (checklist)

- Deterministic outputs; manifest hashes stable with stable inputs.

- Performance meets §7 targets.

- Tests meet §11 gates (coverage, goldens, parity, determinism).

- Security defaults enforced (§8).

- DX items (§9) implemented (IDs, quick-fixes, docs hops).

- Observability metrics emitted (§10) and size caps honored.

---

## Quick Reviewer Checklist

- Purpose clear & out-of-scope declared?

- Inputs/outputs precise with examples?

- Determinism/offline/profile awareness explicit?

- Stable diagnostic IDs + actionable messages?

- Perf budgets realistic & testable (incl. size caps)?

- Determinism tests & native↔WASM parity (if relevant)?

- Implementation stack & file layout identified?

- Roadmap sliced sensibly for M1/M2/M3?

- Naming fits `zpl-*` binary convention and `ZPL_<COMP>_*` env vars?

---

---

Recommended sequence of working doc generations:

1. **Spec Compiler** ✅(initial working doc completed)

2. **Grammar & Parser (AST)** ✅(initial working doc completed)

3. **Validator / Linter** ✅(initial working doc completed)

4. **ULIR executor**

5. **Renderer (PNG first)**

6. **Generator & template ABI**

7. **Print service & Emulator**

8. **VS Code (LSP) bundle & extension**

9. **CLI (“zpl”, incl. `zpl-format`)**

10. **SDK bindings (TS/Py/Go/.NET)**

11. **Transports & Resource Manager** (9100 first; profiles & on-device storage ops)
