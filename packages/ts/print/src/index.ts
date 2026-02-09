import net from "node:net";
import type {
  PrinterConfig,
  PrintResult,
  PrinterStatus,
  PrintErrorCode,
  ValidateOptions,
  BatchOptions,
  BatchProgress,
  BatchResult,
} from "./types.js";
import { PrintError } from "./types.js";
import { parseHostStatus } from "./status.js";

export { parseHostStatus } from "./status.js";
export {
  PrintError,
  type PrinterConfig,
  type PrintResult,
  type PrinterStatus,
  type PrintErrorCode,
  type ValidateOptions,
  type BatchOptions,
  type BatchProgress,
  type BatchResult,
} from "./types.js";

// ─── Defaults ────────────────────────────────────────────────────────────────

const DEFAULT_PORT = 9100;
const DEFAULT_TIMEOUT = 5_000;
const DEFAULT_MAX_RETRIES = 2;
const DEFAULT_RETRY_DELAY = 500;

// ─── Error classification ────────────────────────────────────────────────────

function classifyError(err: NodeJS.ErrnoException): PrintErrorCode {
  switch (err.code) {
    case "ECONNREFUSED":
      return "CONNECTION_REFUSED";
    case "ETIMEDOUT":
      return "TIMEOUT";
    case "ENOTFOUND":
      return "HOST_NOT_FOUND";
    case "EPIPE":
      return "BROKEN_PIPE";
    case "ECONNRESET":
      return "CONNECTION_RESET";
    default:
      return "UNKNOWN";
  }
}

function wrapError(err: unknown): PrintError {
  if (err instanceof PrintError) return err;
  const nodeErr = err as NodeJS.ErrnoException;
  const code = classifyError(nodeErr);
  const msg = nodeErr.message ?? String(err);
  return new PrintError(msg, code, err);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function resolveConfig(cfg: PrinterConfig) {
  if (!cfg.host || cfg.host.trim().length === 0) {
    throw new PrintError("host is required", "UNKNOWN");
  }
  const port = cfg.port ?? DEFAULT_PORT;
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new PrintError(`Invalid port: ${port} (must be 1–65535)`, "UNKNOWN");
  }
  const timeout = cfg.timeout ?? DEFAULT_TIMEOUT;
  if (typeof timeout !== "number" || timeout < 0) {
    throw new PrintError(`Invalid timeout: ${timeout}`, "UNKNOWN");
  }
  return {
    host: cfg.host,
    port,
    timeout,
    maxRetries: cfg.maxRetries ?? DEFAULT_MAX_RETRIES,
    retryDelay: cfg.retryDelay ?? DEFAULT_RETRY_DELAY,
  };
}

/** Sleep for `ms` milliseconds. */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ─── Low-level TCP helpers ───────────────────────────────────────────────────

/**
 * Open a TCP socket, write `data`, and close. Returns bytes written & timing.
 */
function tcpSend(
  host: string,
  port: number,
  data: string,
  timeout: number
): Promise<PrintResult> {
  return new Promise<PrintResult>((resolve, reject) => {
    const start = performance.now();
    const buf = Buffer.from(data, "utf-8");
    let settled = false;

    const fail = (err: PrintError) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      reject(err);
    };

    const succeed = (result: PrintResult) => {
      if (settled) return;
      settled = true;
      resolve(result);
    };

    const socket = net.createConnection({ host, port }, () => {
      socket.setNoDelay(true);
      socket.write(buf, (writeErr) => {
        if (writeErr) {
          fail(wrapError(writeErr));
          return;
        }
        socket.end(() => {
          succeed({
            success: true,
            bytesWritten: buf.length,
            duration: Math.round(performance.now() - start),
          });
          // Force-close after graceful FIN — ZPL printers won't send FIN back,
          // which would leave the socket in FIN_WAIT_2 indefinitely.
          setTimeout(() => {
            if (!socket.destroyed) socket.destroy();
          }, 1000).unref();
        });
      });
    });

    socket.setTimeout(timeout);
    socket.on("timeout", () => {
      fail(
        new PrintError(
          `Connection to ${host}:${port} timed out after ${timeout}ms`,
          "TIMEOUT"
        )
      );
    });
    socket.on("error", (err) => fail(wrapError(err)));
  });
}

/**
 * Open a TCP socket, write `command`, read the response until the socket is
 * idle for a short period, then close and return the response.
 *
 * Exported so that `proxy.ts` can re-use it instead of duplicating the logic.
 */
