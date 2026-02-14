import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { explain, format, parse, validate, validateWithTables } from "../index.js";

describe("@zpl-toolchain/core init guard", () => {
  it("throws a clear error when parse() is called before init()", () => {
    assert.throws(
      () => parse("^XA^XZ"),
      /WASM not initialized/i,
    );
  });

  it("throws a clear error when validate() is called before init()", () => {
    assert.throws(
      () => validate("^XA^XZ"),
      /WASM not initialized/i,
    );
  });

  it("throws a clear error when format() is called before init()", () => {
    assert.throws(
      () => format("^XA^XZ"),
      /WASM not initialized/i,
    );
  });

  it("throws a clear error when explain() is called before init()", () => {
    assert.throws(
      () => explain("ZPL1201"),
      /WASM not initialized/i,
    );
  });

  it("throws a clear error when validateWithTables() is called before init()", () => {
    assert.throws(
      () => validateWithTables("^XA^XZ", "{}"),
      /WASM not initialized/i,
    );
  });
});
