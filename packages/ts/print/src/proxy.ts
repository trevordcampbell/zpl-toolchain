import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import type { Server } from "node:http";
import { WebSocketServer, type WebSocket as WsType } from "ws";
import type { ProxyConfig } from "./types.js";
import { print, tcpQuery } from "./index.js";
import { parseHostStatus } from "./status.js";

export type { ProxyConfig } from "./types.js";

// ─── Defaults ────────────────────────────────────────────────────────────────

const DEFAULT_PROXY_PORT = 3001;
const DEFAULT_HOSTNAME = "127.0.0.1";
const DEFAULT_MAX_PAYLOAD = 1024 * 1024; // 1 MiB
const DEFAULT_CORS = "*";

// ─── Helpers ─────────────────────────────────────────────────────────────────

function readBody(
  req: IncomingMessage,
  maxSize: number,
  timeoutMs = 30_000
): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    let settled = false;

    const fail = (err: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(err);
    };

    const succeed = (value: string) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(value);
    };

    // Guard against slow-loris: reject if body isn't received within the deadline.
    const timer = setTimeout(() => {
      req.removeAllListeners("data");
      req.resume();
      fail(new Error("Body read timed out"));
    }, timeoutMs);

    req.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > maxSize) {
        // Don't destroy the socket — the response still needs to be written
        // (e.g., 413). Just stop collecting data and signal the error.
        req.removeAllListeners("data");
        req.resume(); // drain remaining body data
        fail(new Error(`Payload exceeds maximum size of ${maxSize} bytes`));
        return;
      }
      chunks.push(chunk);
    });

    req.on("end", () => {
      succeed(Buffer.concat(chunks).toString("utf-8"));
    });
    req.on("error", fail);
  });
}

function jsonResponse(
  res: ServerResponse,
  status: number,
  body: unknown
): void {
  const payload = JSON.stringify(body);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(payload),
  });
  res.end(payload);
}

function isPrinterAllowed(
  printer: string,
  compiledList: { pattern: string; regex?: RegExp }[]
): boolean {
  if (compiledList.length === 0) return false;

  return compiledList.some(({ pattern, regex }) => {
    if (regex) return regex.test(printer);
    return pattern === printer;
  });
}

function isPortAllowed(port: number, allowedPorts: number[]): boolean {
  return allowedPorts.includes(port);
}

function setCorsHeaders(
  res: ServerResponse,
  cors: string | string[],
  origin: string | undefined
): void {
  if (cors === "*") {
    res.setHeader("Access-Control-Allow-Origin", "*");
  } else if (Array.isArray(cors)) {
    if (origin && cors.includes(origin)) {
      res.setHeader("Access-Control-Allow-Origin", origin);
      res.setHeader("Vary", "Origin");
    }
  } else if (typeof cors === "string" && origin && cors === origin) {
    res.setHeader("Access-Control-Allow-Origin", origin);
    res.setHeader("Vary", "Origin");
  }
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type");
  res.setHeader("Access-Control-Max-Age", "86400");
}

/** Send JSON to a WebSocket only if the connection is still open. */
function wsSend(ws: WsType, data: unknown): void {
  if (ws.readyState === 1 /* OPEN */) {
    ws.send(JSON.stringify(data));
  }
}

// ─── Proxy server ────────────────────────────────────────────────────────────

