#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const fixturesDir = path.join(repoRoot, "contracts", "fixtures");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parseJson(text, filePath) {
  try {
    return JSON.parse(text);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Invalid JSON in ${filePath}: ${detail}`);
  }
}

function ensureVersionMatchesFilename(name, value) {
  const match = name.match(/\.v(\d+)\.json$/);
  assert(match, `${name}: fixture filename must end with .v<version>.json`);
  const versionFromName = Number.parseInt(match[1], 10);
  assert(
    Number.isInteger(value) && value === versionFromName,
    `${name}: top-level "version" (${value}) must match filename version (${versionFromName})`,
  );
}

function validateBindingsParityFixture(name, fixture) {
  assert(typeof fixture.profile === "object" && fixture.profile !== null, `${name}: missing "profile" object`);
  assert(Array.isArray(fixture.parse), `${name}: "parse" must be an array`);
  assert(Array.isArray(fixture.validate), `${name}: "validate" must be an array`);
  assert(fixture.parse.length > 0, `${name}: "parse" must include at least one case`);
  assert(fixture.validate.length > 0, `${name}: "validate" must include at least one case`);
}

function validatePrintStatusFramingFixture(name, fixture) {
  assert(Array.isArray(fixture.commands), `${name}: "commands" must be an array`);
  assert(typeof fixture.host_status === "object" && fixture.host_status !== null, `${name}: missing "host_status" object`);
  assert(typeof fixture.printer_info === "object" && fixture.printer_info !== null, `${name}: missing "printer_info" object`);

  const commands = new Map(
    fixture.commands.map((entry) => [entry.command, entry.expected_frame_count]),
  );
  assert(commands.get("~HS") === 3, `${name}: expected ~HS frame count must be 3`);
  assert(commands.get("~HI") === 1, `${name}: expected ~HI frame count must be 1`);

  assert(typeof fixture.host_status.healthy_raw === "string", `${name}: host_status.healthy_raw must be a string`);
  assert(typeof fixture.host_status.truncated_raw === "string", `${name}: host_status.truncated_raw must be a string`);
  assert(
    typeof fixture.host_status.expected_healthy === "object" && fixture.host_status.expected_healthy !== null,
    `${name}: host_status.expected_healthy must be an object`,
  );

  assert(typeof fixture.printer_info.raw === "string", `${name}: printer_info.raw must be a string`);
  assert(typeof fixture.printer_info.expected === "object" && fixture.printer_info.expected !== null, `${name}: printer_info.expected must be an object`);
}

function validatePrintJobLifecycleFixture(name, fixture) {
  assert(Array.isArray(fixture.phases), `${name}: "phases" must be an array`);
  const required = ["queued", "sending", "sent", "printing", "completed", "failed", "aborted"];
  for (const p of required) {
    assert(fixture.phases.includes(p), `${name}: phases must include "${p}"`);
  }
  assert(
    typeof fixture.deterministic_completion === "object" && fixture.deterministic_completion !== null,
    `${name}: deterministic_completion must be an object`,
  );
  assert(
    typeof fixture.deterministic_completion.sent === "string",
    `${name}: deterministic_completion.sent must be a string`,
  );
  assert(
    typeof fixture.deterministic_completion.completed === "string",
    `${name}: deterministic_completion.completed must be a string`,
  );
  assert(
    typeof fixture.deterministic_completion.timeout === "string",
    `${name}: deterministic_completion.timeout must be a string`,
  );
  assert(
    typeof fixture.job_id === "object" && fixture.job_id !== null,
    `${name}: job_id must be an object`,
  );
}

function validateFixture(name, fixture) {
  assert(typeof fixture === "object" && fixture !== null, `${name}: fixture root must be an object`);
  ensureVersionMatchesFilename(name, fixture.version);

  switch (name) {
    case "bindings-parity.v1.json":
      validateBindingsParityFixture(name, fixture);
      return;
    case "print-status-framing.v1.json":
      validatePrintStatusFramingFixture(name, fixture);
      return;
    case "print-job-lifecycle.v1.json":
      validatePrintJobLifecycleFixture(name, fixture);
      return;
    default:
      throw new Error(
        `${name}: no schema validator is registered for this fixture. ` +
          `Add a validate* function and a switch case in scripts/validate-contract-fixtures.mjs.`,
      );
  }
}

async function main() {
  const entries = await fs.readdir(fixturesDir, { withFileTypes: true });
  const files = entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
    .map((entry) => entry.name)
    .sort();

  assert(files.length > 0, "No contract fixtures found in contracts/fixtures.");

  for (const name of files) {
    const filePath = path.join(fixturesDir, name);
    const text = await fs.readFile(filePath, "utf8");
    const fixture = parseJson(text, filePath);
    validateFixture(name, fixture);
  }

  console.log(`Contract fixture validation passed (${files.length} fixture file(s)).`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
