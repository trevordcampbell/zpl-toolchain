import { describe, it, before, after } from "node:test";
import { strict as assert } from "node:assert";
import http from "node:http";
import WebSocket from "ws";
import { createPrintProxy } from "../proxy.js";
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

// Helper to make HTTP requests to the proxy
function request(
  port: number,
  method: string,
  path: string,
  body?: unknown,
  headers?: Record<string, string>
): Promise<{ status: number; body: unknown; headers: http.IncomingHttpHeaders }> {
  return new Promise((resolve, reject) => {
    const data = body ? JSON.stringify(body) : undefined;
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        path,
        method,
        headers: data
          ? {
              ...(headers ?? {}),
              "Content-Type": "application/json",
              "Content-Length": Buffer.byteLength(data),
            }
          : headers ?? {},
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => {
          const raw = Buffer.concat(chunks).toString("utf-8");
          try {
            resolve({ status: res.statusCode!, body: JSON.parse(raw), headers: res.headers });
          } catch {
            resolve({ status: res.statusCode!, body: raw, headers: res.headers });
          }
        });
      }
    );
    req.on("error", reject);
    if (data) req.write(data);
    req.end();
  });
}

describe("createPrintProxy", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  it("health endpoint returns ok", async () => {
    const { server, close } = await createPrintProxy({
      port: 0, // random available port
      allowedPrinters: ["test"],
    });
    try {
      const address = server.address();
      if (!address || typeof address === "string") {
        assert.fail("expected proxy server to bind to a numeric port");
      }
      const res = await request(address.port, "GET", "/health");
      assert.equal(res.status, 200);
      assert.deepEqual(res.body, { ok: true });
    } finally {
      await close();
    }
  });
});

describe("createPrintProxy - endpoints", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  let closeProxy: () => Promise<void>;
  const PORT = 18901; // high port unlikely to collide

  after(async () => {
    if (closeProxy) await closeProxy();
  });

  it("starts and responds to health check", async () => {
    const { close } = await createPrintProxy({
      port: PORT,
      allowedPrinters: ["192.168.1.100"],
    });
    closeProxy = close;

    const res = await request(PORT, "GET", "/health");
    assert.equal(res.status, 200);
    assert.deepEqual(res.body, { ok: true });
  });

  it("rejects non-POST methods", async () => {
    const res = await request(PORT, "GET", "/print");
    assert.equal(res.status, 405);
  });

  it("rejects missing content-type", async () => {
    const res = await new Promise<{ status: number; body: unknown }>((resolve, reject) => {
      const req = http.request(
        { hostname: "127.0.0.1", port: PORT, path: "/print", method: "POST" },
        (res) => {
          const chunks: Buffer[] = [];
          res.on("data", (c: Buffer) => chunks.push(c));
          res.on("end", () => resolve({
            status: res.statusCode!,
            body: JSON.parse(Buffer.concat(chunks).toString("utf-8")),
          }));
        }
      );
      req.on("error", reject);
      req.write("{}");
      req.end();
    });
    assert.equal(res.status, 415);
  });

  it("rejects invalid JSON", async () => {
    const res = await new Promise<{ status: number; body: unknown }>((resolve, reject) => {
      const data = "not json{{{";
      const req = http.request(
        {
          hostname: "127.0.0.1",
          port: PORT,
          path: "/print",
          method: "POST",
          headers: { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(data) },
        },
        (res) => {
          const chunks: Buffer[] = [];
          res.on("data", (c: Buffer) => chunks.push(c));
          res.on("end", () => resolve({
            status: res.statusCode!,
            body: JSON.parse(Buffer.concat(chunks).toString("utf-8")),
          }));
        }
      );
      req.on("error", reject);
      req.write(data);
      req.end();
    });
    assert.equal(res.status, 400);
  });

  it("rejects missing printer field", async () => {
    const res = await request(PORT, "POST", "/print", { zpl: "^XA^XZ" });
    assert.equal(res.status, 400);
  });

  it("rejects missing zpl field", async () => {
    const res = await request(PORT, "POST", "/print", {
      printer: "192.168.1.100",
    });
    assert.equal(res.status, 400);
  });

  it("rejects disallowed printer (SSRF protection)", async () => {
    const res = await request(PORT, "POST", "/print", {
      printer: "evil.com",
      zpl: "^XA^XZ",
    });
    assert.equal(res.status, 403);
  });

  it("rejects invalid port number", async () => {
    const res = await request(PORT, "POST", "/print", {
      printer: "192.168.1.100",
      zpl: "^XA^XZ",
      port: 99999,
    });
    assert.equal(res.status, 400);
  });

  it("returns 404 for unknown routes", async () => {
    const res = await request(PORT, "POST", "/unknown", { test: true });
    assert.equal(res.status, 404);
  });

  it("rejects empty allowedPrinters (SSRF)", async () => {
    const { close } = await createPrintProxy({
      port: 18902,
      allowedPrinters: [],
    });

    const res = await request(18902, "POST", "/print", {
      printer: "192.168.1.100",
      zpl: "^XA^XZ",
    });
    assert.equal(res.status, 403);

    await close();
  });
});

