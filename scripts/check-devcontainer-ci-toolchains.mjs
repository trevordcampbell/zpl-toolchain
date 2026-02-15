#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

async function readText(relativePath) {
  return fs.readFile(path.join(repoRoot, relativePath), "utf8");
}

function expectAllMatches(content, regex, label) {
  const matches = [...content.matchAll(regex)]
    .map((match) => match[1]?.trim())
    .filter((value) => Boolean(value));
  if (matches.length === 0) {
    throw new Error(`Unable to detect any ${label}.`);
  }
  return matches;
}

function normalizeDotnet(version) {
  return version.replace(/\.x$/i, "");
}

function normalizeGo(version) {
  return version.trim();
}

async function main() {
  const devcontainerRaw = await readText(".devcontainer/devcontainer.json");
  const ciRaw = await readText(".github/workflows/ci.yml");
  const devcontainer = JSON.parse(devcontainerRaw);
  const features = devcontainer.features ?? {};

  const devGo = features["ghcr.io/devcontainers/features/go:1"]?.version;
  const devDotnet = features["ghcr.io/devcontainers/features/dotnet:2"]?.version;
  if (!devGo || !devDotnet) {
    throw new Error("Missing Go/.NET feature versions in .devcontainer/devcontainer.json.");
  }

  const ciGoVersions = expectAllMatches(
    ciRaw,
    /go-version:\s*"([^"]+)"/g,
    "go-version entries in CI"
  );
  const ciDotnetVersions = expectAllMatches(
    ciRaw,
    /dotnet-version:\s*"([^"]+)"/g,
    "dotnet-version entries in CI"
  );

  const normalizedDevGo = normalizeGo(devGo);
  const normalizedDevDotnet = normalizeDotnet(devDotnet);
  for (const version of ciGoVersions) {
    if (normalizeGo(version) !== normalizedDevGo) {
      throw new Error(
        `Go version mismatch: devcontainer=${devGo}, ci includes ${ciGoVersions.join(", ")}.`
      );
    }
  }
  for (const version of ciDotnetVersions) {
    if (normalizeDotnet(version) !== normalizedDevDotnet) {
      throw new Error(
        `.NET version mismatch: devcontainer=${devDotnet}, ci includes ${ciDotnetVersions.join(", ")}.`
      );
    }
  }

  console.log(
    `Toolchain alignment check passed (Go ${devGo} across ${ciGoVersions.length} CI entries, .NET ${devDotnet} across ${ciDotnetVersions.length} CI entries).`
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
