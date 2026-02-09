import { describe, it, afterEach, mock } from "node:test";
import assert from "node:assert/strict";
import { ZebraBrowserPrint } from "../browser.js";
import type { PrinterDevice } from "../browser.js";

const AGENT_URL = "http://test-agent:9100";

/** Shorthand for a successful (200) Response. */
function ok(body: string): Response {
  return new Response(body, { status: 200 });
}

/** Shorthand for an error Response. */
function err(status: number, body = ""): Response {
  return new Response(body, { status, statusText: "Error" });
}

// ─── Test suite ──────────────────────────────────────────────────────────────

describe("ZebraBrowserPrint", () => {
  afterEach(() => {
    mock.restoreAll();
  });

  // ── Constructor ─────────────────────────────────────────────────────────

  describe("constructor", () => {
    it("uses custom agentUrl when provided", async () => {
      const customUrl = "http://custom-host:1234";
      const fetchMock = mock.method(globalThis, "fetch", () =>
        Promise.resolve(ok(""))
      );
      const zbp = new ZebraBrowserPrint({ agentUrl: customUrl });

      await zbp.isAvailable();

      assert.equal(fetchMock.mock.calls.length, 1);
      const url = String(fetchMock.mock.calls[0].arguments[0]);
      assert.ok(
        url.startsWith(customUrl),
        `expected URL to start with ${customUrl}, got ${url}`
      );
    });

    it("defaults to http://127.0.0.1:9100 outside browser context", async () => {
      const fetchMock = mock.method(globalThis, "fetch", () =>
        Promise.resolve(ok(""))
      );
      const zbp = new ZebraBrowserPrint();

      await zbp.isAvailable();

      const url = String(fetchMock.mock.calls[0].arguments[0]);
      assert.ok(
        url.startsWith("http://127.0.0.1:9100"),
        `expected default URL, got ${url}`
      );
    });
  });

  // ── isAvailable ─────────────────────────────────────────────────────────

  describe("isAvailable", () => {
    it("returns true when agent responds with 200", async () => {
      mock.method(globalThis, "fetch", () => Promise.resolve(ok("ok")));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      assert.equal(await zbp.isAvailable(), true);
    });

    it("returns false when agent is unreachable (fetch throws)", async () => {
      mock.method(globalThis, "fetch", () =>
        Promise.reject(new Error("ECONNREFUSED"))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      assert.equal(await zbp.isAvailable(), false);
    });

    it("returns false when agent returns non-200", async () => {
      mock.method(globalThis, "fetch", () => Promise.resolve(err(503)));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      assert.equal(await zbp.isAvailable(), false);
    });
  });

  // ── discover ────────────────────────────────────────────────────────────

  describe("discover", () => {
    it("parses JSON array device response", async () => {
      const devices = [
        {
          name: "ZD421",
          uid: "abc123",
          connection: "network",
          deviceType: "printer",
          provider: "usb",
          manufacturer: "Zebra",
        },
      ];
      mock.method(globalThis, "fetch", () =>
        Promise.resolve(ok(JSON.stringify(devices)))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 1);
      assert.equal(result[0].name, "ZD421");
      assert.equal(result[0].uid, "abc123");
      assert.equal(result[0].connection, "network");
      assert.equal(result[0].deviceType, "printer");
      assert.equal(result[0].provider, "usb");
      assert.equal(result[0].manufacturer, "Zebra");
    });

    it("parses JSON single object response", async () => {
      const device = {
        name: "ZD620",
        uid: "xyz789",
        connection: "usb",
        deviceType: "printer",
        provider: "driver",
        manufacturer: "Zebra",
      };
      mock.method(globalThis, "fetch", () =>
        Promise.resolve(ok(JSON.stringify(device)))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 1);
      assert.equal(result[0].name, "ZD620");
      assert.equal(result[0].uid, "xyz789");
    });

    it("maps alternative field names (Name, UID, Connection, etc.)", async () => {
      const devices = [
        {
          Name: "ZD421",
          UID: "abc123",
          Connection: "network",
          DeviceType: "printer",
          Provider: "usb",
          Manufacturer: "Zebra",
        },
      ];
      mock.method(globalThis, "fetch", () =>
        Promise.resolve(ok(JSON.stringify(devices)))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result[0].name, "ZD421");
      assert.equal(result[0].uid, "abc123");
      assert.equal(result[0].connection, "network");
    });

    it("throws when agent returns non-200", async () => {
      mock.method(globalThis, "fetch", () =>
        Promise.resolve(err(500, "Internal Server Error"))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      await assert.rejects(() => zbp.discover(), /500/);
    });

    // ── parseLegacyDeviceList edge cases (tested via discover) ──────────

    it("parses legacy tab-separated format", async () => {
      const legacy =
        "ZD421\tabc123\tnetwork\tprinter\tusb\tZebra\n" +
        "ZD620\txyz789\tusb\tprinter\tdriver\tZebra";
      mock.method(globalThis, "fetch", () => Promise.resolve(ok(legacy)));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 2);
      assert.equal(result[0].name, "ZD421");
      assert.equal(result[0].uid, "abc123");
      assert.equal(result[0].connection, "network");
      assert.equal(result[1].name, "ZD620");
      assert.equal(result[1].uid, "xyz789");
      assert.equal(result[1].connection, "usb");
    });

    it("returns empty array for empty legacy response", async () => {
      mock.method(globalThis, "fetch", () => Promise.resolve(ok("")));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 0);
    });

    it("handles legacy format with missing fields (defaults)", async () => {
      const legacy = "ZD421\tabc123";
      mock.method(globalThis, "fetch", () => Promise.resolve(ok(legacy)));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 1);
      assert.equal(result[0].name, "ZD421");
      assert.equal(result[0].uid, "abc123");
      assert.equal(result[0].connection, "unknown");
      assert.equal(result[0].deviceType, "unknown");
      assert.equal(result[0].provider, "unknown");
      assert.equal(result[0].manufacturer, "unknown");
    });

    it("ignores blank lines in legacy format", async () => {
      const legacy =
        "ZD421\tabc123\tnetwork\tprinter\tusb\tZebra\n" +
        "\n" +
        "\n" +
        "ZD620\txyz789\tusb\tprinter\tdriver\tZebra\n";
      mock.method(globalThis, "fetch", () => Promise.resolve(ok(legacy)));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 2);
    });

    it("handles single device in legacy format", async () => {
      const legacy = "ZD421\tabc123\tnetwork\tprinter\tusb\tZebra";
      mock.method(globalThis, "fetch", () => Promise.resolve(ok(legacy)));

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const result = await zbp.discover();

      assert.equal(result.length, 1);
      assert.equal(result[0].name, "ZD421");
    });
  });

  // ── print ───────────────────────────────────────────────────────────────

  describe("print", () => {
    const device: PrinterDevice = {
      name: "ZD421",
      uid: "abc123",
      connection: "network",
      deviceType: "printer",
      provider: "usb",
      manufacturer: "Zebra",
    };

    it("sends correct request body to /write", async () => {
      const fetchMock = mock.method(globalThis, "fetch", () =>
        Promise.resolve(ok(""))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      await zbp.print(device, "^XA^FDHello^FS^XZ");

      assert.equal(fetchMock.mock.calls.length, 1);
      const call = fetchMock.mock.calls[0];

      // URL
      const url = String(call.arguments[0]);
      assert.equal(url, `${AGENT_URL}/write`);

      // Method + headers
      const init = call.arguments[1] as RequestInit;
      assert.equal(init.method, "POST");

      // Body
      const body = JSON.parse(init.body as string);
      assert.equal(body.device, "abc123");
      assert.equal(body.data, "^XA^FDHello^FS^XZ");
    });

    it("throws on non-200 response", async () => {
      mock.method(globalThis, "fetch", () =>
        Promise.resolve(err(500, "printer offline"))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      await assert.rejects(
        () => zbp.print(device, "^XA^FDHello^FS^XZ"),
        /Print failed.*500/
      );
    });
  });

  // ── getStatus ───────────────────────────────────────────────────────────

  describe("getStatus", () => {
    const device: PrinterDevice = {
      name: "ZD421",
      uid: "abc123",
      connection: "network",
      deviceType: "printer",
      provider: "usb",
      manufacturer: "Zebra",
    };

    it("sends write + read requests and parses status", async () => {
      const hsResponse =
        "\x02030,0,0,1245,000,0,0,0,000,0,0,0\x03\r\n" +
        "\x02000,0,0,0,0,2,4,0,00000000,1,000\x03\r\n" +
        "\x021234,0\x03";

      let callIndex = 0;
      const fetchMock = mock.method(globalThis, "fetch", () => {
        callIndex++;
        // First call (write) returns empty ok; second call (read) returns HS
        return Promise.resolve(ok(callIndex === 1 ? "" : hsResponse));
      });

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      const status = await zbp.getStatus(device);

      // Two fetch calls: write then read
      assert.equal(fetchMock.mock.calls.length, 2);

      // First call: POST /write with ~HS command
      const writeUrl = String(fetchMock.mock.calls[0].arguments[0]);
      assert.equal(writeUrl, `${AGENT_URL}/write`);
      const writeInit = fetchMock.mock.calls[0].arguments[1] as RequestInit;
      const writeBody = JSON.parse(writeInit.body as string);
      assert.equal(writeBody.device, "abc123");
      assert.equal(writeBody.data, "~HS");

      // Second call: POST /read
      const readUrl = String(fetchMock.mock.calls[1].arguments[0]);
      assert.equal(readUrl, `${AGENT_URL}/read`);
      const readInit = fetchMock.mock.calls[1].arguments[1] as RequestInit;
      const readBody = JSON.parse(readInit.body as string);
      assert.equal(readBody.device, "abc123");

      // Parsed status fields
      assert.equal(status.ready, true);
      assert.equal(status.paperOut, false);
      assert.equal(status.paused, false);
      assert.equal(status.labelLengthDots, 1245);
      assert.equal(status.password, 1234);
    });

    it("throws when write request fails", async () => {
      mock.method(globalThis, "fetch", () =>
        Promise.resolve(err(500, "write error"))
      );

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      await assert.rejects(() => zbp.getStatus(device), /write failed.*500/);
    });

    it("throws when read request fails", async () => {
      let callIndex = 0;
      mock.method(globalThis, "fetch", () => {
        callIndex++;
        if (callIndex === 1) return Promise.resolve(ok(""));
        return Promise.resolve(err(500, "read error"));
      });

      const zbp = new ZebraBrowserPrint({ agentUrl: AGENT_URL });
      await assert.rejects(() => zbp.getStatus(device), /read failed.*500/);
    });
  });
});
