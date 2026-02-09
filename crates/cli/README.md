# zpl — ZPL Toolchain CLI

Command-line interface for parsing, validating, formatting, and printing ZPL II label code.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Installation

```bash
# From crates.io (TCP printing included by default)
cargo install zpl_toolchain_cli

# With USB and serial/Bluetooth support
cargo install zpl_toolchain_cli --features usb,serial
```

Pre-built binaries with all transports are available from [GitHub Releases](https://github.com/trevordcampbell/zpl-toolchain/releases).

## Commands

```bash
# Parse and inspect ZPL
zpl parse label.zpl

# Validate ZPL (with optional printer profile)
zpl lint label.zpl --profile profiles/zebra-generic-203.json

# Check syntax only
zpl syntax-check label.zpl

# Format ZPL
zpl format label.zpl --write --indent label

# Print ZPL to a network printer
zpl print label.zpl -p 192.168.1.55

# Print with validation and status query
zpl print label.zpl -p 192.168.1.55 --profile profiles/zebra-generic-203.json --status --info

# Print via USB (requires --features usb or release binary)
zpl print label.zpl -p usb

# Print via serial/Bluetooth (requires --features serial or release binary)
zpl print label.zpl -p /dev/rfcomm0 --serial --baud 115200

# Explain a diagnostic code
zpl explain ZPL1201

# Check spec coverage
zpl coverage --coverage generated/coverage.json
```

## Global Options

| Flag | Description |
|------|-------------|
| `--tables <PATH>` | Path to `parser_tables.json` (default: embedded at compile time) |
| `--output pretty\|json` | Output format (default: auto-detect TTY) |

## Print Command Flags

| Flag | Description |
|------|-------------|
| `-p, --printer <ADDR>` | Printer address: IP, hostname, `usb`, `usb:VID:PID`, or serial path |
| `--profile <PATH>` | Printer profile JSON for pre-print validation |
| `--no-lint` | Skip validation before printing |
| `--strict` | Treat warnings as errors during validation |
| `--dry-run` | Validate and resolve address without sending |
| `--status` | Query `~HS` host status after printing |
| `--info` | Query `~HI` printer info after printing |
| `--wait [TIMEOUT]` | Wait for print completion (default: 30s) |
| `--timeout <MS>` | Connection timeout in milliseconds (default: 5000) |
| `--serial` | Use serial/Bluetooth SPP transport (requires `--features serial`) |
| `--baud <RATE>` | Baud rate for serial connections (default: 9600) |

## Printer Address Formats

| Format | Transport | Example |
|--------|-----------|---------|
| IP or hostname | TCP (port 9100) | `192.168.1.55`, `printer.local` |
| IP:port | TCP (custom port) | `192.168.1.55:6101` |
| `usb` | USB (auto-discover Zebra) | `usb` |
| `usb:VID:PID` | USB (specific device) | `usb:0A5F:0100` |
| Serial path | Serial/BT SPP | `/dev/ttyUSB0`, `COM3` |

## License

Dual-licensed under MIT or Apache-2.0.
