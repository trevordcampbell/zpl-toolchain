import test from "node:test";
import assert from "node:assert/strict";

import { resolveTarget } from "../lib/platform.mjs";

test("resolveTarget maps supported runtimes", () => {
  assert.deepEqual(resolveTarget("linux", "x64"), {
    target: "x86_64-unknown-linux-gnu",
    archiveExt: "tar.gz",
    binaryName: "zpl",
  });
  assert.deepEqual(resolveTarget("darwin", "arm64"), {
    target: "aarch64-apple-darwin",
    archiveExt: "tar.gz",
    binaryName: "zpl",
  });
  assert.deepEqual(resolveTarget("win32", "x64"), {
    target: "x86_64-pc-windows-msvc",
    archiveExt: "zip",
    binaryName: "zpl.exe",
  });
});

test("resolveTarget returns null for unsupported runtimes", () => {
  assert.equal(resolveTarget("darwin", "x64"), null);
  assert.equal(resolveTarget("linux", "arm64"), null);
});

