import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import { print, printBatch, PrintError, TcpPrinter } from "../index.js";
import { createMockTcpServer } from "./mock-tcp-server.js";
import { canBindLocalTcp } from "./network-availability.js";

const NETWORK_INTEGRATION_AVAILABLE = await canBindLocalTcp();

async function waitFor(
  predicate: () => boolean,
  timeoutMs = 1000,
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return true;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  return predicate();
}

describe("print happy-path with mock TCP server", {
  skip: !NETWORK_INTEGRATION_AVAILABLE,
}, () => {
  it("sends ZPL and reports bytes written", async () => {
    const mock = await createMockTcpServer();
    try {
      const zpl = "^XA^FDHello^FS^XZ";
      const result = await print(zpl, {
        host: "127.0.0.1",
        port: mock.port,
        timeout: 1000,
      });

      assert.equal(result.success, true);
      assert.equal(result.bytesWritten, Buffer.byteLength(zpl));
      assert.ok(result.duration >= 0);
      assert.equal(
        await waitFor(() => mock.receivedPayloads.some((p) => p.includes("^XA^FDHello^FS^XZ"))),
        true,
      );
    } finally {
      await mock.close();
    }
  });

  it("retries once after transient connect/reset failure", async () => {
    const mock = await createMockTcpServer({ failConnectAttempts: 1 });
    try {
      const result = await print("^XA^FS^XZ", {
        host: "127.0.0.1",
        port: mock.port,
        timeout: 1000,
        maxRetries: 2,
        retryDelay: 10,
      });
      assert.equal(result.success, true);
    } finally {
      await mock.close();
    }
  });
});

describe("TcpPrinter/printBatch resilience", {
  skip: !NETWORK_INTEGRATION_AVAILABLE,
}, () => {
  it("printBatch handles mid-batch connection failure without throwing", async () => {
    const mock = await createMockTcpServer({ failAfterPayloads: 1 });
    try {
      const result = await printBatch(
        ["^XA^FD1^FS^XZ", "^XA^FD2^FS^XZ"],
        { host: "127.0.0.1", port: mock.port, timeout: 500, maxRetries: 0 },
      );
      assert.equal(result.total, 2);
      assert.equal(result.sent >= 0 && result.sent <= 2, true);
      if (result.error) {
        assert.equal(result.error.index >= 0 && result.error.index < result.total, true);
      }
    } finally {
      await mock.close().catch(() => undefined);
    }
  });

  it("aborts print() when signal is already aborted", async () => {
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(
      () =>
        print("^XA^XZ", {
          host: "127.0.0.1",
          port: 19999,
          signal: controller.signal,
        }),
      (err: unknown) => err instanceof PrintError && err.message.includes("aborted"),
    );
  });

  it("printBatch returns abort error when signal is aborted mid-batch", async () => {
    const mock = await createMockTcpServer();
    const controller = new AbortController();
    try {
      const result = await printBatch(
        ["^XA^FD1^FS^XZ", "^XA^FD2^FS^XZ"],
        { host: "127.0.0.1", port: mock.port, timeout: 500, maxRetries: 0 },
        {
          signal: controller.signal,
        },
        (progress) => {
          if (progress.sent === 1) controller.abort();
        },
      );
      assert.equal(result.sent, 1);
      assert.equal(result.total, 2);
      assert.equal(result.error?.message, "Operation aborted");
    } finally {
      await mock.close();
    }
  });

  it("aborts waitForCompletion() when signal is already aborted", async () => {
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999, timeout: 200, maxRetries: 0 });
    const controller = new AbortController();
    controller.abort();
    try {
      await assert.rejects(
        () => printer.waitForCompletion(100, 5000, controller.signal),
        (err: unknown) => err instanceof PrintError && err.message.includes("aborted"),
      );
    } finally {
      await printer.close();
    }
  });

  it("aborts getStatus() when signal is already aborted", async () => {
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999, timeout: 200, maxRetries: 0 });
    const controller = new AbortController();
    controller.abort();
    try {
      await assert.rejects(
        () => printer.getStatus({ signal: controller.signal }),
        (err: unknown) => err instanceof PrintError && err.message.includes("aborted"),
      );
    } finally {
      await printer.close();
    }
  });

  it("aborts query() when signal is already aborted", async () => {
    const printer = new TcpPrinter({ host: "127.0.0.1", port: 19999, timeout: 200, maxRetries: 0 });
    const controller = new AbortController();
    controller.abort();
    try {
      await assert.rejects(
        () => printer.query("~HI", { signal: controller.signal }),
        (err: unknown) => err instanceof PrintError && err.message.includes("aborted"),
      );
    } finally {
      await printer.close();
    }
  });
});