export function tcpQuery(
  host: string,
  port: number,
  command: string,
  timeout: number
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const chunks: Uint8Array[] = [];
    let idleTimer: ReturnType<typeof setTimeout> | undefined;
    let settled = false;

    const finish = (value: string) => {
      if (settled) return;
      settled = true;
      if (idleTimer) clearTimeout(idleTimer);
      socket.destroy();
      resolve(value);
    };

    const fail = (err: PrintError) => {
      if (settled) return;
      settled = true;
      if (idleTimer) clearTimeout(idleTimer);
      socket.destroy();
      reject(err);
    };

    const socket = net.createConnection({ host, port }, () => {
      socket.write(command, "utf-8");
    });

    socket.setTimeout(timeout);

    socket.on("data", (chunk: Buffer) => {
      chunks.push(chunk);
      // Reset idle timer — wait 250ms after last data before closing.
      if (idleTimer) clearTimeout(idleTimer);
      idleTimer = setTimeout(() => {
        socket.end();
      }, 250);
    });

    socket.on("end", () => {
      finish(Buffer.concat(chunks).toString("utf-8"));
    });

    socket.on("timeout", () => {
      // If we have partial data, return it rather than failing.
      if (chunks.length > 0) {
        finish(Buffer.concat(chunks).toString("utf-8"));
      } else {
        fail(
          new PrintError(
            `Query to ${host}:${port} timed out after ${timeout}ms`,
            "TIMEOUT"
          )
        );
      }
    });

    socket.on("error", (err) => fail(wrapError(err)));
  });
}

// ─── One-shot print function ─────────────────────────────────────────────────

/**
 * Send ZPL to a printer and return the result.
 *
 * Automatically retries on transient network errors with exponential backoff.
 *
 * @example
 * ```ts
 * import { print } from "@zpl-toolchain/print";
 *
 * const result = await print("^XA^FO50,50^A0N,50,50^FDHello^FS^XZ", {
 *   host: "192.168.1.100",
 * });
 * console.log(result); // { success: true, bytesWritten: 44, duration: 12 }
 * ```
 */
export async function print(
  zpl: string,
  config: PrinterConfig
): Promise<PrintResult> {
  const cfg = resolveConfig(config);
  let lastError: PrintError | undefined;

  for (let attempt = 0; attempt <= cfg.maxRetries; attempt++) {
    try {
      return await tcpSend(cfg.host, cfg.port, zpl, cfg.timeout);
    } catch (err) {
      lastError = wrapError(err);

      // Only retry on transient errors.
      const retryable: PrintErrorCode[] = [
        "TIMEOUT",
        "CONNECTION_RESET",
        "BROKEN_PIPE",
      ];
      if (!retryable.includes(lastError.code)) throw lastError;
      if (attempt < cfg.maxRetries) {
        const delay = cfg.retryDelay * Math.pow(2, attempt);
        await sleep(delay);
      }
    }
  }

  throw lastError!;
}

// ─── Persistent connection (TcpPrinter) ─────────────────────────────────────

/**
 * A persistent TCP connection to a ZPL printer.
 *
 * @example
 * ```ts
 * import { createPrinter } from "@zpl-toolchain/print";
 *
 * const printer = createPrinter({ host: "192.168.1.100" });
 * await printer.print("^XA^FO50,50^A0N,50,50^FDHello^FS^XZ");
 * const status = await printer.getStatus();
 * console.log(status.ready);
 * await printer.close();
 * ```
 */
export class TcpPrinter {
  private readonly config: ReturnType<typeof resolveConfig>;
  private socket: net.Socket | null = null;
  private connecting: Promise<void> | null = null;
  private closed = false;
  private writeQueue: Promise<void> = Promise.resolve();

  constructor(config: PrinterConfig) {
    this.config = resolveConfig(config);
  }

  // ── Internal helpers ───────────────────────────────────────────────────

