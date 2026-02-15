import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { format, init } from "../index.js";

describe("@zpl-toolchain/core format compaction", () => {
  it("accepts compaction argument in format signature", () => {
    assert.throws(
      () => format("^XA^XZ", "label", "field"),
      /WASM not initialized/i,
    );
  });

  it("accepts comment placement argument in format signature", () => {
    assert.throws(
      () => format("^XA^XZ", "label", "field", "line"),
      /WASM not initialized/i,
    );
  });

  it("applies field compaction and inline comment placement", async () => {
    await init();
    const input = "^XA\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n";
    const formatted = format(input, "label", "field", "inline");
    assert.ok(
      /  \^FO30,30\^A0N,35,35\^FDWIDGET-3000\^FS/.test(formatted),
      "Expected field compaction to collapse printable field commands."
    );
  });

  it("preserves standalone semicolon comments when comment placement is line", async () => {
    await init();
    const input = "^XA\n^PW812\n; set print width\n^XZ\n";
    const formatted = format(input, "none", "none", "line");
    assert.ok(
      /\^PW812\n; set print width/.test(formatted),
      "Expected line comment placement to preserve standalone comment lines."
    );
  });
});
