#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile, chmod, rename, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

import { resolveTarget } from "../lib/platform.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const packageJsonPath = path.join(__dirname, "..", "package.json");
const packageJson = JSON.parse(await readFile(packageJsonPath, "utf8"));
const version = packageJson.version;

const target = resolveTarget(process.platform, process.arch);
if (!target) {
  const runtime = `${process.platform}/${process.arch}`;
  console.error(`Unsupported platform for @zpl-toolchain/cli: ${runtime}`);
  console.error("Supported targets:");
  console.error("  - linux/x64");
  console.error("  - darwin/arm64");
  console.error("  - win32/x64");
  console.error(
    "Use a pre-built release or cargo install zpl_toolchain_cli: https://github.com/trevordcampbell/zpl-toolchain/releases",
  );
  process.exit(1);
}

const baseUrl =
  process.env.ZPL_CLI_BASE_URL ??
  "https://github.com/trevordcampbell/zpl-toolchain/releases/download";
const tag = `v${version}`;
const archiveName = `zpl-${target.target}.${target.archiveExt}`;
const archiveUrl = `${baseUrl}/${tag}/${archiveName}`;
const checksumUrl = `${archiveUrl}.sha256`;

const cacheRoot = getCacheRoot();
const versionDir = path.join(cacheRoot, version, target.target);
const binaryPath = path.join(versionDir, target.binaryName);

try {
  await ensureBinary(binaryPath);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Failed to prepare zpl CLI binary: ${message}`);
  if (message.includes("Download failed (404)")) {
    console.error(
      "Release assets may still be publishing for this version. Retry in a few minutes.",
    );
  }
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});
if (result.error) {
  console.error(`Failed to execute zpl binary: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);

function getCacheRoot() {
  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA;
    if (localAppData) {
      return path.join(localAppData, "zpl-toolchain", "cli");
    }
  }
  const xdgCache = process.env.XDG_CACHE_HOME;
  if (xdgCache) {
    return path.join(xdgCache, "zpl-toolchain", "cli");
  }
  return path.join(os.homedir(), ".cache", "zpl-toolchain", "cli");
}

async function ensureBinary(expectedBinaryPath) {
  const existing = await statSafe(expectedBinaryPath);
  if (existing?.isFile()) {
    return;
  }

  const tempDir = path.join(versionDir, ".tmp");
  await mkdir(tempDir, { recursive: true });
  await mkdir(versionDir, { recursive: true });

  const archivePath = path.join(tempDir, archiveName);
  const extractedDir = path.join(tempDir, "extract");

  await downloadFile(archiveUrl, archivePath);
  await verifyChecksumIfAvailable(archivePath);
  await extractArchive(archivePath, extractedDir);

  const extractedBinary = path.join(extractedDir, target.binaryName);
  const extractedStat = await statSafe(extractedBinary);
  if (!extractedStat?.isFile()) {
    throw new Error(`Downloaded archive did not contain ${target.binaryName}`);
  }

  const finalTmpBinary = `${expectedBinaryPath}.tmp`;
  await mkdir(path.dirname(expectedBinaryPath), { recursive: true });
  await rm(finalTmpBinary, { force: true });
  await rename(extractedBinary, finalTmpBinary);
  if (process.platform !== "win32") {
    await chmod(finalTmpBinary, 0o755);
  }
  await rm(expectedBinaryPath, { force: true });
  await rename(finalTmpBinary, expectedBinaryPath);
}

async function downloadFile(url, outPath) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Download failed (${response.status}) for ${url}`);
  }
  const buf = Buffer.from(await response.arrayBuffer());
  await writeFile(outPath, buf);
}

async function verifyChecksumIfAvailable(archivePath) {
  const checksumResp = await fetch(checksumUrl);
  if (!checksumResp.ok) {
    return;
  }
  const text = await checksumResp.text();
  const expected = text.trim().split(/\s+/)[0]?.toLowerCase();
  if (!expected || expected.length < 64) {
    return;
  }
  const bytes = await readFile(archivePath);
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== expected) {
    throw new Error(`Checksum mismatch for ${archiveName}`);
  }
}

async function extractArchive(archivePath, outDir) {
  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });

  if (target.archiveExt === "tar.gz") {
    const result = spawnSync("tar", ["-xzf", archivePath, "-C", outDir], {
      stdio: "pipe",
      encoding: "utf8",
    });
    if (result.status !== 0) {
      throw new Error(`Failed to extract tar.gz archive: ${result.stderr || result.stdout}`);
    }
    return;
  }

  if (process.platform === "win32") {
    const psCommand = `Expand-Archive -LiteralPath '${escapePsPath(archivePath)}' -DestinationPath '${escapePsPath(outDir)}' -Force`;
    const result = spawnSync(
      "powershell",
      ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", psCommand],
      { stdio: "pipe", encoding: "utf8" },
    );
    if (result.status !== 0) {
      throw new Error(`Failed to extract zip archive: ${result.stderr || result.stdout}`);
    }
    return;
  }

  throw new Error(`Unsupported archive extraction for ${target.archiveExt} on ${process.platform}`);
}

function escapePsPath(value) {
  return value.replaceAll("'", "''");
}

async function statSafe(filePath) {
  try {
    return await stat(filePath);
  } catch {
    return null;
  }
}