  /** Ensure a live connection exists, creating one if needed. */
  private async ensureConnected(): Promise<net.Socket> {
    if (this.closed) {
      throw new PrintError("Printer connection has been closed", "UNKNOWN");
    }
    if (this.socket && !this.socket.destroyed) {
      return this.socket;
    }

    if (this.connecting) {
      await this.connecting;
      return this.socket!;
    }

    this.connecting = new Promise<void>((resolve, reject) => {
      const sock = net.createConnection(
        { host: this.config.host, port: this.config.port },
        () => {
          sock.setNoDelay(true);
          sock.setKeepAlive(true, 60_000);
          // Disable idle timeout — it was only needed for the connection phase.
          // TCP keepalive (above) handles liveness detection for persistent connections.
          sock.setTimeout(0);
          sock.removeAllListeners("timeout");
          sock.removeAllListeners("error");
          sock.on("error", () => { this.socket = null; });
          sock.on("close", () => { this.socket = null; });
          this.socket = sock;
          this.connecting = null;
          resolve();
        }
      );
      sock.setTimeout(this.config.timeout);
      sock.on("timeout", () => {
        sock.destroy();
        this.socket = null;
        this.connecting = null;
        reject(
          new PrintError(
            `Connection to ${this.config.host}:${this.config.port} timed out`,
            "TIMEOUT"
          )
        );
      });
      sock.on("error", (err) => {
        this.socket = null;
        this.connecting = null;
        reject(wrapError(err));
      });
    });

    await this.connecting;
    return this.socket!;
  }

  // ── Public API ─────────────────────────────────────────────────────────

  /**
   * Send ZPL to the printer over the persistent connection.
   *
   * Falls back to a fresh connection if the existing one is broken.
   */
  print(zpl: string): Promise<PrintResult> {
    const job = this.writeQueue.then(async () => {
      const start = performance.now();
      const buf = Buffer.from(zpl, "utf-8");

      const sock = await this.ensureConnected();
      return new Promise<PrintResult>((resolve, reject) => {
        sock.write(buf, (err) => {
          if (err) {
            this.socket = null;
            reject(wrapError(err));
            return;
          }
          resolve({
            success: true,
            bytesWritten: buf.length,
            duration: Math.round(performance.now() - start),
          });
        });
      });
    });
    // Chain onto writeQueue so next print waits for this one.
    // Eat errors in the queue chain — callers get their own rejection.
    this.writeQueue = job.then(
      () => {},
      () => {}
    );
    return job;
  }

  /**
   * Query the printer's host status (~HS).
   *
   * Sends the `~HS` command and parses the response into a
   * {@link PrinterStatus} object.
   *
   * **Note:** This opens a separate short-lived TCP connection for the query
   * rather than using the persistent connection, because the ~HS response
   * needs an idle-timeout-based read strategy that would interfere with the
   * persistent socket's event handlers.
   */
  async getStatus(): Promise<PrinterStatus> {
    const raw = await tcpQuery(
      this.config.host,
      this.config.port,
      "~HS",
      this.config.timeout
    );
    return parseHostStatus(raw);
  }

  /**
   * Send an arbitrary command and return the raw response string.
   *
   * Useful for querying ~HI (Host Identification), ~HM (Host Memory), etc.
   *
   * **Note:** This opens a separate short-lived TCP connection for each query
   * rather than using the persistent connection, because reading a response
   * requires an idle-timeout strategy that would interfere with the
   * persistent socket's event handlers.
   */
  async query(command: string): Promise<string> {
    return tcpQuery(
      this.config.host,
      this.config.port,
      command,
      this.config.timeout
    );
  }

