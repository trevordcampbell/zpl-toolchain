/**
 * @zpl-toolchain/core — TypeScript bindings for the ZPL toolchain.
 *
 * This package wraps the WASM build of the Rust ZPL toolchain, exposing
 * parse, validate, format, and explain functions with full TypeScript types.
 *
 * @example
 * ```ts
 * import { init, parse, format } from "@zpl-toolchain/core";
 *
 * // Initialize WASM (required before calling any function)
 * await init();
 *
 * const result = parse("^XA^FDHello^FS^XZ");
 * console.log(result.ast.labels);
 *
 * const formatted = format("^XA^FD Hello ^FS^XZ");
 * console.log(formatted);
 * ```
 */

// ── Types ───────────────────────────────────────────────────────────────

/** Byte range in the source input. */
export interface Span {
  start: number;
  end: number;
}

/** Argument presence state. Serialized as lowercase by Rust. */
export type Presence = "unset" | "empty" | "value";

/** A parsed argument slot. */
export interface ArgSlot {
  /** Parameter key name (absent when not defined by spec). */
  key?: string | null;
  /** Whether the argument was provided, empty, or not present. */
  presence: Presence;
  /** The argument value (absent when presence is "unset" or "empty"). */
  value?: string | null;
}

/**
 * AST node — discriminated on the `kind` field.
 *
 * Rust serializes `Node` with `#[serde(tag = "kind")]` (internally tagged),
 * producing JSON like `{"kind": "Command", "code": "^XA", ...}`.
 */
export type Node = CommandNode | FieldDataNode | RawDataNode | TriviaNode;

export interface CommandNode {
  kind: "Command";
  code: string;
  args: ArgSlot[];
  span: Span;
}

export interface FieldDataNode {
  kind: "FieldData";
  content: string;
  /** Whether `^FH` hex escapes have been applied. */
  hex_escaped: boolean;
  span: Span;
}

export interface RawDataNode {
  kind: "RawData";
  /** The command that initiated raw data collection (e.g., "^GF"). */
  command: string;
  /** Raw payload data (absent if command header had no trailing data). */
  data?: string | null;
  span: Span;
}

export interface TriviaNode {
  kind: "Trivia";
  text: string;
  span: Span;
}

/** A single ZPL label (^XA ... ^XZ block). */
export interface Label {
  nodes: Node[];
}

/** Top-level AST for a ZPL document. */
export interface Ast {
  labels: Label[];
}

/** Diagnostic severity level. Serialized as lowercase by Rust. */
export type Severity = "error" | "warn" | "info";

/** A diagnostic message from the parser or validator. */
export interface Diagnostic {
  id: string;
  severity: Severity;
  message: string;
  span?: Span;
  context?: Record<string, string>;
}

/** Result of parsing a ZPL string. */
export interface ParseResult {
  ast: Ast;
  diagnostics: Diagnostic[];
}

/** Typed defaults from `^BY`. */
export interface BarcodeDefaults {
  module_width?: number | null;
  ratio?: number | null;
  height?: number | null;
}

/** Typed defaults from `^CF`. */
export interface FontDefaults {
  font?: string | null;
  height?: number | null;
  width?: number | null;
}

/** Typed defaults from `^FW`. */
export interface FieldOrientationDefaults {
  orientation?: string | null;
  justification?: number | null;
}

/** Typed label-home from `^LH` (always present). */
export interface LabelHome {
  x: number;
  y: number;
}

/** Typed layout defaults (`^PW`, `^LL`, `^PO`, `^PM`, `^LR`, `^LT`, `^LS`). */
export interface LayoutDefaults {
  print_width?: number | null;
  label_length?: number | null;
  print_orientation?: string | null;
  mirror_image?: string | null;
  reverse_print?: string | null;
  label_top?: number | null;
  label_shift?: number | null;
}

/** Typed per-label state snapshot. */
export interface LabelValueState {
  barcode: BarcodeDefaults;
  font: FontDefaults;
  field: FieldOrientationDefaults;
  label_home: LabelHome;
  layout: LayoutDefaults;
}

/** Renderer-ready resolved label state from validator output. */
export interface ResolvedLabelState {
  values: LabelValueState;
  effective_width?: number | null;
  effective_height?: number | null;
}