describe("createPrintProxy - wildcard allowlist", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  it("allows printers matching a wildcard pattern", async () => {
    const { close } = await createPrintProxy({
      port: 18904,
      allowedPrinters: ["127.0.0.*"],
    });

    // Should be allowed (matches 127.0.0.*)
    // Uses 127.0.0.1 so the TCP connection fails instantly with ECONNREFUSED
    // instead of timing out on an unreachable remote IP (~16s).
    const res = await request(18904, "POST", "/print", {
      printer: "127.0.0.1",
      zpl: "^XA^XZ",
    });
    // Will get 502 (can't connect to printer) but NOT 403
    assert.notEqual(res.status, 403);

    await close();
  });

  it("rejects printers not matching wildcard pattern", async () => {
    const { close } = await createPrintProxy({
      port: 18905,
      allowedPrinters: ["192.168.1.*"],
    });

    const res = await request(18905, "POST", "/print", {
      printer: "10.0.0.1",
      zpl: "^XA^XZ",
    });
    assert.equal(res.status, 403);

    await close();
  });

  it("supports multiple patterns", async () => {
    const { close } = await createPrintProxy({
      port: 18906,
      allowedPrinters: ["10.0.0.*", "127.0.0.*"],
    });

    // Should match second pattern (127.0.0.*)
    // Uses 127.0.0.1 so the TCP connection fails instantly with ECONNREFUSED
    // instead of timing out on DNS resolution for a non-existent hostname.
    const res = await request(18906, "POST", "/print", {
      printer: "127.0.0.1",
      zpl: "^XA^XZ",
    });
    assert.notEqual(res.status, 403);

    await close();
  });
});

describe("createPrintProxy - CORS", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  it("handles OPTIONS preflight", async () => {
    const { close } = await createPrintProxy({
      port: 18903,
      allowedPrinters: ["test"],
    });

    const res = await request(18903, "OPTIONS", "/print");
    assert.equal(res.status, 204);

    await close();
  });

  it("echoes allowed Origin and omits disallowed Origin", async () => {
    const port = 18907;
    const { close } = await createPrintProxy({
      port,
      allowedPrinters: ["127.0.0.1"],
      cors: ["https://allowed.example"],
    });
    try {
      const allowed = await request(
        port,
        "OPTIONS",
        "/print",
        undefined,
        { Origin: "https://allowed.example" },
      );
      assert.equal(allowed.status, 204);
      assert.equal(allowed.headers["access-control-allow-origin"], "https://allowed.example");
      assert.equal(allowed.headers.vary, "Origin");

      const denied = await request(
        port,
        "OPTIONS",
        "/print",
        undefined,
        { Origin: "https://denied.example" },
      );
      assert.equal(denied.status, 204);
      assert.equal(denied.headers["access-control-allow-origin"], undefined);
    } finally {
      await close();
    }
  });
});

// ── WebSocket tests ─────────────────────────────────────────────────────────

function wsRequest(port: number, msg: unknown): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    const timer = setTimeout(() => {
      ws.close();
      reject(new Error("wsRequest timed out after 10s"));
    }, 10_000);
    ws.on("open", () => {
      ws.send(JSON.stringify(msg));
    });
    ws.on("message", (data: Buffer) => {
      clearTimeout(timer);
      ws.close();
      resolve(JSON.parse(data.toString("utf-8")) as Record<string, unknown>);
    });
    ws.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
  });
}

