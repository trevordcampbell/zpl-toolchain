import type { PrinterDevice, PrinterStatus } from "./types.js";
import { parseHostStatus } from "./status.js";

export type { PrinterDevice, PrinterStatus } from "./types.js";

// ─── Zebra Browser Print Agent ───────────────────────────────────────────────
//
// The Zebra Browser Print SDK provides a localhost HTTP agent that bridges
// the browser to USB / network-attached Zebra printers. By default it listens
// on http://127.0.0.1:9100 (for HTTP) and https://localhost:9101 (for HTTPS).
//
// This module wraps that agent using plain `fetch()` — zero dependencies.
// It is designed to run *only* in browser environments.
// ─────────────────────────────────────────────────────────────────────────────

/** Default HTTP endpoint for the Zebra Browser Print agent. */
const DEFAULT_AGENT_HTTP = "http://127.0.0.1:9100";

/** Default HTTPS endpoint for the Zebra Browser Print agent. */
const DEFAULT_AGENT_HTTPS = "https://localhost:9101";

/** Options for configuring the Zebra Browser Print client. */
export interface ZebraBrowserPrintOptions {
  /**
   * Base URL of the Zebra Browser Print agent.
   * Defaults to `http://127.0.0.1:9100` when served over HTTP, or
   * `https://localhost:9101` when served over HTTPS.
   */
  agentUrl?: string;

  /** Request timeout in milliseconds (default: 5000). */
  timeout?: number;
}

function resolveAgentUrl(options?: ZebraBrowserPrintOptions): string {
  if (options?.agentUrl) return options.agentUrl;

  // Auto-detect protocol if running in a browser context.
  if (typeof globalThis.location !== "undefined") {
    return globalThis.location.protocol === "https:"
      ? DEFAULT_AGENT_HTTPS
      : DEFAULT_AGENT_HTTP;
  }
  return DEFAULT_AGENT_HTTP;
}

function buildAbortSignal(timeout: number): AbortSignal {
  return AbortSignal.timeout(timeout);
}

// ─── ZebraBrowserPrint API ───────────────────────────────────────────────────

/**
 * Wrapper around the Zebra Browser Print local agent.
 *
 * Uses only `fetch()` — no external dependencies.
 *
 * @example
 * ```ts
 * import { ZebraBrowserPrint } from "@zpl-toolchain/print/browser";
 *
 * const zbp = new ZebraBrowserPrint();
 *
 * if (await zbp.isAvailable()) {
 *   const devices = await zbp.discover();
 *   if (devices.length > 0) {
 *     await zbp.print(devices[0], "^XA^FDHello^FS^XZ");
 *   }
 * }
 * ```
 */
export class ZebraBrowserPrint {
  private readonly agentUrl: string;
  private readonly timeout: number;

  constructor(options?: ZebraBrowserPrintOptions) {
    this.agentUrl = resolveAgentUrl(options);
    this.timeout = options?.timeout ?? 5_000;
  }

  // ── Availability ─────────────────────────────────────────────────────

  /**
   * Check whether the Zebra Browser Print agent is running and reachable.
   *
   * @returns `true` if the agent responds, `false` otherwise.
   */
  async isAvailable(): Promise<boolean> {
    try {
      const res = await fetch(`${this.agentUrl}/available`, {
        method: "GET",
        signal: buildAbortSignal(this.timeout),
      });
      return res.ok;
    } catch {
      return false;
    }
  }

  // ── Discovery ────────────────────────────────────────────────────────

  /**
   * Discover Zebra printers available through the local agent.
   *
   * @returns An array of {@link PrinterDevice} objects.
   */
  async discover(): Promise<PrinterDevice[]> {
    const res = await fetch(`${this.agentUrl}/available`, {
      method: "GET",
      signal: buildAbortSignal(this.timeout),
    });

    if (!res.ok) {
      throw new Error(
        `Zebra Browser Print agent returned ${res.status}: ${res.statusText}`
      );
    }

    const text = await res.text();

    // The agent returns a newline-separated list of device records.
    // Each record is a set of key/value pairs separated by \t within the line.
    // Different firmware versions may return JSON or the legacy format.
    try {
      // Try parsing as JSON first (newer agent versions).
      const json = JSON.parse(text);
      if (Array.isArray(json)) {
        return json.map(mapDevice);
      }
      // Single device object.
      if (json && typeof json === "object") {
        return [mapDevice(json)];
      }
    } catch {
      // Fall through to legacy parsing.
    }

    // Legacy tab-separated format.
    return parseLegacyDeviceList(text);
  }

  // ── Printing ─────────────────────────────────────────────────────────

  /**
   * Send ZPL to a specific printer through the agent.
   *
   * @param device - The target printer device (from {@link discover}).
   * @param zpl    - The ZPL II command string to send.
   */
  async print(device: PrinterDevice, zpl: string): Promise<void> {
    const res = await fetch(`${this.agentUrl}/write`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device: device.uid, data: zpl }),
      signal: buildAbortSignal(this.timeout),
    });

    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(
        `Print failed (${res.status}): ${body || res.statusText}`
      );
    }
  }

  // ── Status ───────────────────────────────────────────────────────────

  /**
   * Query the host status of a printer through the agent.
   *
   * Sends `~HS` to the device and parses the response into a
   * {@link PrinterStatus} object.
   *
   * @param device - The target printer device.
   */
  async getStatus(device: PrinterDevice): Promise<PrinterStatus> {
    // Send the ~HS command through the agent's read-after-write flow.
    const writeRes = await fetch(`${this.agentUrl}/write`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device: device.uid, data: "~HS" }),
      signal: buildAbortSignal(this.timeout),
    });

    if (!writeRes.ok) {
      const body = await writeRes.text().catch(() => "");
      throw new Error(
        `Status query write failed (${writeRes.status}): ${body || writeRes.statusText}`
      );
    }

    // Allow the printer time to prepare its response before reading.
    await new Promise<void>((r) => setTimeout(r, 200));

    // Read the response back from the printer.
    const readRes = await fetch(`${this.agentUrl}/read`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device: device.uid }),
      signal: buildAbortSignal(this.timeout),
    });

    if (!readRes.ok) {
      const body = await readRes.text().catch(() => "");
      throw new Error(
        `Status query read failed (${readRes.status}): ${body || readRes.statusText}`
      );
    }

    const raw = await readRes.text();
    return parseHostStatus(raw);
  }
}

// ─── Device mapping helpers ──────────────────────────────────────────────────

function mapDevice(obj: Record<string, unknown>): PrinterDevice {
  return {
    name: String(obj.name ?? obj.Name ?? "Unknown"),
    uid: String(obj.uid ?? obj.UID ?? obj.uniqueId ?? ""),
    connection: String(obj.connection ?? obj.Connection ?? "unknown"),
    deviceType: String(obj.deviceType ?? obj.DeviceType ?? "unknown"),
    provider: String(obj.provider ?? obj.Provider ?? "unknown"),
    manufacturer: String(obj.manufacturer ?? obj.Manufacturer ?? "unknown"),
  };
}

function parseLegacyDeviceList(text: string): PrinterDevice[] {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);

  return lines.map((line) => {
    const fields = line.split("\t");
    return {
      name: fields[0] ?? "Unknown",
      uid: fields[1] ?? "",
      connection: fields[2] ?? "unknown",
      deviceType: fields[3] ?? "unknown",
      provider: fields[4] ?? "unknown",
      manufacturer: fields[5] ?? "unknown",
    };
  });
}