  /**
   * Check whether the printer is reachable by attempting a TCP connection.
   * Returns `true` if the connection succeeds, `false` otherwise.
   */
  async isReachable(): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      let settled = false;
      const done = (value: boolean) => {
        if (settled) return;
        settled = true;
        resolve(value);
      };
      const sock = net.createConnection(
        { host: this.config.host, port: this.config.port },
        () => {
          sock.end();
          // Force-close after graceful FIN — printers won't send FIN back.
          setTimeout(() => { if (!sock.destroyed) sock.destroy(); }, 1000).unref();
          done(true);
        }
      );
      sock.setTimeout(this.config.timeout);
      sock.on("timeout", () => {
        sock.destroy();
        done(false);
      });
      sock.on("error", () => {
        sock.destroy();
        done(false);
      });
    });
  }

  /**
   * Send multiple labels sequentially over the persistent connection.
   *
   * @param labels - Array of ZPL strings, one per label.
   * @param opts - Optional batch options (e.g., status polling interval).
   * @param onProgress - Optional callback after each label.
   *   Return `false` (strictly) to abort the batch early.
   *   Any other return value (including `undefined`/`true`) continues normally.
   * @returns The number of labels sent and the total.
   *
   * @example
   * ```ts
   * // Simple batch
   * await printer.printBatch(labels);
   *
   * // With progress tracking
   * await printer.printBatch(labels, {}, (p) => {
   *   console.log(`${p.sent}/${p.total}`);
   * });
   *
   * // With status polling every 5 labels
   * await printer.printBatch(labels, { statusInterval: 5 }, (p) => {
   *   if (p.status) console.log(`Labels remaining: ${p.status.labelsRemaining}`);
   * });
   * ```
   */
  async printBatch(
    labels: string[],
    opts?: BatchOptions,
    onProgress?: (progress: BatchProgress) => boolean | void
  ): Promise<BatchResult> {
    const total = labels.length;
    const interval = opts?.statusInterval ?? 0;
    let sent = 0;

    for (const label of labels) {
      await this.print(label);
      sent++;

      let status: PrinterStatus | undefined;
      if (interval > 0 && sent % interval === 0) {
        try {
          status = await this.getStatus();
        } catch {
          // Status polling is best-effort — don't abort the batch on query failure.
        }
      }

      if (onProgress) {
        const shouldContinue = onProgress({ sent, total, status });
        if (shouldContinue === false) break;
      }
    }

    return { sent, total };
  }

  /**
   * Poll the printer until all queued labels have been printed.
   *
   * Queries `~HS` at the specified interval and resolves when
   * `formatsInBuffer === 0` and `labelsRemaining === 0`.
   *
   * @param pollInterval - Milliseconds between status polls (default: 500).
   * @param timeout - Maximum wait time in milliseconds (default: 30000).
   * @throws {PrintError} with code `TIMEOUT` if the timeout is exceeded.
   */
  async waitForCompletion(
    pollInterval = 500,
    timeout = 30_000
  ): Promise<void> {
    const start = performance.now();
    const deadline = start + timeout;

    while (true) {
      if (performance.now() >= deadline) {
        throw new PrintError(
          `Printer did not finish within ${timeout}ms`,
          "TIMEOUT"
        );
      }

      try {
        const status = await this.getStatus();
        if (status.formatsInBuffer === 0 && status.labelsRemaining === 0) {
          return;
        }
      } catch {
        // Transient status query failure — retry until deadline.
      }

      const remaining = deadline - performance.now();
      if (remaining <= 0) {
        throw new PrintError(
          `Printer did not finish within ${timeout}ms`,
          "TIMEOUT"
        );
      }
      await new Promise((resolve) =>
        setTimeout(resolve, Math.min(pollInterval, remaining))
      );
    }
  }

  /** Close the persistent connection. Safe to call multiple times. */
  async close(): Promise<void> {
    this.closed = true;

    // Drain the write queue — let in-flight writes settle before teardown.
    try {
      await this.writeQueue;
    } catch {
      // Errors are already consumed in the queue chain.
    }

    // Wait for any in-flight connection attempt to settle first.
    if (this.connecting) {
      try {
        await this.connecting;
      } catch {
        // Connection failed — nothing to close.
      }
    }
    const sock = this.socket;
    this.socket = null;
    this.connecting = null;
    if (!sock || sock.destroyed) return;
    return new Promise<void>((resolve) => {
      const forceTimer = setTimeout(() => {
        if (!sock.destroyed) sock.destroy();
        resolve();
      }, 2000);
      forceTimer.unref();
      sock.end(() => {
        clearTimeout(forceTimer);
        resolve();
      });
    });
  }
}

/**
 * Create a {@link TcpPrinter} with a persistent TCP connection.
 *
 * @see {@link TcpPrinter}
 */
export function createPrinter(config: PrinterConfig): TcpPrinter {
  return new TcpPrinter(config);
}

// ─── Validation helper ──────────────────────────────────────────────────────

/** @internal */
function isDiagnosticError(d: unknown): boolean {
  return (
    typeof d === "object" &&
    d !== null &&
    (d as Record<string, unknown>).severity === "error"
  );
}

/** @internal */
function isDiagnosticWarning(d: unknown): boolean {
  return (
    typeof d === "object" &&
    d !== null &&
    (d as Record<string, unknown>).severity === "warn"
  );
}

/**
 * Process the result of a `validate()` call and throw a {@link PrintError} if
 * validation fails.
 *
 * @remarks
 * Exported for unit-testing purposes. Not part of the public API contract.
 *
 * @param result - Raw return value from `@zpl-toolchain/core`'s `validate()`.
 * @param opts   - Validation options (strict mode, etc.).
 * @throws {PrintError} with code `VALIDATION_FAILED` when errors (or warnings
 *   in strict mode) are found.
 *
 * @internal
 */
