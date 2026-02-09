import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import { _processValidationResult, PrintError } from "../index.js";

describe("_processValidationResult", () => {
  it("does nothing when result has no issues", () => {
    // Should not throw
    _processValidationResult({ ok: true, issues: [] });
  });

  it("does nothing for null/undefined result", () => {
    _processValidationResult(null);
    _processValidationResult(undefined);
  });

  it("does nothing when result is a non-array non-object", () => {
    _processValidationResult("ok");
    _processValidationResult(42);
  });

  it("throws VALIDATION_FAILED for errors in {ok, issues} format", () => {
    const result = {
      ok: false,
      issues: [
        { severity: "error", message: "Unknown command ^ZZ" },
      ],
    };
    assert.throws(
      () => _processValidationResult(result),
      (err: unknown) => {
        assert(err instanceof PrintError);
        assert.equal(err.code, "VALIDATION_FAILED");
        assert(err.message.includes("Unknown command ^ZZ"));
        assert(err.message.includes("ZPL validation failed"));
        return true;
      }
    );
  });

  it("throws VALIDATION_FAILED for errors in array format", () => {
    const result = [
      { severity: "error", message: "Bad field data" },
    ];
    assert.throws(
      () => _processValidationResult(result),
      (err: unknown) => {
        assert(err instanceof PrintError);
        assert.equal(err.code, "VALIDATION_FAILED");
        assert(err.message.includes("Bad field data"));
        return true;
      }
    );
  });

  it("ignores warnings in non-strict mode", () => {
    const result = {
      ok: true,
      issues: [
        { severity: "warn", message: "Label width exceeds printhead" },
      ],
    };
    // Should NOT throw — warnings are allowed in non-strict mode
    _processValidationResult(result);
    _processValidationResult(result, { strict: false });
  });

  it("throws on warnings in strict mode", () => {
    const result = {
      ok: true,
      issues: [
        { severity: "warn", message: "Label width exceeds printhead" },
      ],
    };
    assert.throws(
      () => _processValidationResult(result, { strict: true }),
      (err: unknown) => {
        assert(err instanceof PrintError);
        assert.equal(err.code, "VALIDATION_FAILED");
        assert(err.message.includes("Label width exceeds printhead"));
        assert(err.message.includes("strict mode"));
        return true;
      }
    );
  });

  it("reports both errors and warnings in strict mode", () => {
    const result = {
      ok: false,
      issues: [
        { severity: "error", message: "Syntax error" },
        { severity: "warn", message: "Deprecated command" },
      ],
    };
    assert.throws(
      () => _processValidationResult(result, { strict: true }),
      (err: unknown) => {
        assert(err instanceof PrintError);
        // Should say "ZPL validation failed" (not "strict mode") because errors are present
        assert(err.message.includes("ZPL validation failed"));
        assert(err.message.includes("Syntax error"));
        assert(err.message.includes("Deprecated command"));
        return true;
      }
    );
  });

  it("handles diagnostics without message field", () => {
    const result = [{ severity: "error" }];
    assert.throws(
      () => _processValidationResult(result),
      (err: unknown) => {
        assert(err instanceof PrintError);
        assert(err.message.includes("unknown error"));
        return true;
      }
    );
  });

  it("joins multiple error messages with semicolons", () => {
    const result = [
      { severity: "error", message: "Unknown command ^ZZ" },
      { severity: "error", message: "Invalid field origin" },
      { severity: "error", message: "Unclosed ^XA" },
    ];
    assert.throws(
      () => _processValidationResult(result),
      (err: unknown) => {
        assert(err instanceof PrintError);
        assert(err.message.includes("Unknown command ^ZZ"));
        assert(err.message.includes("Invalid field origin"));
        assert(err.message.includes("Unclosed ^XA"));
        return true;
      }
    );
  });

  it("falls back to 'unknown error' for non-string message values", () => {
    const result = [
      { severity: "error", message: 42 },
      { severity: "error", message: true },
    ];
    assert.throws(
      () => _processValidationResult(result),
      (err: unknown) => {
        assert(err instanceof PrintError);
        assert(err.message.includes("unknown error"));
        return true;
      }
    );
  });

  it("skips non-error, non-warning severities", () => {
    const result = {
      ok: true,
      issues: [
        { severity: "info", message: "Using thermal transfer mode" },
        { severity: "note", message: "Consider using ^BY for barcode width" },
      ],
    };
    // info and note should be ignored even in strict mode
    _processValidationResult(result);
    _processValidationResult(result, { strict: true });
  });
});

describe("printValidated fallback", () => {
  it("attempts to print when @zpl-toolchain/core is not installed", async () => {
    // Since @zpl-toolchain/core is not installed, printValidated should
    // skip validation and proceed to print(). This will fail with
    // CONNECTION_REFUSED since there's no printer, but the important
    // thing is that it does NOT throw VALIDATION_FAILED.
    const { printValidated } = await import("../index.js");
    try {
      await printValidated("^XA^INVALID^XZ", {
        host: "127.0.0.1",
        port: 19999, // no printer here
        timeout: 500,
        maxRetries: 0,
      });
      assert.fail("Should have thrown");
    } catch (err: unknown) {
      assert(err instanceof PrintError);
      // Should be a connection error, NOT a validation error
      assert.notEqual(err.code, "VALIDATION_FAILED");
    }
  });
});
