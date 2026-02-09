import type { PrinterStatus } from "./types.js";

// ─── ~HS Response Parser ─────────────────────────────────────────────────────
//
// The Zebra ~HS (Host Status) command returns three lines of comma-separated
// fields framed with STX (\x02) and ETX (\x03).
//
//   Line 1  (prefix \x02): communication / paper / head flags
//   Line 2  (prefix \x02): function settings
//   Line 3  (prefix \x02): miscellaneous
//
// Each line is delimited by CR/LF and wrapped in STX…ETX.
// Reference: ZPL II Programming Guide, chapter "Host Status Return".
// ─────────────────────────────────────────────────────────────────────────────

const STX = "\x02";
const ETX = "\x03";

/**
 * Strip STX / ETX framing characters and split the response into its three
 * comma-separated field arrays.
 */
function splitHsLines(raw: string): string[][] {
  // Remove STX/ETX framing, normalise line endings, trim.
  const cleaned = raw
    .replaceAll(STX, "")
    .replaceAll(ETX, "")
    .replace(/\r\n?/g, "\n")
    .trim();

  const lines = cleaned.split("\n").filter((l) => l.trim().length > 0);

  return lines.map((line) =>
    line.split(",").map((f) => f.trim())
  );
}

/**
 * Parse a raw ~HS (Host Status) response string into a structured
 * {@link PrinterStatus} object.
 *
 * The parser is lenient: it returns sensible defaults for fields it cannot
 * extract, so callers never need to worry about partial / malformed responses
 * in the field.
 *
 * @param raw - The full ~HS response as received from the printer socket.
 * @returns A {@link PrinterStatus} with boolean flags and remaining-label count.
 */
export function parseHostStatus(raw: string): PrinterStatus {
  const lines = splitHsLines(raw);

  const line1 = lines[0] ?? [];
  const line2 = lines[1] ?? [];
  const line3 = lines[2] ?? [];

  const flag = (fields: string[], idx: number): boolean =>
    (fields[idx] ?? "0") !== "0";

  const int = (fields: string[], idx: number): number => {
    const n = parseInt(fields[idx] ?? "", 10);
    return Number.isNaN(n) ? 0 : n;
  };

  // ── Line 1 (12 fields) ────────────────────────────────────────────
  const communicationFlag = int(line1, 0);
  const paperOut = flag(line1, 1);
  const paused = flag(line1, 2);
  const labelLengthDots = int(line1, 3);
  const formatsInBuffer = int(line1, 4);
  const bufferFull = flag(line1, 5);
  const commDiagMode = flag(line1, 6);
  const partialFormat = flag(line1, 7);
  const reserved1 = int(line1, 8);
  const corruptRam = flag(line1, 9);
  const underTemperature = flag(line1, 10);
  const overTemperature = flag(line1, 11);

  // ── Line 2 (10 fields) ────────────────────────────────────────────
  const functionSettings = int(line2, 0);
  const headOpen = flag(line2, 1);
  const ribbonOut = flag(line2, 2);
  const thermalTransferMode = flag(line2, 3);
  const printMode = int(line2, 4);
  const printWidthMode = int(line2, 5);
  const labelWaiting = flag(line2, 6);
  const labelsRemaining = int(line2, 7);
  const formatWhilePrinting = int(line2, 8);
  const graphicsStoredInMemory = int(line2, 9);

  // ── Line 3 (2 fields) ─────────────────────────────────────────────
  const password = int(line3, 0);
  const staticRamInstalled = flag(line3, 1);

  // "ready" = not paused, head closed, media loaded.
  const ready = !paused && !headOpen && !paperOut;

  return {
    ready,
    communicationFlag,
    paperOut,
    paused,
    labelLengthDots,
    formatsInBuffer,
    bufferFull,
    commDiagMode,
    partialFormat,
    reserved1,
    corruptRam,
    underTemperature,
    overTemperature,
    functionSettings,
    headOpen,
    ribbonOut,
    thermalTransferMode,
    printMode,
    printWidthMode,
    labelWaiting,
    labelsRemaining,
    formatWhilePrinting,
    graphicsStoredInMemory,
    password,
    staticRamInstalled,
    raw,
  };
}
