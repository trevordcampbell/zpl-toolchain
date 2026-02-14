# zpl_toolchain_diagnostics

Shared diagnostic types for parser/validator/CLI.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Code generation
Diagnostic ID constants (`codes::ARITY`, etc.) and the `explain()` function are
**auto-generated** from `spec/diagnostics.jsonc` (this crate's
`crates/diagnostics/spec/diagnostics.jsonc`) at build time via `build.rs`.
This makes the JSONC file the single source of truth — adding or updating a
diagnostic only requires editing the spec, and the Rust code regenerates
automatically.

## Types
- `Span { start: usize, end: usize }` -- canonical byte-offset span, re-exported by `core::grammar::diag`.
- `Severity`: `Error | Warn | Info`. Implements `Display` (lowercase: `error`, `warn`, `info`).
- `Diagnostic { id, severity, message, span, context }` with convenience constructors: `Diagnostic::error()`, `::warn()`, `::info()`.
  - `context: Option<BTreeMap<String, String>>` — machine-readable structured metadata for tooling. Uses `BTreeMap` for deterministic key ordering in JSON output. Attach via the `.with_context(map)` builder method. Omitted from serialized JSON when `None`.
  - Implements `Display` — formats as `severity[id]: message` (e.g. `error[ZPL1101]: too many arguments`).
  - Derives `PartialEq`, `Eq` for easy test assertions and exhaustive equality checks.

## Functions
- `explain(code: &str) -> Option<&'static str>` -- human-readable explanation for all 46 diagnostic codes (auto-generated).
- `Diagnostic::explain(&self) -> Option<&'static str>` -- convenience method that calls the free `explain()` function with the diagnostic's own `id`.

## Guidance
- Use stable IDs: `ZPL1xxx` (value-level), `ZPL2xxx` (structural/semantic), `ZPL3xxx` (notes), `ZPL.PARSER.xxxx` (parser).
- Keep messages concise and actionable.
- To add a new diagnostic: add an entry to `spec/diagnostics.jsonc` (in this crate) with `id`, `constName`, `severity`, `category`, `summary`, `description`, and `contextKeys`.
- See `docs/DIAGNOSTIC_CODES.md` for the full reference.

