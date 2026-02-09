// ─── Printer connection configuration ────────────────────────────────────────

/** Configuration for connecting to a ZPL printer over TCP/IP. */
export interface PrinterConfig {
  /** Printer IP address or hostname. */
  host: string;

  /** TCP port (default: 9100 — the standard ZPL raw printing port). */
  port?: number;

  /** Connection timeout in milliseconds (default: 5000). */
  timeout?: number;

  /** Maximum number of retry attempts on transient errors (default: 2). */
  maxRetries?: number;

  /** Base delay between retries in milliseconds; grows with exponential backoff (default: 500). */
  retryDelay?: number;
}

// ─── Print result ────────────────────────────────────────────────────────────

/** Outcome of a print operation. */
export interface PrintResult {
  /** Whether the ZPL was successfully sent to the printer. */
  success: boolean;

  /** Number of bytes written to the printer socket. */
  bytesWritten: number;

  /** Wall-clock duration of the operation in milliseconds. */
  duration: number;
}

// ─── Printer status (~HS response) ───────────────────────────────────────────

/** Parsed representation of the Zebra ~HS (Host Status) response.
 *
 * Contains all 24 fields from the three response lines,
 * matching the Rust `HostStatus` struct for full parity.
 */
export interface PrinterStatus {
  // ── Convenience ────────────────────────────────────────────────────
  /** Printer is ready to accept data (not paused, head closed, media loaded). */
  ready: boolean;

  // ── Line 1: Communication / paper / head flags ─────────────────────
  /** Communication settings (field 0). */
  communicationFlag: number;
  /** Media (label stock) is depleted or not detected (field 1). */
  paperOut: boolean;
  /** Printer is in a paused state (field 2). */
  paused: boolean;
  /** Label length in dots (field 3). */
  labelLengthDots: number;
  /** Number of formats waiting in the receive buffer (field 4). */
  formatsInBuffer: number;
  /** Receive-buffer full (field 5). */
  bufferFull: boolean;
  /** Communications diagnostic mode active (field 6). */
  commDiagMode: boolean;
  /** Partial format in progress (field 7). */
  partialFormat: boolean;
  /** Reserved/unused field (field 8). */
  reserved1: number;
  /** Corrupt RAM detected (field 9). */
  corruptRam: boolean;
  /** Under-temperature condition (field 10). */
  underTemperature: boolean;
  /** Over-temperature condition (field 11). */
  overTemperature: boolean;

  // ── Line 2: Function / print settings ──────────────────────────────
  /** Function settings bitmask (field 0). */
  functionSettings: number;
  /** Print head is open / not latched (field 1). */
  headOpen: boolean;
  /** Ribbon cartridge is depleted or missing (field 2). */
  ribbonOut: boolean;
  /** Thermal transfer mode active (field 3). */
  thermalTransferMode: boolean;
  /** Print mode code (field 4): 0=tear-off, 1=peel-off, 2=rewind, etc. */
  printMode: number;
  /** Print width mode (field 5). */
  printWidthMode: number;
  /** Label waiting to be taken (field 6). */
  labelWaiting: boolean;
  /** Number of labels remaining in the current batch (field 7). */
  labelsRemaining: number;
  /** Format-while-printing flag/mask (field 8). */
  formatWhilePrinting: number;
  /** Number of graphics stored in memory (field 9). */
  graphicsStoredInMemory: number;

  // ── Line 3: Miscellaneous ──────────────────────────────────────────
  /** Password value (field 0). */
  password: number;
  /** Static RAM installed flag (field 1). */
  staticRamInstalled: boolean;

  /** The raw ~HS response string for advanced inspection. */
  raw: string;
}

// ─── Browser Print SDK types ─────────────────────────────────────────────────

/** A printer device as reported by the Zebra Browser Print agent. */
export interface PrinterDevice {
  /** Human-readable device name (e.g. "ZD421"). */
  name: string;

  /** Unique device identifier. */
  uid: string;

  /** Connection type reported by the agent (e.g. "network", "usb", "driver"). */
  connection: string;

  /** Device type string. */
  deviceType: string;

  /** Provider identifier. */
  provider: string;

  /** Manufacturer string. */
  manufacturer: string;
}

// ─── Print proxy types ───────────────────────────────────────────────────────

/** Configuration for the HTTP print proxy server. */
export interface ProxyConfig {
  /** Port the proxy listens on (default: 3001). */
  port?: number;

  /** Hostname to bind to (default: "127.0.0.1"). */
  hostname?: string;

  /**
   * List of allowed printer IPs / hostnames.
   * Required — the proxy rejects all requests when this is empty or undefined (SSRF protection).
   * Supports simple glob patterns with `*` (e.g., `"192.168.1.*"`).
   */
  allowedPrinters?: string[];

  /**
   * List of allowed destination ports.
   * Defaults to `[9100]`. Restricts which ports the proxy can connect to,
   * preventing port-scanning attacks on allowed hosts.
   */
  allowedPorts?: number[];

  /**
   * Maximum number of concurrent WebSocket connections.
   * Defaults to 50. New connections are rejected when the limit is reached.
   */
  maxConnections?: number;

  /** Maximum request body size in bytes (default: 1 MiB). */
  maxPayloadSize?: number;

  /**
   * CORS allowed origins.
   * `"*"` allows any origin; an array restricts to specific origins.
   * Default: `"*"`.
   */
  cors?: string | string[];
}

// ─── Validation options for printValidated ───────────────────────────────────

/** Options for pre-print validation via `printValidated()`. */
export interface ValidateOptions {
  /**
   * Printer profile JSON string for profile-aware validation.
   * Enables DPI-specific, media-specific, and hardware-gated checks.
   *
   * Pass the raw JSON string of a printer profile (e.g., `fs.readFileSync("ZD421.json", "utf-8")`).
   */
  profileJson?: string;

  /**
   * Whether to treat warnings as errors (abort printing if any warnings are found).
   * Default: `false`.
   */
  strict?: boolean;
}

// ─── Batch printing ──────────────────────────────────────────────────────────

/** Options for batch printing with status polling. */
export interface BatchOptions {
  /**
   * Poll printer status (`~HS`) every N labels.
   * When undefined or 0, no status polling is performed.
   */
  statusInterval?: number;
}

/** Progress update emitted after each label in a batch. */
export interface BatchProgress {
  /** Number of labels sent so far. */
  sent: number;

  /** Total number of labels in the batch. */
  total: number;

  /** Latest printer status (only present when `statusInterval` is set). */
  status?: PrinterStatus;
}

/** Final result of a batch print operation. */
export interface BatchResult {
  /** Number of labels successfully sent. */
  sent: number;

  /** Total number of labels in the batch. */
  total: number;
}

// ─── Error classification ────────────────────────────────────────────────────

/** Well-known error codes surfaced by the print client. */
export type PrintErrorCode =
  | "CONNECTION_REFUSED"
  | "TIMEOUT"
  | "HOST_NOT_FOUND"
  | "BROKEN_PIPE"
  | "CONNECTION_RESET"
  | "VALIDATION_FAILED"
  | "UNKNOWN";

/** An error thrown by the print client with a classified code. */
export class PrintError extends Error {
  public readonly code: PrintErrorCode;

  constructor(message: string, code: PrintErrorCode, cause?: unknown) {
    super(message);
    this.name = "PrintError";
    this.code = code;
    if (cause) {
      this.cause = cause;
    }
  }
}
