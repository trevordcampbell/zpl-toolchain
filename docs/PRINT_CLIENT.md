# Print Client

Send ZPL labels to Zebra and ZPL-compatible printers over TCP, USB, or serial/Bluetooth.

---

## Quick Start

### CLI

```bash
# Print a label to a network printer
zpl print label.zpl --printer 192.168.1.55

# Print without validation
zpl print label.zpl --printer 192.168.1.55 --no-lint

# Dry run (validate + resolve address, don't send)
zpl print label.zpl --printer 192.168.1.55 --dry-run

# Print and check printer status afterward
zpl print label.zpl --printer 192.168.1.55 --status

# Print multiple files and wait for completion
zpl print *.zpl --printer printer01.local --wait

# Print via USB
zpl print label.zpl --printer usb

# Print via serial / Bluetooth SPP
zpl print label.zpl --printer /dev/rfcomm0 --serial
```

> **All transports** (TCP, USB, serial/Bluetooth) are included by default in every install method. For a minimal TCP-only build: `cargo install zpl_toolchain_cli --no-default-features --features tcp`.

### Rust

```rust
use zpl_toolchain_print_client::{TcpPrinter, PrinterConfig, Printer, StatusQuery};

let config = PrinterConfig::default();
let mut printer = TcpPrinter::connect("192.168.1.55", config)?;

// Send a label
printer.send_zpl("^XA^FO50,50^A0N,30,30^FDHello World^FS^XZ")?;

// Query printer status
let status = printer.query_status()?;
println!("Paper out: {}, Paused: {}", status.paper_out, status.paused);

// Query printer info
let info = printer.query_info()?;
println!("Model: {}, DPI: {}", info.model, info.dpi);
```

### Python

```python
from zpl_toolchain import print_zpl, query_printer_status

# Send ZPL to a network printer
result = print_zpl("^XA^FO50,50^A0N,30,30^FDHello^FS^XZ", "192.168.1.55")
# result is a JSON string: '{"success": true, "bytes_sent": 36}'

# Query printer status (~HS)
status = query_printer_status("192.168.1.55")
# status is a JSON string with parsed ~HS fields
```

### Go

```go
result, err := zpltoolchain.Print("^XA^FDHello^FS^XZ", "192.168.1.100", "", true)
```

`Print()` sends ZPL over TCP (port 9100). The third argument is an optional printer profile JSON string (empty for no profile). The fourth argument enables/disables pre-print validation. `QueryStatus()` sends `~HS` and returns a parsed JSON status object.

```go
status, err := zpltoolchain.QueryStatus("192.168.1.100")
```

> Requires the C FFI shared library (`libzpl_toolchain_ffi`) — see the [Go package README](../packages/go/zpltoolchain/README.md) for setup.

### .NET / C\#

```csharp
var result = Zpl.Print("^XA^FDHello^FS^XZ", "192.168.1.100");
```

`Zpl.Print()` sends ZPL over TCP. Optional parameters for printer profile JSON, validation toggle. `Zpl.QueryStatus()` queries `~HS`:

```csharp
var status = Zpl.QueryStatus("192.168.1.100");
```

> Requires the C FFI shared library (`zpl_toolchain_ffi`) — see the [.NET package README](../packages/dotnet/ZplToolchain/README.md) for setup.

### TypeScript (Node.js)

```typescript
import { createPrinter } from '@zpl-toolchain/print';

const printer = createPrinter({ host: '192.168.1.55' });
await printer.print('^XA^FO50,50^A0N,30,30^FDHello^FS^XZ');
await printer.close();
```

---

## API Surface by Language

Not all features are available in every binding. The table below summarises what each target exposes:

| Feature | Rust (native) | Python / C FFI | Go | .NET (C#) | TypeScript (Node.js) |
|---------|---------------|----------------|-----|-----------|----------------------|
| Send ZPL | `printer.send_zpl()` | `print_zpl()` | `Print()` | `Zpl.Print()` | `printer.print()` |
| Host Status (`~HS`) | `printer.query_status()` → `HostStatus` | `query_printer_status()` → JSON | `QueryStatus()` | `Zpl.QueryStatus()` | `printer.getStatus()` → `PrinterStatus` |
| Host Identification (`~HI`) | `printer.query_info()` → `PrinterInfo` | — | — | — | `printer.query('~HI')` → raw string |
| Raw command query | `printer.query_raw()` | — | — | — | `printer.query(cmd)` → raw string |
| Batch printing | `send_batch()` / `send_batch_with_status()` | — | — | — | `printBatch()` / `printer.printBatch()` |
| Wait for completion | `wait_for_completion()` (generic) | — | — | — | `printer.waitForCompletion()` |
| USB transport | `UsbPrinter` + CLI `--printer usb` | — | — | — | — |
| Serial / BT SPP transport | `SerialPrinter` + CLI `--serial` | — | — | — | — |

> **Note:** `query_info()` is Rust-only. Batch printing is available in Rust and TypeScript. `wait_for_completion()` is available in Rust (generic, works with any `StatusQuery` implementor) and TypeScript (`TcpPrinter`). USB and serial transports are available from the CLI (`--printer usb`, `--serial`) and as Rust API. Python and C FFI expose `print_zpl()` and `query_printer_status()` over TCP. Go and .NET expose `Print()` and `QueryStatus()` over TCP via the C FFI. TypeScript has `printer.query('~HI')` for raw commands (returns the unparsed response string).
>
> **Naming conventions:** C FFI uses the `zpl_` prefix (`zpl_print()`, `zpl_query_status()`), Python uses snake_case without prefix (`print_zpl()`, `query_printer_status()`), Go uses PascalCase (`Print()`, `QueryStatus()`), .NET uses `Zpl.` prefix with PascalCase (`Zpl.Print()`, `Zpl.QueryStatus()`).

### Return Values (Python / C FFI)

**`print_zpl(zpl, printer_addr, profile_json=None, validate=True)`**

On success:
```json
{ "success": true, "bytes_sent": 44 }
```

On validation failure (when `validate=True`):
```json
{
  "success": false,
  "error": "validation_failed",
  "issues": [
    { "code": "ZPL1201", "message": "...", "severity": "error", "span": { "start": 0, "end": 4 } }
  ]
}
```

On connection/send error, a `RuntimeError` (Python) or error string (C FFI) is raised with the message.

**`query_printer_status(printer_addr)`**

Returns a JSON-serialised `HostStatus` object with 24 fields, including:
```json
{
  "paper_out": false,
  "paused": false,
  "head_up": false,
  "ribbon_out": false,
  "over_temperature": false,
  "under_temperature": false,
  "formats_in_buffer": 0,
  "labels_remaining": 0,
  "print_mode": "TearOff"
}
```

---

## CLI Reference

```
zpl print <FILES>... --printer <ADDR> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<FILES>...` | One or more ZPL files to print. |

### Required Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--printer` | `-p` | Printer address: IP, hostname, `usb`, `usb:VID:PID`, or serial port path. See [Address Formats](#address-formats). |

### Optional Flags

| Flag | Description |
|------|-------------|
| `--profile <PATH>` | Printer profile for pre-print validation (e.g., `profiles/ZD421-300dpi.json`). |
| `--tables <PATH>` | Override parser tables with custom JSON. When omitted, embedded tables are used (hidden flag). |
| `--no-lint` | Skip validation and send raw ZPL directly. |
| `--strict` | Treat warnings as errors — abort if any warnings are found. |
| `--dry-run` | Validate files and resolve the printer address, but don't actually send. |
| `--status` | Query printer status (`~HS`) after sending and display the result. |
| `--verify` | Require post-send status verification. Fails if `~HS` cannot be read or reports hard fault flags (paper/ribbon/head/temp/RAM/pause/buffer). |
| `--wait` | Poll printer status until it finishes processing all labels. |
| `--info` | Query printer info (`~HI`) before sending and display model, firmware, DPI, and memory. |
| `--timeout <SECS>` | Connection timeout in seconds (minimum 1). Write timeout scales to 6× and read to 2×. Default profile: connect=5s, write=30s, read=10s. When using `--serial` without `--timeout`, safer serial defaults are used (connect=10s, write=120s, read=30s). |
| `--wait-timeout <SECS>` | Timeout in seconds for `--wait` polling (default: 120). Requires `--wait`. |
| `--serial` | Use serial/Bluetooth SPP transport (printer address is a serial port path). |
| `--baud <RATE>` | Baud rate for serial connections (default: 9600). Requires `--serial`. |
| `--serial-flow-control <MODE>` | Serial flow control override: `none`, `software` (XON/XOFF), or `hardware` (RTS/CTS). Requires `--serial`. |
| `--serial-parity <MODE>` | Serial parity override: `none`, `even`, or `odd`. Requires `--serial`. |
| `--serial-stop-bits <MODE>` | Serial stop bit override: `one` or `two`. Requires `--serial`. |
| `--serial-data-bits <MODE>` | Serial data bit override: `seven` or `eight`. Requires `--serial`. |
| `--trace-io` | Emit serial transport hex/ASCII TX/RX dumps to stderr (diagnostics only). Requires `--serial`. |
| `--output <FORMAT>` | Output format: `pretty` or `json`. Defaults to `pretty` when stdout is a TTY, `json` when piped. Global flag. |

### Address Formats

The `--printer` flag accepts these formats:

| Format | Example | Transport |
|--------|---------|-----------|
| IP | `192.168.1.55` | TCP (port 9100) |
| IP:PORT | `192.168.1.55:9100` | TCP (explicit port) |
| Hostname | `printer01.local` | TCP (port 9100) |
| Hostname:PORT | `printer01.local:6101` | TCP (explicit port) |
| IPv6 | `::1` | TCP (port 9100) |
| IPv6:PORT | `[::1]:9100` | TCP (explicit port) |
| `usb` | `usb` | USB (first Zebra printer, VID 0x0A5F). |
| `usb:VID:PID` | `usb:0A5F:00A0` | USB (specific device). |
| Serial path | `/dev/ttyUSB0` | Serial (use with `--serial` flag). |

### Examples

```bash
# Basic print
zpl print shipping-label.zpl -p 192.168.1.55

# Validate with a specific printer profile, then print
zpl print label.zpl -p 10.0.0.42 --profile profiles/ZD421-300dpi.json

# Print multiple files and wait for completion
zpl print label1.zpl label2.zpl label3.zpl -p printer01.local --wait

# Dry run with JSON output (useful for scripting)
zpl print label.zpl -p 192.168.1.55 --dry-run --output json

# Send raw ZPL without validation, query status after
zpl print raw-label.zpl -p 192.168.1.55 --no-lint --status

# Strict mode: abort on any warning
zpl print label.zpl -p 192.168.1.55 --strict --profile profiles/ZT411-203dpi.json

# Custom timeout (30 seconds)
zpl print large-label.zpl -p 192.168.1.55 --timeout 30

# Print via USB (auto-discover first Zebra printer)
zpl print label.zpl -p usb

# Print via USB with specific VID:PID
zpl print label.zpl -p usb:0A5F:00A0

# Print via serial port
zpl print label.zpl -p /dev/ttyUSB0 --serial

# Print via Bluetooth SPP with custom baud rate
zpl print label.zpl -p /dev/rfcomm0 --serial --baud 115200

# Print via serial with explicit line settings and IO traces
zpl print label.zpl -p /dev/cu.TheBeast --serial \
  --baud 9600 \
  --serial-flow-control software \
  --serial-parity none \
  --serial-stop-bits one \
  --serial-data-bits eight \
  --trace-io
```

---

## Supported Transports

### TCP (Port 9100)

The default and most common transport. All network-connected Zebra printers (and most other industrial label printers) listen on TCP port 9100 for raw ZPL.

```bash
zpl print label.zpl --printer 192.168.1.55
zpl print label.zpl --printer 192.168.1.55:9100  # explicit port
```

```rust
use zpl_toolchain_print_client::{TcpPrinter, PrinterConfig, Printer};

let mut printer = TcpPrinter::connect("192.168.1.55", PrinterConfig::default())?;
printer.send_zpl(zpl)?;
```

Features:
- Automatic address resolution (IP, hostname, IPv4/IPv6)
- TCP_NODELAY for low-latency sends
- TCP keepalive (60s) for persistent connections
- `reconnect()` to re-establish after errors
- `wait_for_completion()` to poll until all labels are printed
- `ReconnectRetryPrinter` — retry wrapper that automatically reconnects between attempts

### USB

Direct USB connection to Zebra printers.

```bash
# Auto-discover first Zebra printer (VID 0x0A5F)
zpl print label.zpl --printer usb

# Specific device by VID:PID (hex)
zpl print label.zpl --printer usb:0A5F:00A0
```

```rust
use zpl_toolchain_print_client::{UsbPrinter, PrinterConfig, Printer};

// Auto-discover first Zebra printer (VID 0x0A5F)
let mut printer = UsbPrinter::find_zebra(PrinterConfig::default())?;
printer.send_zpl(zpl)?;

// Or find by specific VID:PID
let mut printer = UsbPrinter::find(0x0A5F, 0x00A0, PrinterConfig::default())?;

// List all USB devices
let devices = UsbPrinter::list_devices();
for (vid, pid, desc) in devices {
    println!("{:04X}:{:04X} {}", vid, pid, desc);
}
```

### Serial / Bluetooth SPP

Serial port connection for RS-232, USB-serial adapters, and Bluetooth SPP.

```bash
# Default baud rate (9600)
zpl print label.zpl --printer /dev/ttyUSB0 --serial

# Custom baud rate
zpl print label.zpl --printer /dev/ttyUSB0 --serial --baud 115200
```

```rust
use zpl_toolchain_print_client::{SerialPrinter, PrinterConfig, Printer};

// Open with default baud rate (9600)
let config = PrinterConfig::default();
let mut printer = SerialPrinter::open_default("/dev/ttyUSB0", config)?;
printer.send_zpl(zpl)?;

// Open with custom baud rate
let mut printer = SerialPrinter::open("/dev/ttyUSB0", 115200, PrinterConfig::default())?;

// List available serial ports
let ports = SerialPrinter::list_ports();
for port in ports {
    println!("{}", port);
}
```

---

## USB Setup

USB printing requires the host OS to allow user-space access to the printer's USB interface.

### Linux (udev Rules)

Create a udev rule to allow non-root access to Zebra printers:

```bash
# /etc/udev/rules.d/99-zebra-printer.rules
SUBSYSTEM=="usb", ATTR{idVendor}=="0a5f", MODE="0666", GROUP="plugdev"
```

Reload rules and replug the printer:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

**Note:** The `usblp` kernel driver may claim the printer interface. The `UsbPrinter` implementation calls `detach_and_claim_interface()` automatically, but if you encounter permission issues, you can blacklist the driver:

```bash
# /etc/modprobe.d/zebra-printer.conf
blacklist usblp
```

### Windows (Zadig / WinUSB)

Windows requires a WinUSB-compatible driver for user-space USB access:

1. Download [Zadig](https://zadig.akeo.ie/)
2. Connect the Zebra printer via USB
3. In Zadig, select the printer device
4. Choose **WinUSB** as the replacement driver
5. Click **Replace Driver**

**Caution:** This replaces the default Windows printer driver. The printer will no longer appear in Windows printer settings. Use Zadig to restore the original driver if needed.

### macOS (Entitlements)

macOS sandboxing requires the `com.apple.security.device.usb` entitlement for USB access. When distributing a signed app, add to your entitlements plist:

```xml
<key>com.apple.security.device.usb</key>
<true/>
```

For development (unsigned builds), USB access typically works without additional configuration.

---

## Bluetooth Setup

Zebra recommends Bluetooth SPP (Serial Port Profile) for printing. BLE (Bluetooth Low Energy) is designed for status monitoring, not bulk data transfer.

### Pairing

Pair the printer at the OS level before using the serial transport:

1. **Put the printer in discoverable mode** (refer to your printer's manual — typically a button sequence or `^JUS` command)
2. **Pair via OS Bluetooth settings** — the printer will appear as a Bluetooth device
3. **Note the serial port path** that the OS assigns:
   - **macOS**: `/dev/tty.ZebraPrinter-SerialPort` (name varies by model)
   - **Linux**: `/dev/rfcomm0` (may require `rfcomm bind`)
   - **Windows**: `COM5` (check Device Manager → Ports)

### Printing via Bluetooth

Once paired, use the serial transport with the assigned port:

```bash
# macOS
zpl print label.zpl --printer /dev/tty.ZebraPrinter-SerialPort --serial

# Linux
zpl print label.zpl --printer /dev/rfcomm0 --serial

# Windows
zpl print label.zpl --printer COM5 --serial
```

> **Important:** With `--serial`, pass the OS-assigned serial port path.
> Do **not** pass a Bluetooth MAC address (for example `60:95:32:1C:7A:10`).
> Also note that serial/Bluetooth send success means bytes were written to the OS serial device. Use
> `--verify` (or `--status` / `--wait`) in the CLI when you need verification that the printer processed the label.

If the CLI reports `sent:` but no physical label prints:

1. Confirm your OS created a serial port for the paired printer (SPP/RFCOMM), and use that path (`/dev/cu.*`, `/dev/tty*`, `/dev/rfcomm*`, `COM*`).
2. Send a tiny probe label first (to separate transport/setup issues from large-label throughput issues):
   ```bash
   printf '^XA^FO20,20^A0N,40,40^FDHELLO^FS^XZ' > /tmp/zpl_probe.zpl
   zpl print /tmp/zpl_probe.zpl --printer /dev/cu.<printer-port> --serial --status
   ```
3. If tiny labels print but large labels fail, increase `--timeout` (serial/Bluetooth can be slower than TCP, especially with `^GF` graphics).
4. Verify the printer and host are using the same serial settings (`--baud` and printer communication settings).

```rust
use zpl_toolchain_print_client::{SerialPrinter, PrinterConfig, Printer};

let config = PrinterConfig::default();
let mut printer = SerialPrinter::open_default("/dev/tty.ZebraPrinter-SerialPort", config)?;
printer.send_zpl(zpl)?;
```

---

## TypeScript Package

The `@zpl-toolchain/print` package provides TypeScript-native printing, separate from the WASM `@zpl-toolchain/core` package.

### Installation

```bash
npm install @zpl-toolchain/print
```

### Node.js

```typescript
import { createPrinter } from '@zpl-toolchain/print';

// Create a persistent connection
const printer = createPrinter({ host: '192.168.1.55' });

// Send a label
await printer.print('^XA^FO50,50^A0N,30,30^FDHello^FS^XZ');

// Query status (~HS) — parsed into a structured object
const status = await printer.getStatus();
console.log(`Paper out: ${status.paperOut}, Paused: ${status.paused}`);

// Send a raw command and get the raw response string
const info = await printer.query('~HI');
console.log(`Printer info: ${info}`);

await printer.close();
```

Uses `node:net` for TCP sockets — pure TypeScript, no native dependencies.

You can also use the `TcpPrinter` constructor directly or the one-shot `print()` function:

```typescript
import { TcpPrinter, print } from '@zpl-toolchain/print';

// Constructor form (equivalent to createPrinter)
const printer = new TcpPrinter({ host: '192.168.1.55' });

// One-shot print (opens connection, sends, closes)
const result = await print('^XA^FDHello^FS^XZ', { host: '192.168.1.55' });
console.log(`Sent ${result.bytesWritten} bytes in ${result.duration}ms`);
```

> **Concurrency note:** `TcpPrinter` serializes concurrent `print()` calls through an internal write queue, so it is safe to call `print()` from multiple async contexts simultaneously — writes will never interleave. `close()` drains the queue before tearing down the socket.

#### Validated Print

`printValidated()` validates ZPL before sending (requires `@zpl-toolchain/core` as an optional peer dependency). Supports printer profile-aware validation and strict mode:

```typescript
import { printValidated } from '@zpl-toolchain/print';
import { readFileSync } from 'node:fs';

// Basic: validate then print (skips validation silently if @zpl-toolchain/core is not installed)
await printValidated('^XA^FDHello^FS^XZ', { host: '192.168.1.55' });

// With printer profile for hardware-aware validation
const profile = readFileSync('profiles/ZD421.json', 'utf-8');
await printValidated('^XA^FDHello^FS^XZ', { host: '192.168.1.55' }, {
  profileJson: profile,
  strict: true,  // treat warnings as errors
});
```

#### Batch Printing

Send multiple labels with progress tracking using `printBatch()` (standalone) or `TcpPrinter.printBatch()`:

```typescript
import { printBatch, TcpPrinter } from '@zpl-toolchain/print';

const labels = ["^XA^FDLabel 1^FS^XZ", "^XA^FDLabel 2^FS^XZ"];
const result = await printBatch(labels, { host: "192.168.1.100" });

// With progress tracking
const printer = new TcpPrinter({ host: "192.168.1.100" });
await printer.printBatch(labels, {}, (p) => {
  console.log(`${p.sent}/${p.total}`);
});
await printer.waitForCompletion();
await printer.close();
```

The standalone `printBatch()` opens a connection, sends all labels, and closes. `TcpPrinter.printBatch()` uses a persistent connection with optional `BatchOptions` (e.g., `statusInterval` for `~HS` polling between labels) and a progress callback (`BatchProgress`).

Related types: `BatchOptions`, `BatchProgress`, `BatchResult`.

### Browser (via Proxy)

Browsers cannot open raw TCP sockets. Two options:

**Option 1: Local proxy server**

Run a small Node.js proxy that bridges HTTP and WebSocket to TCP:

```typescript
import { createPrintProxy } from '@zpl-toolchain/print/proxy';

// Start a local HTTP proxy server bridging browser requests to TCP printers.
// IMPORTANT: allowedPrinters is required — the proxy rejects all requests
// when no printers are explicitly allowed (SSRF protection).
const { server, close } = await createPrintProxy({
  port: 3001,
  allowedPrinters: ['192.168.1.*', 'printer-*'],  // glob patterns; bare '*' matches all
  allowedPorts: [9100],        // default; empty array = deny all
  maxConnections: 50,          // default; limit concurrent WebSocket clients
});
// Browsers send POST requests to http://localhost:3001/print
```

**Option 2: Zebra Browser Print**

If the user has [Zebra Browser Print](https://www.zebra.com/us/en/support-downloads/software/printer-software/browser-print.html) installed, the SDK provides a local HTTP API:

```typescript
import { ZebraBrowserPrint } from '@zpl-toolchain/print/browser';

const zbp = new ZebraBrowserPrint();
const devices = await zbp.discover();
if (devices.length > 0) {
  await zbp.print(devices[0], '^XA^FDHello^FS^XZ');
}
```

#### WebSocket

The proxy also accepts WebSocket connections on the same port for persistent, bidirectional communication:

```typescript
const ws = new WebSocket("ws://localhost:3001");
ws.onopen = () => {
  ws.send(JSON.stringify({
    type: "print",
    printer: "192.168.1.55",
    zpl: "^XA^FDHello^FS^XZ",
  }));
};
ws.onmessage = (event) => {
  const result = JSON.parse(event.data);
  console.log(result.ok ? "Printed!" : result.error);
};
```

Query status via WebSocket:

```typescript
ws.send(JSON.stringify({ type: "status", printer: "192.168.1.55" }));
```

The WebSocket endpoint shares the same security configuration as the HTTP endpoints:

- **Origin validation** — WebSocket upgrades are checked against the `cors` setting
- **Payload limits** — `maxPayloadSize` applies to both HTTP and WebSocket messages
- **Keepalive** — Idle connections are automatically terminated after 30 seconds of inactivity
- **SSRF protection** — `allowedPrinters` is enforced for all WebSocket messages
- **Port restriction** — `allowedPorts` (default `[9100]`) restricts destination ports; empty array denies all
- **Connection limits** — `maxConnections` (default `50`) caps concurrent HTTP and WebSocket connections; new connections are rejected with 503 when the limit is reached
- **Body-read timeout** — HTTP request bodies must be received within 30 seconds (mitigates slow-loris attacks)
- **Error sanitization** — TCP error details (internal IPs, port numbers, errno codes) are not forwarded to clients; generic error messages are returned instead

---

## Status Querying

### Host Status (`~HS`)

Query comprehensive printer status:

```rust
let status = printer.query_status()?;

// Check error conditions
if status.paper_out { println!("Paper out!"); }
if status.head_up { println!("Print head is open!"); }
if status.ribbon_out { println!("Ribbon out!"); }
if status.paused { println!("Printer is paused"); }

// Check print progress
println!("Formats in buffer: {}", status.formats_in_buffer);
println!("Labels remaining: {}", status.labels_remaining);
println!("Print mode: {:?}", status.print_mode);

// Temperature
if status.over_temperature { println!("WARNING: Over temperature!"); }
if status.under_temperature { println!("WARNING: Under temperature!"); }
```

### Host Identification (`~HI`)

Query printer identity:

```rust
let info = printer.query_info()?;
println!("Model: {}", info.model);       // e.g., "ZTC ZD421-300dpi ZPL"
println!("Firmware: {}", info.firmware);  // e.g., "V85.20.19"
println!("DPI: {}", info.dpi);           // e.g., 300
println!("Memory: {} KB", info.memory_kb);
```

### CLI Status

```bash
# Query status after printing
zpl print label.zpl -p 192.168.1.55 --status

# JSON output for scripting
zpl print label.zpl -p 192.168.1.55 --status --output json
```

---

## Batch Printing

Send multiple labels with optional progress tracking:

```rust
use zpl_toolchain_print_client::{send_batch, send_batch_with_status, BatchOptions};
use std::ops::ControlFlow;

let labels = vec![
    "^XA^FO50,50^A0N,30,30^FDLabel 1^FS^XZ",
    "^XA^FO50,50^A0N,30,30^FDLabel 2^FS^XZ",
    "^XA^FO50,50^A0N,30,30^FDLabel 3^FS^XZ",
];

use std::num::NonZeroUsize;

let opts = BatchOptions {
    status_interval: Some(NonZeroUsize::new(10).unwrap()),
};

// Basic batch (no status polling)
let result = send_batch(&mut printer, &labels, |progress| {
    println!("Sent {}/{}", progress.sent, progress.total);
    ControlFlow::Continue(())
})?;

// Batch with status polling (requires StatusQuery)
let result = send_batch_with_status(&mut printer, &labels, &opts, |progress| {
    if let Some(ref status) = progress.status {
        if status.paper_out {
            println!("Paper out! Aborting.");
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
})?;

println!("Sent {}/{} labels", result.sent, result.total);
```

### Wait for Completion

After sending labels, wait for the printer to finish. The standalone `wait_for_completion()` function works with any transport that implements `StatusQuery`:

```rust
use zpl_toolchain_print_client::wait_for_completion;
use std::time::Duration;

// Wait up to 2 minutes, polling every 500ms
wait_for_completion(
    &mut printer,                // any &mut impl StatusQuery
    Duration::from_millis(500),  // poll interval
    Duration::from_secs(120),    // timeout
)?;
```

---

## Troubleshooting

### Connection Refused

```
error: connection refused: 192.168.1.55:9100
```

- Verify the printer is powered on and connected to the network
- Verify the IP address (`ping 192.168.1.55`)
- Verify port 9100 is open (`nc -z 192.168.1.55 9100`)
- Check if a firewall is blocking port 9100
- Print a network configuration label from the printer (usually via button sequence)

### Connection Timeout

```
error: connection timed out: 192.168.1.55:9100 (5s)
```

- The printer may be on a different subnet or VLAN
- Try increasing the timeout: `--timeout 30`
- Check network routing between your machine and the printer

### Write Failed / Broken Pipe

```
error: write failed: Broken pipe
```

- The printer disconnected mid-send. This can happen if:
  - The label is very large (embedded `^GF` graphics) and exceeds the write timeout
  - The printer's receive buffer is full (try adding a delay between labels)
  - Network instability
- Increase write timeout: `--timeout 60`
- Use `ReconnectRetryPrinter` to automatically reconnect and retry:

```rust
use zpl_toolchain_print_client::{
    TcpPrinter, ReconnectRetryPrinter, Printer, RetryConfig, PrinterConfig,
};

let tcp = TcpPrinter::connect("192.168.1.100", PrinterConfig::default())?;
let mut printer = ReconnectRetryPrinter::new(tcp, RetryConfig::default());
// If the connection drops, ReconnectRetryPrinter will reconnect and retry
printer.send_zpl("^XA^FDHello^FS^XZ")?;
```

### USB Device Not Found

```
error: USB device not found
```

- Verify the printer is connected via USB and powered on
- **Linux**: Check udev rules (see [USB Setup](#linux-udev-rules))
- **Windows**: Ensure WinUSB driver is installed (see [USB Setup](#windows-zadig--winusb))
- **macOS**: Check USB permissions
- List devices to verify: `UsbPrinter::list_devices()`

### Serial Port Error

```
error: serial port error: No such file or directory
```

- Verify the serial port path exists (e.g., `ls /dev/ttyUSB*`)
- Check permissions: `sudo chmod 666 /dev/ttyUSB0` or add user to `dialout` group
- For Bluetooth: ensure the printer is paired and the SPP service is active

### Read Timeout (Status Query)

```
warning: failed to query printer status: read timed out
```

- The printer may be busy processing a label. Status queries are delayed while printing.
- Some non-Zebra printers don't respond to `~HS` / `~HI` queries
- This is non-fatal — the label was still sent successfully

### Bluetooth/Serial Forensics (bytes sent, no print)

If serial/Bluetooth reports `sent:` but no label prints, and `--status`/`--wait` repeatedly time out, use this checklist to isolate transport vs printer behavior:

1. **Confirm the file is plain ZPL** (not RTF/rich text). Rich text wrappers (`{\rtf1...}`) cause enum and stray-content diagnostics.
2. **Print config over TCP** (`~WC`) and verify active serial settings (baud, data bits, parity, handshake/protocol) on the printer.
3. **Apply serial settings over TCP** and persist:
   ```zpl
   ^SC9600,8,N,1,X,N
   ^JUS
   ```
4. **Use a tiny probe label** first (to avoid conflating throughput issues with transport setup):
   ```bash
   printf '^XA^FO20,20^A0N,40,40^FDHELLO^FS^XZ' > /tmp/zpl_probe.zpl
   zpl print /tmp/zpl_probe.zpl --printer /dev/cu.<printer-port> --serial --baud 9600 --timeout 5
   ```
5. **If write succeeds but reads are always empty**, test with an external serial probe (for example `pyserial`) to confirm whether the OS serial endpoint is effectively write-only or non-responsive for this printer.
6. **Check for port ownership conflicts** (`Resource busy`) before testing (`lsof /dev/cu.* /dev/tty.*`).
7. **If all flow-control modes still show zero read bytes and no print**, treat it as endpoint/profile mismatch (OS Bluetooth serial service path not carrying printer data), not a parser/validator issue.

Known behavior: `sent:` on serial/Bluetooth means bytes were accepted by the OS device file, not necessarily that the printer processed them. Use status queries when available, and verify with physical output.

### Serial Probe Command

Use `zpl serial-probe` to quickly classify whether a serial/Bluetooth path is usable for bidirectional Zebra communication:

```bash
# Baseline probe
zpl serial-probe /dev/cu.TheBeast

# Probe with explicit serial settings and wire traces
zpl serial-probe /dev/cu.TheBeast \
  --baud 9600 \
  --serial-flow-control software \
  --serial-parity none \
  --serial-stop-bits one \
  --serial-data-bits eight \
  --send-test-label \
  --trace-io

# JSON output for scripts/incident reports
zpl serial-probe /dev/cu.TheBeast --output json
```

`serial-probe` checks:
- open/connect viability on the selected serial endpoint,
- `~HS` host status read path,
- `~HI` host identification read path,
- optional tiny test-label write path.

Diagnosis categories currently emitted:
- `bidirectional_serial_ok` — status/info reads are working.
- `write_path_only_or_response_blocked` — writes succeed but reads fail (common with wrong channel/profile).
- `serial_transport_not_viable_with_current_settings` — connect/read/write path failed with current settings.

### Validation Errors Blocking Print

```
error: aborting print due to validation errors
```

- Fix the ZPL errors reported by the validator, or
- Use `--no-lint` to skip validation and send raw ZPL

---

## Preflight Checks

The validator includes preflight diagnostics that catch graphics-related issues before printing. These are especially useful when a printer profile is provided:

| Code | Name | Description |
|------|------|-------------|
| **ZPL2308** | Graphics bounds | A `^GF` graphic field exceeds the printable area (`^PW`/`^LL` or profile page bounds). The graphic will be clipped or may cause a printer error. |
| **ZPL2309** | Graphics memory | The total memory required by `^GF` graphic fields in a label exceeds the printer's available memory (from profile `memory.ram_kb`). Large graphics may fail to print or cause buffer overflows. |
| **ZPL2310** | Missing explicit dimensions | The label uses profile-provided dimensions but does not contain explicit `^PW` or `^LL` commands. Adding explicit dimension commands makes the label portable across printers. |

These diagnostics are emitted during `lint` / `validate` when a printer profile is active:

```bash
# Preflight with a printer profile
zpl lint label-with-graphics.zpl --profile profiles/ZD421-300dpi.json
```

```rust
use zpl_toolchain_core::{parse_with_tables, validate_with_profile};

let parsed = parse_with_tables(zpl, Some(&tables));
let result = validate_with_profile(&parsed.ast, &tables, Some(&profile));
for diag in &result.issues {
    if diag.id.starts_with("ZPL23") {
        println!("Preflight: {}", diag.message);
    }
}
```

---

## Non-Zebra Printers

TCP port 9100 (JetDirect / RAW) is a de facto industry standard. The following brands accept ZPL over TCP 9100:

- **SATO** (CLNX series with ZPL emulation)
- **Honeywell** (PX/PM series with ZPL emulation)
- **TSC** (with ZPL emulation mode)
- **Godex** (with ZPL emulation mode)
- **Citizen** (CL-S series with ZPL emulation)
- **CAB** (with ZPL emulation mode)

**Caveats:**
- Status queries (`~HS`, `~HI`) are Zebra-specific and may not be supported
- Some ZPL emulations don't support all 223 commands
- USB auto-discovery (`find_zebra()`) only matches Zebra's vendor ID — use `find(vid, pid)` for other brands
- Printer profiles are Zebra-specific; validation may produce false positives for non-Zebra hardware
