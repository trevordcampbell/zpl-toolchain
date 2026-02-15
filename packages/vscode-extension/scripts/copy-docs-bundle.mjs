import { mkdir, copyFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const packageRoot = path.resolve(__dirname, "..");
const source = path.resolve(packageRoot, "..", "..", "generated", "docs_bundle.json");
const resourcesDir = path.resolve(packageRoot, "resources");
const destination = path.resolve(resourcesDir, "docs_bundle.json");

await mkdir(resourcesDir, { recursive: true });
await copyFile(source, destination);
console.log(`Copied docs bundle to ${destination}`);
