# zpl_toolchain_print_client

Send ZPL to Zebra and ZPL-compatible label printers over TCP, USB, or serial/Bluetooth SPP.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Features

- **Three transports**: TCP (port 9100, default), USB (`nusb`, feature-gated), Serial/BT SPP (`serialport`, feature-gated)
- **Split trait design**: `Printer` (send-only) + `StatusQuery` (bidirectional)
- **Status parsing**: `~HS` → `HostStatus` (24 fields), `~HI` → `PrinterInfo`
- **Batch printing**: `send_batch()` / `send_batch_with_status()` with progress callbacks and `ControlFlow` abort; `wait_for_completion()` generic polling
- **Retry with backoff**: `RetryPrinter<P>` wrapper with exponential backoff and jitter; `ReconnectRetryPrinter<P>` for automatic reconnection between retry attempts
- **Semver-safe**: `#[non_exhaustive]` on all public structs and enums
- **Synchronous**: No async runtime required — uses `std::net` and `std::io`

## Quick Start

```rust
use zpl_toolchain_print_client::{TcpPrinter, PrinterConfig, Printer, StatusQuery};

let config = PrinterConfig::default();
let mut printer = TcpPrinter::connect("192.168.1.55", config)?;

// Send a label
printer.send_zpl("^XA^FO50,50^A0N,30,30^FDHello World^FS^XZ")?;

// Query printer status
let status = printer.query_status()?;
println!("Paper out: {}, Paused: {}", status.paper_out, status.paused);
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `tcp` | Yes | TCP transport via `socket2` |
| `usb` | No | USB transport via `nusb` |
| `serial` | No | Serial/Bluetooth SPP via `serialport` |

```toml
[dependencies]
zpl_toolchain_print_client = "0.1"                          # TCP only
zpl_toolchain_print_client = { version = "0.1", features = ["usb", "serial"] }  # All transports
```

## Documentation

See the [Print Client Guide](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/PRINT_CLIENT.md) for comprehensive usage, CLI integration, and troubleshooting.

## License

Dual-licensed under MIT or Apache-2.0.