/** Result of validating a ZPL string. */
export interface ValidationResult {
  ok: boolean;
  issues: Diagnostic[];
  resolved_labels?: ResolvedLabelState[];
}

/** Indentation style for the formatter. */
export type IndentStyle = "none" | "label" | "field";
/** Optional compaction mode for the formatter. */
export type CompactionStyle = "none" | "field";
// ── WASM Module ─────────────────────────────────────────────────────────

// The WASM module is loaded lazily. In a bundler environment (webpack, vite,
// etc.), the WASM file is typically handled as an asset. For Node.js, the
// WASM file must be on disk.
//
// We use dynamic import so the WASM binary is only fetched when init() is
// called, keeping the initial bundle lightweight.

let wasmModule: typeof import("../wasm/pkg/zpl_toolchain_wasm.js") | null = null;

/**
 * Initialize the WASM module. Must be called once before using any other
 * function. Safe to call multiple times (subsequent calls are no-ops).
 *
 * @example
 * ```ts
 * await init();
 * ```
 */
export async function init(): Promise<void> {
  if (wasmModule) return;

  // Dynamic import — bundlers will resolve the WASM package
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  wasmModule = await import("../wasm/pkg/zpl_toolchain_wasm.js");
}

function ensureInit(): NonNullable<typeof wasmModule> {
  if (!wasmModule) {
    throw new Error(
      "@zpl-toolchain/core: WASM not initialized. Call `await init()` first."
    );
  }
  return wasmModule;
}

function invokeWasm<T>(op: string, fn: () => T): T {
  try {
    return fn();
  } catch (error) {
    const message =
      error instanceof Error ? error.message : String(error ?? "unknown error");
    const wrapped = new Error(`@zpl-toolchain/core ${op} failed: ${message}`);
    if (error instanceof Error) {
      wrapped.cause = error;
    }
    throw wrapped;
  }
}

// ── Public API ──────────────────────────────────────────────────────────

/**
 * Parse a ZPL string and return the AST with diagnostics.
 *
 * Uses embedded parser tables for spec-driven parsing.
 */
export function parse(input: string): ParseResult {
  const wasm = ensureInit();
  return invokeWasm("parse", () => wasm.parse(input) as ParseResult);
}

/**
 * Parse a ZPL string with explicitly provided parser tables (JSON string).
 */
export function parseWithTables(
  input: string,
  tablesJson: string
): ParseResult {
  const wasm = ensureInit();
  return invokeWasm(
    "parseWithTables",
    () => wasm.parseWithTables(input, tablesJson) as ParseResult
  );
}

/**
 * Parse and validate a ZPL string.
 *
 * @param input ZPL source code.
 * @param profileJson Optional printer profile JSON string.
 */
export function validate(
  input: string,
  profileJson?: string
): ValidationResult {
  const wasm = ensureInit();
  return invokeWasm(
    "validate",
    () => wasm.validate(input, profileJson) as ValidationResult
  );
}

/**
 * Parse and validate a ZPL string with explicitly provided parser tables.
 *
 * @param input ZPL source code.
 * @param tablesJson Parser tables JSON string.
 * @param profileJson Optional printer profile JSON string.
 */
export function validateWithTables(
  input: string,
  tablesJson: string,
  profileJson?: string
): ValidationResult {
  const wasm = ensureInit();
  return invokeWasm(
    "validateWithTables",
    () => wasm.validateWithTables(input, tablesJson, profileJson) as ValidationResult
  );
}

/**
 * Format a ZPL string (normalize whitespace, one command per line).
 *
 * @param input ZPL source code.
 * @param indent Indentation style: "none" (default), "label", or "field".
 * @param compaction Optional compaction style: "none" (default) or "field".
 */
export function format(
  input: string,
  indent?: IndentStyle,
  compaction?: CompactionStyle
): string {
  const wasm = ensureInit();
  return invokeWasm("format", () => wasm.format(input, indent, compaction));
}

/**
 * Explain a diagnostic code (e.g., "ZPL1201").
 *
 * @returns The explanation string, or null if the code is unknown.
 */
export function explain(id: string): string | null {
  const wasm = ensureInit();
  return invokeWasm("explain", () => wasm.explain(id) ?? null);
}
