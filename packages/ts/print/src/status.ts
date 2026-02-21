import type { PrintMode, PrinterStatus } from "./types.js";

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
const HS_STRICT_FRAME_COUNT = 3;
const HS_LINE_FIELD_COUNTS = [12, 10, 2] as const;

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

function extractFramedPayloads(raw: string): string[] {
  const payloads: string[] = [];
  let inFrame = false;
  let current = "";

  for (const ch of raw) {
    if (!inFrame) {
      if (ch === STX) {
        inFrame = true;
        current = "";
      }
      continue;
    }

    if (ch === ETX) {
      payloads.push(current);
      inFrame = false;
      current = "";
      continue;
    }

    current += ch;
  }

  return payloads;
}

function parseHostStatusFromLines(
  line1: string[],
  line2: string[],
  line3: string[],
  raw: string
): PrinterStatus {
  const parsePrintMode = (value: number): PrintMode => {
    switch (value) {
      case 0:
        return "TearOff";
      case 1:
        return "PeelOff";
      case 2:
        return "Rewind";
      case 3:
        return "Applicator";
      case 4:
        return "Cutter";
      case 5:
        return "DelayedCutter";
      case 6:
        return "Linerless";
      default:
        return "Unknown";
    }
  };

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
  const printMode = parsePrintMode(int(line2, 4));
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

function parseStrictInt(fields: string[], idx: number, lineNo: number): number {
  const raw = fields[idx];
  if (raw === undefined) {
    throw new Error(
      `~HS line ${lineNo}: expected field at index ${idx}, only got ${fields.length} fields`
    );
  }
  const parsed = parseInt(raw.trim(), 10);
  if (Number.isNaN(parsed)) {
    throw new Error(
      `~HS line ${lineNo}: cannot parse field ${idx} (${JSON.stringify(raw)}) as int`
    );
  }
  return parsed;
}

function parseStrictPrintMode(fields: string[], idx: number, lineNo: number): string {
  const mode = parseStrictInt(fields, idx, lineNo);
  if (mode < 0 || mode > 6) {
    throw new Error(`~HS line ${lineNo}: unknown print mode code: ${mode}`);
  }
  return mode.toString();
}

function parseStrictBool(fields: string[], idx: number, lineNo: number): boolean {
  return parseStrictInt(fields, idx, lineNo) !== 0;
}

/**
 * Strictly parse a raw ~HS response string.
 *
 * Requires exactly 3 STX/ETX frames and numeric parseability for all expected
 * fields. Throws an error on malformed/truncated responses.
 */
export function parseHostStatusStrict(raw: string): PrinterStatus {
  const frames = extractFramedPayloads(raw);
  if (frames.length !== HS_STRICT_FRAME_COUNT) {
    throw new Error(`~HS requires 3 frames, got ${frames.length}`);
  }

  const line1 = frames[0]!.split(",").map((f) => f.trim());
  const line2 = frames[1]!.split(",").map((f) => f.trim());
  const line3 = frames[2]!.split(",").map((f) => f.trim());

  if (line1.length < HS_LINE_FIELD_COUNTS[0]) {
    throw new Error(
      `~HS line 1: expected at least ${HS_LINE_FIELD_COUNTS[0]} fields, got ${line1.length}`
    );
  }
  if (line2.length < HS_LINE_FIELD_COUNTS[1]) {
    throw new Error(
      `~HS line 2: expected at least ${HS_LINE_FIELD_COUNTS[1]} fields, got ${line2.length}`
    );
  }
  if (line3.length < HS_LINE_FIELD_COUNTS[2]) {
    throw new Error(
      `~HS line 3: expected at least ${HS_LINE_FIELD_COUNTS[2]} fields, got ${line3.length}`
    );
  }

  const strictLine1 = [
    parseStrictInt(line1, 0, 1).toString(),
    parseStrictBool(line1, 1, 1) ? "1" : "0",
    parseStrictBool(line1, 2, 1) ? "1" : "0",
    parseStrictInt(line1, 3, 1).toString(),
    parseStrictInt(line1, 4, 1).toString(),
    parseStrictBool(line1, 5, 1) ? "1" : "0",
    parseStrictBool(line1, 6, 1) ? "1" : "0",
    parseStrictBool(line1, 7, 1) ? "1" : "0",
    parseStrictInt(line1, 8, 1).toString(),
    parseStrictBool(line1, 9, 1) ? "1" : "0",
    parseStrictBool(line1, 10, 1) ? "1" : "0",
    parseStrictBool(line1, 11, 1) ? "1" : "0",
  ];

  const strictLine2 = [
    parseStrictInt(line2, 0, 2).toString(),
    parseStrictBool(line2, 1, 2) ? "1" : "0",
    parseStrictBool(line2, 2, 2) ? "1" : "0",
    parseStrictBool(line2, 3, 2) ? "1" : "0",
    parseStrictPrintMode(line2, 4, 2),
    parseStrictInt(line2, 5, 2).toString(),
    parseStrictBool(line2, 6, 2) ? "1" : "0",
    parseStrictInt(line2, 7, 2).toString(),
    parseStrictInt(line2, 8, 2).toString(),
    parseStrictInt(line2, 9, 2).toString(),
  ];

  const strictLine3 = [
    parseStrictInt(line3, 0, 3).toString(),
    parseStrictBool(line3, 1, 3) ? "1" : "0",
  ];

  return parseHostStatusFromLines(strictLine1, strictLine2, strictLine3, raw);
}

/**
 * Leniently parse a raw ~HS response string into a structured
 * {@link PrinterStatus} object.
 *
 * Returns sensible defaults for fields it cannot extract.
 */
export function parseHostStatusLenient(raw: string): PrinterStatus {
  const lines = splitHsLines(raw);
  const line1 = lines[0] ?? [];
  const line2 = lines[1] ?? [];
  const line3 = lines[2] ?? [];
  return parseHostStatusFromLines(line1, line2, line3, raw);
}

/**
 * Parse a raw ~HS (Host Status) response string into a structured
 * {@link PrinterStatus} object using strict framing and field validation.
 *
 * @param raw - The full ~HS response as received from the printer socket.
 * @returns A fully parsed {@link PrinterStatus}.
 * @throws Error when response framing or fields are malformed.
 */
export function parseHostStatus(raw: string): PrinterStatus {
  return parseHostStatusStrict(raw);
}
