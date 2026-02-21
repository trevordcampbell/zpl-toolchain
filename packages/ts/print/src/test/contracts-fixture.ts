import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixtureRoot = path.resolve(__dirname, "../../../../../contracts/fixtures");

export interface PrintStatusFramingFixture {
  version: number;
  commands: Array<{ command: string; expected_frame_count: number }>;
  host_status: {
    healthy_raw: string;
    truncated_raw: string;
    expected_healthy: {
      paper_out: boolean;
      paused: boolean;
      head_up: boolean;
      ribbon_out: boolean;
      formats_in_buffer: number;
      labels_remaining: number;
      print_mode: string;
    };
  };
  printer_info: {
    raw: string;
    expected: {
      model: string;
      firmware: string;
      dpi: number;
      memory_kb: number;
    };
  };
}

export function loadPrintStatusFramingFixture(): PrintStatusFramingFixture {
  const fixturePath = path.join(fixtureRoot, "print-status-framing.v1.json");
  const raw = fs.readFileSync(fixturePath, "utf-8");
  return JSON.parse(raw) as PrintStatusFramingFixture;
}

export interface PrintJobLifecycleFixture {
  version: number;
  phases: string[];
}

export function loadPrintJobLifecycleFixture(): PrintJobLifecycleFixture {
  const fixturePath = path.join(fixtureRoot, "print-job-lifecycle.v1.json");
  const raw = fs.readFileSync(fixturePath, "utf-8");
  return JSON.parse(raw) as PrintJobLifecycleFixture;
}
