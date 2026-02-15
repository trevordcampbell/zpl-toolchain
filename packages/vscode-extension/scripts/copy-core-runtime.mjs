import { mkdir, rm, copyFile, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const packageRoot = path.resolve(__dirname, "..");

const coreRoot = path.resolve(packageRoot, "..", "ts", "core");
const vendorRoot = path.resolve(packageRoot, "vendor", "core");
const sourceDist = path.resolve(coreRoot, "dist");
const sourceWasm = path.resolve(coreRoot, "wasm", "pkg");

await rm(vendorRoot, { recursive: true, force: true });
await mkdir(path.resolve(vendorRoot, "dist"), { recursive: true });
await mkdir(path.resolve(vendorRoot, "wasm", "pkg"), { recursive: true });

const sourceIndexPath = path.resolve(sourceDist, "index.js");
const vendorIndexPath = path.resolve(vendorRoot, "dist", "index.js");
let vendorIndexSource = await readFile(sourceIndexPath, "utf8");
// Normalize legacy/non-legacy dynamic import paths during vendoring.
vendorIndexSource = vendorIndexSource.replace(
  /import\("\.\.\/wasm\/pkg\/zpl_toolchain_wasm(?:\.js)?"\)/g,
  'import("../wasm/pkg/zpl_toolchain_wasm.js")'
);
await writeFile(vendorIndexPath, vendorIndexSource, "utf8");
await copyFile(
  path.resolve(sourceWasm, "zpl_toolchain_wasm.js"),
  path.resolve(vendorRoot, "wasm", "pkg", "zpl_toolchain_wasm.js")
);
await copyFile(
  path.resolve(sourceWasm, "zpl_toolchain_wasm_bg.js"),
  path.resolve(vendorRoot, "wasm", "pkg", "zpl_toolchain_wasm_bg.js")
);
await copyFile(
  path.resolve(sourceWasm, "zpl_toolchain_wasm_bg.wasm"),
  path.resolve(vendorRoot, "wasm", "pkg", "zpl_toolchain_wasm_bg.wasm")
);
await writeFile(
  path.resolve(vendorRoot, "wasm", "pkg", "zpl_toolchain_wasm.js"),
  `/* Node-compatible loader generated for VS Code extension runtime. */
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as wasmBg from "./zpl_toolchain_wasm_bg.js";

let initPromise = null;

async function initWasm() {
  if (initPromise) {
    return initPromise;
  }
  initPromise = (async () => {
    const wasmPath = path.resolve(
      path.dirname(fileURLToPath(import.meta.url)),
      "zpl_toolchain_wasm_bg.wasm"
    );
    const bytes = await readFile(wasmPath);
    const { instance } = await WebAssembly.instantiate(bytes, {
      "./zpl_toolchain_wasm_bg.js": wasmBg,
    });
    wasmBg.__wbg_set_wasm(instance.exports);
    instance.exports.__wbindgen_start();
  })();
  return initPromise;
}

await initWasm();
export { explain, format, parse, parseWithTables, validate } from "./zpl_toolchain_wasm_bg.js";
`,
  "utf8"
);
await writeFile(
  path.resolve(vendorRoot, "package.json"),
  `${JSON.stringify(
    {
      name: "@zpl-toolchain/core-vendored-runtime",
      private: true,
      type: "module",
    },
    null,
    2
  )}\n`,
  "utf8"
);

console.log(`Copied core runtime assets into ${vendorRoot}`);
