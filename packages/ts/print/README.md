# @zpl-toolchain/print

![ZPL Toolchain logo](https://raw.githubusercontent.com/trevordcampbell/zpl-toolchain/main/docs/assets/branding/logo-square-128.png)

Send ZPL to Zebra and ZPL-compatible label printers from Node.js. Supports persistent TCP connections, batch printing, status queries, browser printing, and a built-in HTTP/WebSocket proxy for web apps.

> Transport scope: this package is **TCP-focused** (`host`/`port`).
> For USB (`--printer usb`) and serial/Bluetooth SPP (`--serial`), use the `zpl` CLI or the Rust `zpl_toolchain_print_client` API.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Installation

```bash
npm install @zpl-toolchain/print
```

## Quick Start

```ts
import { print, TcpPrinter } from "@zpl-toolchain/print";

// One-shot: send ZPL and disconnect
const result = await print("^XA^FDHello^FS^XZ", { host: "192.168.1.55" });
console.log(result); // { success: true, bytesWritten: 21 }

// Persistent connection: reuse for multiple labels
const printer = new TcpPrinter({ host: "192.168.1.55" });
await printer.print("^XA^FDLabel 1^FS^XZ");
await printer.print("^XA^FDLabel 2^FS^XZ");

// Check if a printer is reachable
if (await printer.isReachable()) {
  console.log("Printer is online");
}

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
- **Abort support** — `AbortSignal` support on one-shot and batch/completion flows
- **Validated printing** — `printValidated()` validates ZPL before sending (requires optional `@zpl-toolchain/core` peer dependency)
- **HTTP/WebSocket proxy** — `createPrintProxy()` bridges browser apps to network printers with CORS, SSRF protection, allowlists, connection limits, optional per-client WS rate limiting, and correlation IDs
- **Browser printing** — `ZebraBrowserPrint` wraps the Zebra Browser Print agent for printing from the browser via `fetch()`
- **Full TypeScript types** — all APIs are fully typed with exported interfaces

## API

### One-shot functions

| Function | Description |
|----------|-------------|
| `print(zpl, config)` | Send ZPL and disconnect |
| `printValidated(zpl, config, opts?)` | Validate then print (requires `@zpl-toolchain/core`) |
| `createPrinter(config)` | Convenience factory for `new TcpPrinter(config)` |

### `TcpPrinter` — persistent connection

| Method | Description |
|--------|-------------|
| `new TcpPrinter(config)` | Create a printer (connects lazily on first call) |
| `print(zpl, opts?)` | Send ZPL over the persistent connection |
| `isReachable()` | Check if the printer accepts TCP connections |
| `getStatus(opts?)` | Query `~HS` host status (paper out, paused, labels remaining, etc.); supports `AbortSignal` |
| `query(command, opts?)` | Send an arbitrary command (e.g. `~HI`) and return raw response; supports `AbortSignal` |
| `printBatch(labels, opts?, onProgress?)` | Send multiple labels with optional status polling and progress callbacks |
| `waitForCompletion(pollInterval?, timeoutMs?, signal?)` | Poll until all labels are printed, timeout, or abort |
| `close()` | Gracefully close the connection |

### Proxy — `@zpl-toolchain/print/proxy`

```ts
import { createPrintProxy } from "@zpl-toolchain/print/proxy";

const server = createPrintProxy({
  port: 3001,
  allowedPrinters: ["192.168.1.*", "printer.local"],
  allowedPorts: [9100],
  wsRateLimitPerClient: { maxRequests: 10, windowMs: 1000 },
  cors: "*",
});
// POST /print  { printer, port?, zpl }
// POST /status { printer, port? }
// WebSocket: send { id?, type: "print"|"status", printer, port?, zpl? }
// WebSocket responses echo id when provided.
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

- **`PrinterConfig`** — host, port, timeout, retries, `signal`
- **`PrintResult`** — success, bytesWritten, error details
- **`PrinterStatus`** — 24-field parsed `~HS` response
- **`PrintError`** — typed error with `code` (CONNECTION_REFUSED, TIMEOUT, etc.)
- **`BatchOptions`** / **`BatchProgress`** / **`BatchResult`** — batch printing control (including partial error details where `error.index` is the 0-based failed label index)
- **`ProxyConfig`** — proxy server configuration
- **`ValidateOptions`** — options for validated printing

## Requirements

- **Node.js 18+** (uses `node:net` for TCP)
- **Optional**: `@zpl-toolchain/core` peer dependency for `printValidated()`

## Documentation

See the [Print Client Guide](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/PRINT_CLIENT.md) for comprehensive usage, CLI integration, proxy setup, and troubleshooting.

## License

Dual-licensed under MIT or Apache-2.0.
