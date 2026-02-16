import * as path from "node:path";
import { pathToFileURL } from "node:url";

export type CoreSeverity = "error" | "warn" | "info";

export interface CoreSpan {
  start: number;
  end: number;
}

export interface CoreDiagnostic {
  id: string;
  severity: CoreSeverity;
  message: string;
  span?: CoreSpan;
  // serde-wasm-bindgen may materialize Rust maps as JS Map instances.
  context?: Record<string, string> | Map<string, string>;
}

export interface CoreLabelValueState {
  barcode: {
    module_width?: number | null;
    ratio?: number | null;
    height?: number | null;
  };
  font: {
    name?: string | null;
    height?: number | null;
    width?: number | null;
  };
  field: {
    orientation?: string | null;
    justification?: number | null;
  };
  label_home: {
    x: number;
    y: number;
  };
  layout: {
    print_width?: number | null;
    label_length?: number | null;
    print_orientation?: string | null;
    mirror_image?: string | null;
    reverse_print?: string | null;
    label_top?: number | null;
    label_shift?: number | null;
  };
}

export interface CoreResolvedLabelState {
  values: CoreLabelValueState;
  effective_width?: number | null;
  effective_height?: number | null;
}

export interface CoreValidationResult {
  ok: boolean;
  issues: CoreDiagnostic[];
  resolved_labels?: CoreResolvedLabelState[];
}

export interface ZplCoreApi {
  init(): Promise<void>;
  validate(input: string, profileJson?: string): CoreValidationResult;
  format(
    input: string,
    indent?: "none" | "label" | "field",
    compaction?: "none" | "field"
  ): string;
  explain(id: string): string | null;
}

let cachedApi: ZplCoreApi | null = null;
let initPromise: Promise<ZplCoreApi> | null = null;
let initError: Error | null = null;

/**
 * Lazily import the ESM-only core package from a CommonJS extension host.
 */
export async function getCoreApi(): Promise<ZplCoreApi> {
  if (cachedApi) {
    return cachedApi;
  }
  if (initError) {
    throw initError;
  }
  if (initPromise) {
    return initPromise;
  }

  initPromise = (async () => {
    try {
      const vendorEntry = path.resolve(__dirname, "../vendor/core/dist/index.js");
      // Use native dynamic import at runtime from CJS output.
      const nativeImport = new Function(
        "moduleUrl",
        "return import(moduleUrl);"
      ) as (moduleUrl: string) => Promise<unknown>;
      const mod = (await nativeImport(pathToFileURL(vendorEntry).href)) as ZplCoreApi;
      await mod.init();
      cachedApi = mod;
      return mod;
    } catch (error) {
      initError =
        error instanceof Error
          ? error
          : new Error(`Failed to initialize core API: ${String(error)}`);
      throw initError;
    } finally {
      initPromise = null;
    }
  })();

  return initPromise;
}
