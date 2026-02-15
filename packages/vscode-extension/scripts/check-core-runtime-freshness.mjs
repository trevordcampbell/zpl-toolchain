import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const extensionRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(extensionRoot, "..", "..");

const wasmArtifactPath = path.resolve(
  repoRoot,
  "packages",
  "ts",
  "core",
  "wasm",
  "pkg",
  "zpl_toolchain_wasm_bg.wasm"
);
const tsCoreDistPath = path.resolve(
  repoRoot,
  "packages",
  "ts",
  "core",
  "dist",
  "index.js"
);
const parserTablesPath = path.resolve(
  repoRoot,
  "crates",
  "cli",
  "data",
  "parser_tables.json"
);

const rustSourceRoots = [
  path.resolve(repoRoot, "crates", "core", "src"),
  path.resolve(repoRoot, "crates", "wasm", "src"),
  path.resolve(repoRoot, "crates", "bindings-common", "src"),
];
const tsCoreSourceRoot = path.resolve(repoRoot, "packages", "ts", "core", "src");

async function newestMtimeMs(rootPath, allowedExtensions) {
  let newest = 0;
  async function walk(currentPath) {
    const entries = await readdir(currentPath, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.resolve(currentPath, entry.name);
      if (entry.isDirectory()) {
        await walk(fullPath);
        continue;
      }
      if (!entry.isFile()) {
        continue;
      }
      if (!allowedExtensions.includes(path.extname(entry.name))) {
        continue;
      }
      const fileStat = await stat(fullPath);
      newest = Math.max(newest, fileStat.mtimeMs);
    }
  }

  await walk(rootPath);
  return newest;
}

async function statMtimeMs(filePath) {
  return (await stat(filePath)).mtimeMs;
}

function failWith(message, remediation) {
  console.error(`\ncheck:core-runtime-freshness failed: ${message}\n`);
  if (remediation) {
    console.error(`${remediation}\n`);
  }
  process.exit(1);
}

const rustNewest = Math.max(
  ...(await Promise.all(rustSourceRoots.map((root) => newestMtimeMs(root, [".rs"]))))
);
const parserTablesMtime = await statMtimeMs(parserTablesPath);
const tsCoreSourceNewest = await newestMtimeMs(tsCoreSourceRoot, [".ts"]);

const wasmArtifactMtime = await statMtimeMs(wasmArtifactPath);
const tsCoreDistMtime = await statMtimeMs(tsCoreDistPath);

if (wasmArtifactMtime < rustNewest || wasmArtifactMtime < parserTablesMtime) {
  failWith(
    "WASM runtime is older than Rust source/spec tables. Extension packaging would vendor stale diagnostics/format behavior.",
    [
      "Rebuild TS core WASM runtime first:",
      '  wasm-pack build "crates/wasm" --target bundler --out-dir "../../packages/ts/core/wasm/pkg"',
      "  cd packages/ts/core && npm run build",
    ].join("\n")
  );
}

if (tsCoreDistMtime < tsCoreSourceNewest) {
  failWith(
    "packages/ts/core/dist/index.js is older than packages/ts/core/src/*.ts.",
    "Rebuild TS core package first:\n  cd packages/ts/core && npm run build"
  );
}

console.log("check:core-runtime-freshness passed");