/**
 * Create and start an HTTP and WebSocket print proxy server.
 *
 * The proxy accepts JSON POST requests and forwards them to printers over
 * TCP port 9100. This enables browser-based applications to print without
 * requiring the Zebra Browser Print agent.
 *
 * ## Endpoints
 *
 * ### `POST /print`
 * Send ZPL to a printer.
 * ```json
 * { "printer": "192.168.1.100", "zpl": "^XA^FDHello^FS^XZ" }
 * ```
 *
 * ### `POST /status`
 * Query a printer's status.
 * ```json
 * { "printer": "192.168.1.100" }
 * ```
 *
 * ### `GET /health`
 * Returns `{ "ok": true }`.
 *
 * ### WebSocket (`ws://host:port`)
 * Accepts JSON messages with a `type` field (`"print"` or `"status"`).
 * ```json
 * { "type": "print", "printer": "192.168.1.100", "zpl": "^XA^FDHello^FS^XZ" }
 * ```
 * Responses are JSON with an `ok` field:
 * ```json
 * { "ok": true, "success": true, "bytesWritten": 42, "duration": 12 }
 * ```
 *
 * @example
 * ```ts
 * import { createPrintProxy } from "@zpl-toolchain/print/proxy";
 *
 * const { server, close } = await createPrintProxy({
 *   port: 3001,
 *   allowedPrinters: ["192.168.1.100", "192.168.1.101"],
 * });
 *
 * console.log("Proxy listening on port 3001");
 *
 * // Shut down later
 * await close();
 * ```
 */