describe("createPrintProxy - WebSocket", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  let closeProxy: () => Promise<void>;
  const PORT = 18910;

  after(async () => {
    if (closeProxy) await closeProxy();
  });

  before(async () => {
    const { close } = await createPrintProxy({
      port: PORT,
      allowedPrinters: ["192.168.1.100", "192.168.1.*"],
    });
    closeProxy = close;
  });

  it("rejects invalid JSON over WebSocket", async () => {
    const res = await new Promise<Record<string, unknown>>((resolve, reject) => {
      const ws = new WebSocket(`ws://127.0.0.1:${PORT}`);
      ws.on("open", () => {
        ws.send("not valid json{{{");
      });
      ws.on("message", (data: Buffer) => {
        ws.close();
        resolve(JSON.parse(data.toString("utf-8")) as Record<string, unknown>);
      });
      ws.on("error", reject);
    });
    assert.equal(res.ok, false);
    assert.equal(res.error, "Invalid JSON");
  });

  it("rejects WebSocket origin when CORS allowlist does not include it", async () => {
    const port = 18911;
    const { close } = await createPrintProxy({
      port,
      allowedPrinters: ["127.0.0.1"],
      cors: ["https://allowed.example"],
    });
    try {
      const rejected = await new Promise<boolean>((resolve) => {
        const ws = new WebSocket(`ws://127.0.0.1:${port}`, {
          headers: { Origin: "https://denied.example" },
        });
        ws.once("unexpected-response", (_req, res) => {
          resolve(res.statusCode === 403);
          ws.terminate();
        });
        ws.once("open", () => {
          resolve(false);
          ws.terminate();
        });
        ws.once("error", () => {
          resolve(true);
          ws.terminate();
        });
      });
      assert.equal(rejected, true);
    } finally {
      await close();
    }
  });

  it("rejects missing type field", async () => {
    const res = await wsRequest(PORT, { printer: "192.168.1.100" });
    assert.equal(res.ok, false);
    assert.match(res.error as string, /Missing or invalid "type" field/);
  });

  it("rejects disallowed printer (SSRF)", async () => {
    const res = await wsRequest(PORT, {
      type: "print",
      printer: "evil.com",
      zpl: "^XA^XZ",
    });
    assert.equal(res.ok, false);
    assert.match(res.error as string, /not in the allowed list/);
  });

  it("rejects invalid port number", async () => {
    const res = await wsRequest(PORT, {
      type: "print",
      printer: "192.168.1.100",
      zpl: "^XA^XZ",
      port: 99999,
    });
    assert.equal(res.ok, false);
    assert.match(res.error as string, /Port must be an integer/);
  });

  it("rejects disallowed port", async () => {
    const res = await wsRequest(PORT, {
      type: "print",
      printer: "192.168.1.100",
      zpl: "^XA^XZ",
      port: 9101,
    });
    assert.equal(res.ok, false);
    assert.match(res.error as string, /not in the allowed list/);
  });

  it("rejects missing zpl for print type", async () => {
    const res = await wsRequest(PORT, {
      type: "print",
      printer: "192.168.1.100",
    });
    assert.equal(res.ok, false);
    assert.equal(res.error, 'Missing required field "zpl"');
  });

  it("handles print request (connection error expected)", async () => {
    const res = await wsRequest(PORT, {
      type: "print",
      printer: "192.168.1.100",
      zpl: "^XA^XZ",
      timeout: 1000,
    });
    // No real printer, so we expect an error response — but not a crash
    assert.equal(res.ok, false);
    assert.equal(typeof res.error, "string");
  });

  it("handles status request (connection error expected)", async () => {
    const res = await wsRequest(PORT, {
      type: "status",
      printer: "192.168.1.100",
      timeout: 1000,
    });
    // No real printer, so we expect an error response — but not a crash
    assert.equal(res.ok, false);
    assert.equal(typeof res.error, "string");
  });
});

