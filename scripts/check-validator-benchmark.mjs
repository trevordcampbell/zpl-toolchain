#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const iterations = process.env.ZPL_BENCH_ITERS || "100";
const toleranceRatio = Number.parseFloat(
  process.env.ZPL_BENCH_TOLERANCE_RATIO || "0.10",
);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const thresholdsMs = {
  usps_surepost: 0.5,
  compliance: 0.2,
  trivial_empty: 0.05,
  trivial_non_indexed: 0.05,
  effect_heavy: 0.1,
  field_heavy: 0.1,
  mixed_effects_fields: 0.1,
};

const run = spawnSync(
  "cargo",
  ["run", "-p", "zpl_toolchain_core", "--example", "pipeline_benchmark", "--release"],
  {
    encoding: "utf8",
    cwd: repoRoot,
    env: { ...process.env, ZPL_BENCH_ITERS: iterations },
  },
);

if (run.status !== 0) {
  process.stderr.write(run.stdout || "");
  process.stderr.write(run.stderr || "");
  process.exit(run.status ?? 1);
}

const output = run.stdout || "";
const lines = output.split(/\r?\n/);
const observed = new Map();
let currentLabel = null;

for (const line of lines) {
  const benchMatch = /^Benchmark:\s+(.+)$/.exec(line);
  if (benchMatch) {
    currentLabel = benchMatch[1].trim();
    continue;
  }
  const validateMatch = /^\s+validate:\s+total=.*per_iter=([0-9.]+)\s+ms$/.exec(line);
  if (validateMatch && currentLabel) {
    observed.set(currentLabel, Number.parseFloat(validateMatch[1]));
  }
}

const missing = Object.keys(thresholdsMs).filter((label) => !observed.has(label));
if (missing.length > 0) {
  console.error(
    `Benchmark output missing required scenario(s): ${missing.join(", ")}\n` +
      "Raw benchmark output:\n" +
      output,
  );
  process.exit(1);
}

const regressions = [];
for (const [label, threshold] of Object.entries(thresholdsMs)) {
  const value = observed.get(label);
  const effectiveThreshold = threshold * (1 + toleranceRatio);
  if (value > effectiveThreshold) {
    regressions.push({ label, value, threshold: effectiveThreshold });
  }
}

const summaryLines = [
  "### Validator Benchmark Guardrail",
  "",
  `Iterations: ${iterations}`,
  `Tolerance ratio: ${Math.round(toleranceRatio * 100)}%`,
  "",
  "| Scenario | Validate ms/iter | Threshold ms/iter |",
  "|---|---:|---:|",
  ...Object.keys(thresholdsMs).map((label) => {
    const value = observed.get(label);
    const threshold = thresholdsMs[label];
    return `| ${label} | ${value.toFixed(3)} | ${threshold.toFixed(3)} |`;
  }),
];

const summaryPath = process.env.GITHUB_STEP_SUMMARY;
if (summaryPath) {
  appendFileSync(summaryPath, summaryLines.join("\n") + "\n");
}

if (regressions.length > 0) {
  for (const item of regressions) {
    console.error(
      `Benchmark regression: ${item.label} validate ${item.value.toFixed(3)}ms/iter ` +
        `exceeds threshold ${item.threshold.toFixed(3)}ms/iter`,
    );
  }
  console.error("\nRaw benchmark output:\n" + output);
  process.exit(1);
}

console.log("Validator benchmark guardrail passed.");
