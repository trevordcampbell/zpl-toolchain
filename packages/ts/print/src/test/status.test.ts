import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import {
  parseHostStatus,
  parseHostStatusLenient,
  parseHostStatusStrict,
} from "../status.js";
import { loadPrintStatusFramingFixture } from "./contracts-fixture.js";

const fixture = loadPrintStatusFramingFixture();
const MOCK_HS_RESPONSE = fixture.host_status.healthy_raw;

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
    assert.equal(status.formatsInBuffer, fixture.host_status.expected_healthy.formats_in_buffer);
    assert.equal(status.bufferFull, false);
    assert.equal(status.commDiagMode, false);
    assert.equal(status.partialFormat, false);
    assert.equal(status.corruptRam, false);
    assert.equal(status.underTemperature, false);
    assert.equal(status.overTemperature, false);
    assert.equal(status.headOpen, fixture.host_status.expected_healthy.head_up);
    assert.equal(status.ribbonOut, fixture.host_status.expected_healthy.ribbon_out);
    assert.equal(status.labelsRemaining, fixture.host_status.expected_healthy.labels_remaining);
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
    assert.equal(status.printMode, "Applicator");
    assert.equal(status.staticRamInstalled, true);
  });

  it("throws on empty input (strict default)", () => {
    assert.throws(
      () => parseHostStatus(""),
      /~HS requires 3 frames/
    );
  });

  it("throws on partial response (strict default)", () => {
    const partial = fixture.host_status.truncated_raw;
    assert.throws(
      () => parseHostStatus(partial),
      /~HS requires 3 frames/
    );
  });

  it("lenient parser handles empty input", () => {
    const status = parseHostStatusLenient("");
    assert.equal(status.ready, true);
    assert.equal(status.paperOut, false);
    assert.equal(status.paused, false);
    assert.equal(status.labelsRemaining, 0);
  });

  it("lenient parser handles partial response", () => {
    const partial = "\x02030,0,0,800,002,0,0,0,000,0,0,0\x03";
    const status = parseHostStatusLenient(partial);

    assert.equal(status.labelLengthDots, 800);
    assert.equal(status.formatsInBuffer, 2);
    assert.equal(status.labelsRemaining, 0); // line 2 missing, defaults to 0
  });

  it("strict parser rejects malformed numeric field", () => {
    const malformed =
      "\x02030,0,0,1245,000,0,0,0,000,0,0,0\x03\r\n" +
      "\x02000,0,0,0,abc,2,4,0,00000000,1,000\x03\r\n" +
      "\x021234,0\x03";
    assert.throws(
      () => parseHostStatusStrict(malformed),
      /cannot parse field 4/
    );
  });

  it("strict parser rejects unknown print mode", () => {
    const unknownMode =
      "\x02030,0,0,1245,000,0,0,0,000,0,0,0\x03\r\n" +
      "\x02000,0,0,0,9,2,4,0,00000000,1,000\x03\r\n" +
      "\x021234,0\x03";
    assert.throws(
      () => parseHostStatusStrict(unknownMode),
      /unknown print mode code: 9/
    );
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