describe("createPrintProxy - mock printer forwarding", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  it("forwards HTTP /print payload to a mock TCP printer", async () => {
    const mock = await createMockTcpServer();
    const { close } = await createPrintProxy({
      port: 18920,
      allowedPrinters: ["127.0.0.1"],
      allowedPorts: [mock.port],
    });
    try {
      const zpl = "^XA^FDproxy-http^FS^XZ";
      const res = await request(18920, "POST", "/print", {
        printer: "127.0.0.1",
        port: mock.port,
        zpl,
      });
      assert.equal(res.status, 200);
      assert.equal(
        await waitFor(() => mock.receivedPayloads.some((p) => p.includes("proxy-http"))),
        true,
      );
    } finally {
      await close();
      await mock.close();
    }
  });

  it("forwards WebSocket print payload to a mock TCP printer and echoes correlation id", async () => {
    const mock = await createMockTcpServer();
    const { close } = await createPrintProxy({
      port: 18921,
      allowedPrinters: ["127.0.0.1"],
      allowedPorts: [mock.port],
    });
    try {
      const res = await wsRequest(18921, {
        id: "req-1",
        type: "print",
        printer: "127.0.0.1",
        port: mock.port,
        zpl: "^XA^FDproxy-ws^FS^XZ",
      });
      assert.equal(res.ok, true);
      assert.equal(res.id, "req-1");
      assert.equal(
        await waitFor(() => mock.receivedPayloads.some((p) => p.includes("proxy-ws"))),
        true,
      );
    } finally {
      await close();
      await mock.close();
    }
  });

  it("enforces per-client WebSocket rate limit", async () => {
    const mock = await createMockTcpServer();
    const { close } = await createPrintProxy({
      port: 18922,
      allowedPrinters: ["127.0.0.1"],
      allowedPorts: [mock.port],
      wsRateLimitPerClient: { maxRequests: 1, windowMs: 5_000 },
    });

    try {
      const ws = new WebSocket("ws://127.0.0.1:18922");
      const messages: Record<string, unknown>[] = [];
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("timed out waiting for ws")), 10_000);
        ws.once("open", () => {
          ws.send(
            JSON.stringify({
              id: 1,
              type: "print",
              printer: "127.0.0.1",
              port: mock.port,
              zpl: "^XA^FDone^FS^XZ",
            }),
          );
          ws.send(
            JSON.stringify({
              id: 2,
              type: "print",
              printer: "127.0.0.1",
              port: mock.port,
              zpl: "^XA^FDtwo^FS^XZ",
            }),
          );
        });
        ws.on("message", (data: Buffer) => {
          messages.push(JSON.parse(data.toString("utf-8")) as Record<string, unknown>);
          if (messages.length >= 2) {
            clearTimeout(timer);
            resolve();
          }
        });
        ws.on("error", (err) => {
          clearTimeout(timer);
          reject(err);
        });
      });
      const second = messages.find((m) => m.id === 2);
      assert.ok(second);
      assert.equal(second?.ok, false);
      assert.match(String(second?.error), /Rate limit exceeded/);
      ws.close();
    } finally {
      await close();
      await mock.close();
    }
  });

  it("rejects additional WebSocket connections above maxConnections", async () => {
    const port = 18923;
    const { close } = await createPrintProxy({
      port,
      allowedPrinters: ["127.0.0.1"],
      maxConnections: 1,
    });

    const ws1 = new WebSocket(`ws://127.0.0.1:${port}`);
    await new Promise<void>((resolve, reject) => {
      ws1.once("open", () => resolve());
      ws1.once("error", reject);
    });

    try {
      const secondRejected = await new Promise<boolean>((resolve) => {
        const ws2 = new WebSocket(`ws://127.0.0.1:${port}`);
        const timer = setTimeout(() => {
          resolve(false);
          ws2.terminate();
        }, 5_000);
        ws2.once("unexpected-response", (_req, res) => {
          clearTimeout(timer);
          resolve(res.statusCode === 503);
          ws2.terminate();
        });
        ws2.once("error", () => {
          clearTimeout(timer);
          resolve(true);
          ws2.terminate();
        });
        ws2.once("open", () => {
          clearTimeout(timer);
          resolve(false);
          ws2.terminate();
        });
      });
      assert.equal(secondRejected, true);
    } finally {
      ws1.terminate();
      await close();
    }
  });
});

describe("createPrintProxy - additional security coverage", { skip: !NETWORK_INTEGRATION_AVAILABLE }, () => {
  it("forwards /status to mock printer and parses ~HS response", async () => {
    const mock = await createMockTcpServer();
    const { close } = await createPrintProxy({
      port: 18924,
      allowedPrinters: ["127.0.0.1"],
      allowedPorts: [mock.port],
    });
    try {
      const res = await request(18924, "POST", "/status", {
        printer: "127.0.0.1",
        port: mock.port,
      });
      assert.equal(res.status, 200);
      assert.equal((res.body as { ready: boolean }).ready, true);
      assert.ok(mock.receivedPayloads.some((p) => p.includes("~HS")));
    } finally {
      await close();
      await mock.close();
    }
  });

  it("rejects /status when port is not allowlisted", async () => {
    const { close } = await createPrintProxy({
      port: 18925,
      allowedPrinters: ["127.0.0.1"],
      allowedPorts: [9100],
    });
    try {
      const res = await request(18925, "POST", "/status", {
        printer: "127.0.0.1",
        port: 9101,
      });
      assert.equal(res.status, 403);
      assert.match((res.body as { error: string }).error, /not in the allowed list/);
    } finally {
      await close();
    }
  });

  it("rejects oversized payloads with 413", async () => {
    const { close } = await createPrintProxy({
      port: 18926,
      allowedPrinters: ["127.0.0.1"],
      maxPayloadSize: 64,
    });
    try {
      const largeZpl = "^XA^FD" + "X".repeat(512) + "^FS^XZ";
      const res = await request(18926, "POST", "/print", {
        printer: "127.0.0.1",
        zpl: largeZpl,
      });
      assert.equal(res.status, 413);
      assert.match((res.body as { error: string }).error, /Payload exceeds maximum size/);
    } finally {
      await close();
    }
  });
});
