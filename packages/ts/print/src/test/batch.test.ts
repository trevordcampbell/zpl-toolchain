import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import { TcpPrinter, PrintError } from "../index.js";
import type { BatchProgress, BatchResult } from "../types.js";

describe("TcpPrinter batch API", () => {
  it("printBatch sends empty batch and returns zero counts", async () => {
    // Use a printer that we can't connect to — empty batch should
    // never attempt a connection, so it should succeed immediately.
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999 });
    try {
      const result = await printer.printBatch([]);
      assert.equal(result.sent, 0);
      assert.equal(result.total, 0);
    } finally {
      await printer.close();
    }
  });

  it("printBatch returns partial error details on send failure", async () => {
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999, timeout: 200, maxRetries: 0 });
    const labels = ["^XA^XZ", "^XA^XZ", "^XA^XZ"];

    try {
      const result = await printer.printBatch(labels);
      assert.equal(result.sent, 0);
      assert.equal(result.total, labels.length);
      assert.ok(result.error);
      assert.equal(result.error?.index, 0);
      assert.notEqual(result.error?.code, "VALIDATION_FAILED");
    } finally {
      await printer.close();
    }
  });

  it("printBatch with statusInterval sends empty batch and returns zero counts", async () => {
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999 });
    try {
      const result = await printer.printBatch([], { statusInterval: 1 });
      assert.equal(result.sent, 0);
      assert.equal(result.total, 0);
    } finally {
      await printer.close();
    }
  });

  it("waitForCompletion throws TIMEOUT when printer is unreachable", async () => {
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999, timeout: 200, maxRetries: 0 });
    try {
      await printer.waitForCompletion(100, 500);
      assert.fail("Should have thrown");
    } catch (err: unknown) {
      assert(err instanceof PrintError);
      assert.equal(err.code, "TIMEOUT");
    } finally {
      await printer.close();
    }
  });
});

describe("BatchProgress / BatchResult types", () => {
  it("BatchProgress has correct shape", () => {
    const progress: BatchProgress = { sent: 3, total: 10 };
    assert.equal(progress.sent, 3);
    assert.equal(progress.total, 10);
    assert.equal(progress.status, undefined);
  });

  it("BatchProgress with status has correct shape", () => {
    const progress: BatchProgress = {
      sent: 5,
      total: 10,
      status: {
        ready: true,
        communicationFlag: 30,
        paperOut: false,
        paused: false,
        labelLengthDots: 1200,
        formatsInBuffer: 2,
        bufferFull: false,
        commDiagMode: false,
        partialFormat: false,
        reserved1: 0,
        corruptRam: false,
        underTemperature: false,
        overTemperature: false,
        functionSettings: 0,
        headOpen: false,
        ribbonOut: false,
        thermalTransferMode: false,
        printMode: 0,
        printWidthMode: 2,
        labelWaiting: false,
        labelsRemaining: 3,
        formatWhilePrinting: 0,
        graphicsStoredInMemory: 1,
        password: 1234,
        staticRamInstalled: false,
        raw: "",
      },
    };
    assert.equal(progress.status?.formatsInBuffer, 2);
    assert.equal(progress.status?.labelsRemaining, 3);
  });

  it("BatchResult has correct shape", () => {
    const result: BatchResult = { sent: 10, total: 10 };
    assert.equal(result.sent, 10);
    assert.equal(result.total, 10);
  });
});