export function _processValidationResult(
  result: unknown,
  opts?: ValidateOptions
): void {
  // The core validate() returns { ok, issues } — extract issues array.
  let diagnostics: unknown[];
  if (
    typeof result === "object" &&
    result !== null &&
    Array.isArray((result as Record<string, unknown>).issues)
  ) {
    diagnostics = (result as Record<string, unknown>).issues as unknown[];
  } else if (Array.isArray(result)) {
    diagnostics = result;
  } else {
    diagnostics = [];
  }

  const errors = diagnostics.filter(isDiagnosticError);
  const warnings = diagnostics.filter(isDiagnosticWarning);

  // In strict mode, warnings are also treated as errors.
  const failures = opts?.strict
    ? [...errors, ...warnings]
    : errors;

  if (failures.length > 0) {
    const msgs = failures
      .map((e: unknown) => {
        if (typeof e === "object" && e !== null) {
          const msg = (e as Record<string, unknown>).message;
          return typeof msg === "string" && msg.length > 0 ? msg : "unknown error";
        }
        return "unknown error";
      })
      .join("; ");
    const label = opts?.strict && errors.length === 0
      ? "ZPL validation warnings (strict mode)"
      : "ZPL validation failed";
    throw new PrintError(`${label}: ${msgs}`, "VALIDATION_FAILED");
  }
}

// ─── printValidated — convenience with optional @zpl-toolchain/core ──────────

/**
 * Print ZPL after first validating it with `@zpl-toolchain/core` (if
 * installed). Falls back to a plain print if the core package is not
 * available.
 *
 * Supports optional printer-profile-aware validation and strict mode
 * (treat warnings as errors).
 *
 * @param zpl - The ZPL source string to validate and send.
 * @param config - Printer connection configuration.
 * @param validateOpts - Optional validation settings (profile, strict mode).
 *
 * @throws {PrintError} with code `VALIDATION_FAILED` if validation finds errors
 *   (or warnings in strict mode).
 * @throws {PrintError} with a network-related code if the print operation fails.
 *
 * @example
 * ```ts
 * import { printValidated } from "@zpl-toolchain/print";
 * import { readFileSync } from "node:fs";
 *
 * // Basic: validate then print
 * await printValidated("^XA^FDHello^FS^XZ", { host: "192.168.1.100" });
 *
 * // With printer profile for hardware-aware validation
 * const profile = readFileSync("profiles/ZD421.json", "utf-8");
 * await printValidated("^XA^FDHello^FS^XZ", { host: "192.168.1.100" }, {
 *   profileJson: profile,
 *   strict: true,  // treat warnings as errors
 * });
 * ```
 */
export async function printValidated(
  zpl: string,
  config: PrinterConfig,
  validateOpts?: ValidateOptions
): Promise<PrintResult> {
  // Step 1: Try to load the optional peer dependency.
  let core: Record<string, unknown> | undefined;
  try {
    // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
    core = await (Function(
      'return import("@zpl-toolchain/core")'
    )() as Promise<Record<string, unknown>>);
  } catch {
    // Module not installed — skip validation silently.
  }

  // Step 2: If loaded, validate (errors propagate naturally).
  if (core) {
    const validate = core.validate;
    if (typeof validate === "function") {
      const result: unknown = await validate(zpl, validateOpts?.profileJson);
      _processValidationResult(result, validateOpts);
    }
  }

  return print(zpl, config);
}

// ─── Standalone batch print ──────────────────────────────────────────────────

/**
 * Send multiple ZPL labels to a printer in a single session.
 *
 * Opens a persistent connection, sends all labels sequentially, and closes.
 *
 * @param labels - Array of ZPL strings, one per label.
 * @param config - Printer connection configuration.
 * @param opts - Optional batch options (e.g., status polling interval).
 * @param onProgress - Optional callback after each label.
 *   Return `false` (strictly) to abort the batch early.
 * @returns The number of labels sent and the total.
 *
 * @example
 * ```ts
 * import { printBatch } from "@zpl-toolchain/print";
 *
 * const labels = [
 *   "^XA^FO50,50^A0N,50,50^FDLabel 1^FS^XZ",
 *   "^XA^FO50,50^A0N,50,50^FDLabel 2^FS^XZ",
 * ];
 * const result = await printBatch(labels, { host: "192.168.1.100" });
 * console.log(`Sent ${result.sent}/${result.total} labels`);
 * ```
 */
export async function printBatch(
  labels: string[],
  config: PrinterConfig,
  opts?: BatchOptions,
  onProgress?: (progress: BatchProgress) => boolean | void
): Promise<BatchResult> {
  const printer = new TcpPrinter(config);
  try {
    return await printer.printBatch(labels, opts, onProgress);
  } finally {
    await printer.close();
  }
}
