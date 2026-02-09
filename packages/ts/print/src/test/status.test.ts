import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import { parseHostStatus } from "../status.js";

// A realistic ~HS response from a Zebra ZD421
const MOCK_HS_RESPONSE =
  "\x02030,0,0,1245,000,0,0,0,000,0,0,0\x03\r\n" +
  "\x02000,0,0,0,0,2,4,0,00000000,1,000\x03\r\n" +
  "\x021234,0\x03";

// ~HS with error conditions
const MOCK_HS_ERRORS =
  "\x02030,1,1,1245,003,1,0,0,000,1,1,1\x03\r\n" +
  "\x02000,1,1,1,3,2,0,5,00000000,1,000\x03\r\n" +
  "\x021234,1\x03";

describe("parseHostStatus", () => {
  it("parses a healthy printer response", () => {
    const status = parseHostStatus(MOCK_HS_RESPONSE);

    assert.equal(status.ready, true);
    assert.equal(status.paperOut, false);
    assert.equal(status.paused, false);
    assert.equal(status.labelLengthDots, 1245);
    assert.equal(status.formatsInBuffer, 0);
    assert.equal(status.bufferFull, false);
    assert.equal(status.commDiagMode, false);
    assert.equal(status.partialFormat, false);
    assert.equal(status.corruptRam, false);
    assert.equal(status.underTemperature, false);
    assert.equal(status.overTemperature, false);
    assert.equal(status.headOpen, false);
    assert.equal(status.ribbonOut, false);
    assert.equal(status.labelsRemaining, 0);
    assert.equal(status.password, 1234);
    assert.equal(status.staticRamInstalled, false);
    assert.equal(status.raw, MOCK_HS_RESPONSE);
  });

  it("parses a printer with errors", () => {
    const status = parseHostStatus(MOCK_HS_ERRORS);

    assert.equal(status.ready, false);
    assert.equal(status.paperOut, true);
    assert.equal(status.paused, true);
    assert.equal(status.formatsInBuffer, 3);
    assert.equal(status.bufferFull, true);
    assert.equal(status.corruptRam, true);
    assert.equal(status.underTemperature, true);
    assert.equal(status.overTemperature, true);
    assert.equal(status.headOpen, true);
    assert.equal(status.ribbonOut, true);
    assert.equal(status.labelsRemaining, 5);
    assert.equal(status.thermalTransferMode, true);
    assert.equal(status.printMode, 3);
    assert.equal(status.staticRamInstalled, true);
  });

  it("handles empty input gracefully", () => {
    const status = parseHostStatus("");

    assert.equal(status.ready, true); // no errors detected = ready
    assert.equal(status.paperOut, false);
    assert.equal(status.paused, false);
    assert.equal(status.labelsRemaining, 0);
  });

  it("handles partial response (only line 1)", () => {
    const partial = "\x02030,0,0,800,002,0,0,0,000,0,0,0\x03";
    const status = parseHostStatus(partial);

    assert.equal(status.labelLengthDots, 800);
    assert.equal(status.formatsInBuffer, 2);
    assert.equal(status.labelsRemaining, 0); // line 2 missing, defaults to 0
  });

  it("preserves raw response string", () => {
    const status = parseHostStatus(MOCK_HS_RESPONSE);
    assert.equal(status.raw, MOCK_HS_RESPONSE);
  });

  it("correctly derives ready from flags", () => {
    // paused = true → not ready
    const paused = "\x02030,0,1,1245,000,0,0,0,000,0,0,0\x03\r\n\x02000,0,0,0,0,2,4,0,00000000,1,000\x03\r\n\x021234,0\x03";
    assert.equal(parseHostStatus(paused).ready, false);

    // head open → not ready
    const headOpen = "\x02030,0,0,1245,000,0,0,0,000,0,0,0\x03\r\n\x02000,1,0,0,0,2,4,0,00000000,1,000\x03\r\n\x021234,0\x03";
    assert.equal(parseHostStatus(headOpen).ready, false);

    // paper out → not ready
    const paperOut = "\x02030,1,0,1245,000,0,0,0,000,0,0,0\x03\r\n\x02000,0,0,0,0,2,4,0,00000000,1,000\x03\r\n\x021234,0\x03";
    assert.equal(parseHostStatus(paperOut).ready, false);
  });
});
