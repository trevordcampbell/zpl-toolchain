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

  it("applies field compaction", async () => {
    await init();
    const input = "^XA\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n";
    const formatted = format(input, "label", "field");
    assert.ok(
      /  \^FO30,30\^A0N,35,35\^FDWIDGET-3000\^FS/.test(formatted),
      "Expected field compaction to collapse printable field commands."
    );
  });

  it("treats semicolon as plain data (no comment semantics)", async () => {
    await init();
    const input = "^XA\n^FO10,10^FDPart;A^FS\n^XZ\n";
    const formatted = format(input, "none", "none");
    assert.ok(
      formatted.includes("Part;A"),
      "Expected semicolon to be preserved as normal field data."
    );
  });
});
