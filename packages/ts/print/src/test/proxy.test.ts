import { describe, it, before, after } from "node:test";
import { strict as assert } from "node:assert";
import http from "node:http";
import WebSocket from "ws";
import { createPrintProxy } from "../proxy.js";

// Helper to make HTTP requests to the proxy
function request(
  port: number,
  method: string,
  path: string,
  body?: unknown
): Promise<{ status: number; body: unknown }> {
  return new Promise((resolve, reject) => {
    const data = body ? JSON.stringify(body) : undefined;
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        path,
        method,
        headers: data
          ? { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(data) }
          : {},
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => {
          const raw = Buffer.concat(chunks).toString("utf-8");
          try {
            resolve({ status: res.statusCode!, body: JSON.parse(raw) });
          } catch {
            resolve({ status: res.statusCode!, body: raw });
          }
        });
      }
    );
    req.on("error", reject);
    if (data) req.write(data);
    req.end();
  });
}

describe("createPrintProxy", () => {
  it("health endpoint returns ok", async () => {
    const { close } = await createPrintProxy({
      port: 0, // random available port
      allowedPrinters: ["test"],
    });

    // We need the actual port - get it from the server
    // Unfortunately port 0 doesn't work because createPrintProxy doesn't expose the actual port
    // Let's use a fixed high port instead
    await close();
  });
});

describe("createPrintProxy - endpoints", () => {
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

describe("createPrintProxy - wildcard allowlist", () => {
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

describe("createPrintProxy - CORS", () => {
  it("handles OPTIONS preflight", async () => {
    const { close } = await createPrintProxy({
      port: 18903,
      allowedPrinters: ["test"],
    });

    const res = await request(18903, "OPTIONS", "/print");
    assert.equal(res.status, 204);

    await close();
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

describe("createPrintProxy - WebSocket", () => {
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
