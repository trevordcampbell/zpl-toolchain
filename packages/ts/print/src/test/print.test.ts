import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import net from "node:net";
import { print, printBatch, PrintError, TcpPrinter, tcpQuery } from "../index.js";
import { createMockTcpServer } from "./mock-tcp-server.js";
import { canBindLocalTcp } from "./network-availability.js";
import { loadPrintStatusFramingFixture } from "./contracts-fixture.js";

const NETWORK_INTEGRATION_AVAILABLE = await canBindLocalTcp();
const fixture = loadPrintStatusFramingFixture();
const hsExpectedFrameCount =
  fixture.commands.find((entry) => entry.command === "~HS")?.expected_frame_count ?? 3;

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
  it("getStatus reads bursty framed ~HS responses without truncation", async () => {
    const server = net.createServer((socket) => {
      socket.on("data", async (data) => {
        const payload = data.toString("utf-8");
        if (!payload.includes("~HS")) {
          socket.end();
          return;
        }
        const frames = fixture.host_status.healthy_raw.split("\x03").filter(Boolean).map((f) => `${f}\x03`);
        socket.write(frames[0]);
        await new Promise((resolve) => setTimeout(resolve, 325));
        socket.write(frames[1]);
        await new Promise((resolve) => setTimeout(resolve, 325));
        socket.write(frames[2]);
        socket.end();
      });
    });

    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", () => resolve());
    });
    const address = server.address();
    if (!address || typeof address === "string") {
      server.close();
      throw new Error("Failed to bind test server");
    }

    const printer = new TcpPrinter({
      host: "127.0.0.1",
      port: address.port,
      timeout: 2500,
      maxRetries: 0,
    });
    try {
      const status = await printer.getStatus();
      assert.equal(status.ready, true);
      assert.equal(status.partialFormat, false);
      assert.equal(status.labelsRemaining, 0);
    } finally {
      await printer.close();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  });

  it("tcpQuery rejects framed ~HS responses with wrong frame count", async () => {
    const server = net.createServer((socket) => {
      socket.on("data", (data) => {
        const payload = data.toString("utf-8");
        if (!payload.includes("~HS")) {
          socket.end();
          return;
        }
        socket.write(fixture.host_status.truncated_raw);
        socket.end();
      });
    });

    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", () => resolve());
    });
    const address = server.address();
    if (!address || typeof address === "string") {
      server.close();
      throw new Error("Failed to bind test server");
    }

    try {
      await assert.rejects(
        () => tcpQuery("127.0.0.1", address.port, "~HS", 1000),
        (err: unknown) =>
          err instanceof PrintError &&
          err.message.includes(`returned 2 frames (expected ${hsExpectedFrameCount})`)
      );
    } finally {
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  });

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

  it("printBatch returns abort error when progress callback aborts", async () => {
    const mock = await createMockTcpServer();
    try {
      const result = await printBatch(
        ["^XA^FD1^FS^XZ", "^XA^FD2^FS^XZ"],
        { host: "127.0.0.1", port: mock.port, timeout: 500, maxRetries: 0 },
        undefined,
        (progress) => {
          if (progress.phase === "queued") return;
          if (progress.sent >= 1) return false;
        },
      );
      assert.equal(result.sent, 1);
      assert.equal(result.total, 2);
      assert.equal(result.error?.message, "Operation aborted");
      assert.equal(result.error?.index, 1);
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
