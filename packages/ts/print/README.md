# @zpl-toolchain/print

Send ZPL to Zebra and ZPL-compatible label printers from Node.js. Supports persistent TCP connections, batch printing, status queries, browser printing, and a built-in HTTP/WebSocket proxy for web apps.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Installation

```bash
npm install @zpl-toolchain/print
```

## Quick Start

```ts
import { print, TcpPrinter, isReachable } from "@zpl-toolchain/print";

// One-shot: send ZPL and disconnect
const result = await print({ host: "192.168.1.55" }, "^XA^FDHello^FS^XZ");
console.log(result); // { success: true, bytesWritten: 21 }

// Check if a printer is reachable
if (await isReachable({ host: "192.168.1.55" })) {
  console.log("Printer is online");
}

// Persistent connection: reuse for multiple labels
const printer = new TcpPrinter({ host: "192.168.1.55" });
await printer.print("^XA^FDLabel 1^FS^XZ");
await printer.print("^XA^FDLabel 2^FS^XZ");

// Query printer status
const status = await printer.getStatus();
console.log(`Paper out: ${status.paperOut}, Paused: ${status.paused}`);

await printer.close();
```

## Features

- **One-shot printing** — `print()` sends ZPL and disconnects in a single call
- **Persistent connections** — `TcpPrinter` keeps the socket open for high-throughput printing
- **Batch printing** — `TcpPrinter.printBatch()` sends multiple labels with optional progress callbacks and status polling
- **Status queries** — `TcpPrinter.getStatus()` parses the full `~HS` host status response (24 fields)
- **Retry with backoff** — configurable automatic retries on transient errors
- **Validated printing** — `printValidated()` validates ZPL before sending (requires optional `@zpl-toolchain/core` peer dependency)
- **HTTP/WebSocket proxy** — `createPrintProxy()` bridges browser apps to network printers with CORS, SSRF protection, allowlists, and connection limits
- **Browser printing** — `ZebraBrowserPrint` wraps the Zebra Browser Print agent for printing from the browser via `fetch()`
- **Full TypeScript types** — all APIs are fully typed with exported interfaces

## API

### One-shot functions

| Function | Description |
|----------|-------------|
| `print(config, zpl)` | Send ZPL and disconnect |
| `printValidated(config, zpl, opts?)` | Validate then print (requires `@zpl-toolchain/core`) |
| `isReachable(config)` | Check if a printer accepts TCP connections |

### `TcpPrinter` — persistent connection

| Method | Description |
|--------|-------------|
| `new TcpPrinter(config)` | Create a printer (connects lazily on first call) |
| `print(zpl)` | Send ZPL over the persistent connection |
| `getStatus()` | Query `~HS` host status (paper out, paused, labels remaining, etc.) |
| `printBatch(labels, opts?, onProgress?)` | Send multiple labels with optional status polling and progress callbacks |
| `waitForCompletion(timeoutMs?)` | Poll until all labels are printed or timeout |
| `close()` | Gracefully close the connection |

### Proxy — `@zpl-toolchain/print/proxy`

```ts
import { createPrintProxy } from "@zpl-toolchain/print/proxy";

const server = createPrintProxy({
  port: 3001,
  allowed: ["192.168.1.*", "printer.local"],
  allowedPorts: [9100],
  cors: "*",
});
// POST /print  { host, port?, zpl }
// POST /status { host, port? }
// WebSocket: send { action: "print"|"status", host, port?, zpl? }
```

### Browser — `@zpl-toolchain/print/browser`

```ts
import { ZebraBrowserPrint } from "@zpl-toolchain/print/browser";

const zbp = new ZebraBrowserPrint();
if (await zbp.isAvailable()) {
  const printers = await zbp.discover();
  await zbp.print(printers[0], "^XA^FDHello^FS^XZ");
}
```

## Types

All types are exported from the main entry point:

- **`PrinterConfig`** — host, port, timeout, retries
- **`PrintResult`** — success, bytesWritten, error details
- **`PrinterStatus`** — 24-field parsed `~HS` response
- **`PrintError`** — typed error with `code` (CONNECTION_REFUSED, TIMEOUT, etc.)
- **`BatchOptions`** / **`BatchProgress`** / **`BatchResult`** — batch printing control
- **`ProxyConfig`** — proxy server configuration
- **`ValidateOptions`** — options for validated printing

## Requirements

- **Node.js 18+** (uses `node:net` for TCP)
- **Optional**: `@zpl-toolchain/core` peer dependency for `printValidated()`

## Documentation

See the [Print Client Guide](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/PRINT_CLIENT.md) for comprehensive usage, CLI integration, proxy setup, and troubleshooting.

## License

Dual-licensed under MIT or Apache-2.0.