export async function createPrintProxy(
  config: ProxyConfig = {}
): Promise<{ server: Server; close: () => Promise<void> }> {
  const port = config.port ?? DEFAULT_PROXY_PORT;
  const hostname = config.hostname ?? DEFAULT_HOSTNAME;
  const maxPayload = config.maxPayloadSize ?? DEFAULT_MAX_PAYLOAD;
  const cors = config.cors ?? DEFAULT_CORS;
  const allowed = config.allowedPrinters;
  const allowedPorts = config.allowedPorts ?? [9100];
  const maxConnections = config.maxConnections ?? 50;
  const wsRateLimit = (() => {
    const rl = config.wsRateLimitPerClient;
    if (!rl) return undefined;
    if (!Number.isInteger(rl.maxRequests) || rl.maxRequests <= 0) return undefined;
    if (!Number.isInteger(rl.windowMs) || rl.windowMs <= 0) return undefined;
    return rl;
  })();

  // Pre-compile glob patterns into regexes once at startup.
  const compiledAllowlist: { pattern: string; regex?: RegExp }[] = (allowed ?? []).map((pattern) => {
    if (!pattern.includes("*")) return { pattern };
    // Bare "*" is a special case meaning "match all" (including dotted IPs/hostnames).
    if (pattern === "*") return { pattern, regex: /^.*$/ };
    const escaped = pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
    const regex = new RegExp("^" + escaped.replace(/\*/g, "[^.]*") + "$");
    return { pattern, regex };
  });

  const server = createServer(async (req, res) => {
    const origin = req.headers.origin;
    setCorsHeaders(res, cors, origin);

    // Handle CORS preflight.
    if (req.method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    // ── GET /health ────────────────────────────────────────────────────
    if (req.method === "GET" && req.url === "/health") {
      jsonResponse(res, 200, { ok: true });
      return;
    }

    // Only POST from here on.
    if (req.method !== "POST") {
      jsonResponse(res, 405, { error: "Method not allowed" });
      return;
    }

    // Fail fast on unknown routes before reading body.
    if (req.url !== "/print" && req.url !== "/status") {
      jsonResponse(res, 404, { error: "Not found" });
      return;
    }

    try {
      const contentType = req.headers["content-type"] ?? "";
      if (!contentType.toLowerCase().includes("application/json")) {
        jsonResponse(res, 415, {
          error: "Content-Type must be application/json",
        });
        return;
      }

      const body = await readBody(req, maxPayload);
      let parsed: Record<string, unknown>;
      try {
        parsed = JSON.parse(body);
      } catch {
        jsonResponse(res, 400, { error: "Invalid JSON" });
        return;
      }

      // ── POST /print ──────────────────────────────────────────────────
      if (req.url === "/print") {
        const printer = parsed.printer;
        const zpl = parsed.zpl;

        if (typeof printer !== "string" || !printer) {
          jsonResponse(res, 400, { error: 'Missing required field "printer"' });
          return;
        }
        if (typeof zpl !== "string" || !zpl) {
          jsonResponse(res, 400, { error: 'Missing required field "zpl"' });
          return;
        }

        if (!isPrinterAllowed(printer, compiledAllowlist)) {
          jsonResponse(res, 403, {
            error: `Printer "${printer}" is not in the allowed list`,
          });
          return;
        }

        const reqPort = typeof parsed.port === "number" ? parsed.port : 9100;
        if (!Number.isInteger(reqPort) || reqPort < 1 || reqPort > 65535) {
          jsonResponse(res, 400, { error: "Port must be an integer between 1 and 65535" });
          return;
        }
        if (!isPortAllowed(reqPort, allowedPorts)) {
          jsonResponse(res, 403, { error: `Port ${reqPort} is not in the allowed list` });
          return;
        }

        const result = await print(zpl, {
          host: printer,
          port: reqPort,
          timeout: typeof parsed.timeout === "number" ? Math.max(1000, Math.min(parsed.timeout, 60000)) : 5000,
        });

        jsonResponse(res, 200, result);
        return;
      }

      // ── POST /status ─────────────────────────────────────────────────
      if (req.url === "/status") {
        const printer = parsed.printer;

        if (typeof printer !== "string" || !printer) {
          jsonResponse(res, 400, { error: 'Missing required field "printer"' });
          return;
        }

        if (!isPrinterAllowed(printer, compiledAllowlist)) {
          jsonResponse(res, 403, {
            error: `Printer "${printer}" is not in the allowed list`,
          });
          return;
        }

        const printerPort =
          typeof parsed.port === "number" ? parsed.port : 9100;
        if (!Number.isInteger(printerPort) || printerPort < 1 || printerPort > 65535) {
          jsonResponse(res, 400, { error: "Port must be an integer between 1 and 65535" });
          return;
        }
        if (!isPortAllowed(printerPort, allowedPorts)) {
          jsonResponse(res, 403, { error: `Port ${printerPort} is not in the allowed list` });
          return;
        }
        const timeout =
          typeof parsed.timeout === "number" ? Math.max(1000, Math.min(parsed.timeout, 60000)) : 5000;

        const raw = await tcpQuery(printer, printerPort, "~HS", timeout);
        const status = parseHostStatus(raw);

        jsonResponse(res, 200, status);
        return;
      }

    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      const isPayloadError = message.includes("Payload exceeds maximum size");
      const isBodyTimeout = message.includes("Body read timed out");
      const status = isPayloadError ? 413 : isBodyTimeout ? 408 : 502;
      // Sanitize: don't leak internal IPs/ports from TCP errors to clients.
      const clientMessage = isPayloadError || isBodyTimeout
        ? message
        : "Failed to communicate with printer";
      jsonResponse(res, status, { error: clientMessage });
    }
  });

  // ── WebSocket upgrade endpoint ──────────────────────────────────────────
  const wss = new WebSocketServer({
    server,
    maxPayload,
    verifyClient: (
      info: { origin: string; secure: boolean; req: IncomingMessage },
      cb: (res: boolean, code?: number, message?: string) => void,
    ) => {
      if (wss.clients.size >= maxConnections) {
        cb(false, 503, "Too many connections");
        return;
      }
      const originAllowed =
        cors === "*" ||
        (Array.isArray(cors) ? cors.includes(info.origin) : cors === info.origin);
      if (!originAllowed) {
        cb(false, 403, "Origin not allowed");
        return;
      }
      cb(true);
    },
  });

  wss.on("connection", (ws: WsType) => {
    let alive = true;
    const requestTimestamps: number[] = [];
    ws.on("pong", () => { alive = true; });

    const heartbeat = setInterval(() => {
      if (!alive) {
        ws.terminate();
        return;
      }
      alive = false;
      ws.ping();
    }, 30_000);

    ws.on("close", () => clearInterval(heartbeat));

    ws.on("error", () => {
      // Swallow per-connection errors to prevent process crash.
      // The connection will be cleaned up by the "close" event.
    });

    ws.on("message", (raw: Buffer | string) => {
      void (async () => {
        let parsed: Record<string, unknown>;
        try {
          const text = typeof raw === "string" ? raw : raw.toString("utf-8");
          parsed = JSON.parse(text) as Record<string, unknown>;
        } catch {
          wsSend(ws, { ok: false, error: "Invalid JSON" });
          return;
        }

        const correlationId = parsed.id;
        const withId = <T extends Record<string, unknown>>(obj: T): T & { id?: string | number } => {
          if (typeof correlationId === "string" || typeof correlationId === "number") {
            return { ...obj, id: correlationId };
          }
          return obj;
        };
        if (
          correlationId !== undefined &&
          typeof correlationId !== "string" &&
          typeof correlationId !== "number"
        ) {
          wsSend(ws, withId({ ok: false, error: '"id" must be a string or number when provided' }));
          return;
        }

        if (wsRateLimit) {
          const now = Date.now();
          while (requestTimestamps.length > 0 && now - requestTimestamps[0] >= wsRateLimit.windowMs) {
            requestTimestamps.shift();
          }
          if (requestTimestamps.length >= wsRateLimit.maxRequests) {
            wsSend(
              ws,
              withId({
                ok: false,
                error: `Rate limit exceeded: max ${wsRateLimit.maxRequests} requests per ${wsRateLimit.windowMs}ms`,
              }),
            );
            return;
          }
          requestTimestamps.push(now);
        }

        const msgType = parsed.type;
        if (typeof msgType !== "string" || !["print", "status"].includes(msgType)) {
          wsSend(ws, withId({ ok: false, error: 'Missing or invalid "type" field (expected "print" or "status")' }));
          return;
        }

        const printer = parsed.printer;
        if (typeof printer !== "string" || !printer) {
          wsSend(ws, withId({ ok: false, error: 'Missing required field "printer"' }));
          return;
        }

        if (!isPrinterAllowed(printer, compiledAllowlist)) {
          wsSend(ws, withId({ ok: false, error: `Printer "${printer}" is not in the allowed list` }));
          return;
        }

        const reqPort = typeof parsed.port === "number" ? parsed.port : 9100;
        if (!Number.isInteger(reqPort) || reqPort < 1 || reqPort > 65535) {
          wsSend(ws, withId({ ok: false, error: "Port must be an integer between 1 and 65535" }));
          return;
        }
        if (!isPortAllowed(reqPort, allowedPorts)) {
          wsSend(ws, withId({ ok: false, error: `Port ${reqPort} is not in the allowed list` }));
          return;
        }

        const timeout = typeof parsed.timeout === "number"
          ? Math.max(1000, Math.min(parsed.timeout, 60000))
          : 5000;

        try {
          if (msgType === "print") {
            const zpl = parsed.zpl;
            if (typeof zpl !== "string" || !zpl) {
              wsSend(ws, withId({ ok: false, error: 'Missing required field "zpl"' }));
              return;
            }
            const result = await print(zpl, { host: printer, port: reqPort, timeout });
            wsSend(ws, withId({ ok: true, ...result }));
          } else {
            // status
            const rawResp = await tcpQuery(printer, reqPort, "~HS", timeout);
            const status = parseHostStatus(rawResp);
            wsSend(ws, withId({ ok: true, ...status }));
          }
        } catch (err: unknown) {
          // Sanitize: don't leak internal IPs/ports from TCP errors.
          wsSend(ws, withId({ ok: false, error: "Failed to communicate with printer" }));
        }
      })().catch((err) => {
        // Shouldn't happen — inner handler has its own try/catch.
        if (typeof console !== "undefined") console.error("[print-proxy] Unhandled error:", err);
      });
    });
  });

  // Cap total connections (HTTP + WebSocket combined) at the OS level.
  server.maxConnections = maxConnections;

  return new Promise((resolve, reject) => {
    server.on("error", reject);
    server.listen(port, hostname, () => {
      server.removeListener("error", reject);
      resolve({
        server,
        close: () =>
          new Promise<void>((res, rej) => {
            // Close all WebSocket connections first
            for (const client of wss.clients) {
              client.terminate();
            }
            wss.close(() => {
              server.close((err) => (err ? rej(err) : res()));
            });
          }),
      });
    });
  });
}
