import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import { PrintError } from "../types.js";

describe("PrintError", () => {
  it("has correct name and code", () => {
    const err = new PrintError("test message", "TIMEOUT");
    assert.equal(err.name, "PrintError");
    assert.equal(err.code, "TIMEOUT");
    assert.equal(err.message, "test message");
  });

  it("extends Error", () => {
    const err = new PrintError("test", "UNKNOWN");
    assert(err instanceof Error);
    assert(err instanceof PrintError);
  });

  it("carries cause", () => {
    const cause = new Error("original");
    const err = new PrintError("wrapped", "CONNECTION_REFUSED", cause);
    assert.equal(err.cause, cause);
  });

  it("has no cause when not provided", () => {
    const err = new PrintError("msg", "BROKEN_PIPE");
    assert.equal(err.cause, undefined);
  });
});
