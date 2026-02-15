declare module "../vendor/core/dist/index.js" {
  export function init(): Promise<void>;
  export function validate(
    input: string,
    profileJson?: string
  ): {
    ok: boolean;
    issues: Array<{
      id: string;
      severity: "error" | "warn" | "info";
      message: string;
      span?: { start: number; end: number };
      context?: Record<string, string> | Map<string, string>;
    }>;
    resolved_labels?: Array<{
      values: {
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
      };
      effective_width?: number | null;
      effective_height?: number | null;
    }>;
  };
  export function format(
    input: string,
    indent?: "none" | "label" | "field",
    compaction?: "none" | "field",
    commentPlacement?: "inline" | "line"
  ): string;
  export function explain(id: string): string | null;
}
