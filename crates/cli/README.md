# zpl — ZPL Toolchain CLI

Command-line interface for parsing, validating, formatting, and printing ZPL II label code.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Installation

```bash
# From crates.io (all transports: TCP, USB, serial/Bluetooth)
cargo install zpl_toolchain_cli
# Or use cargo-binstall for a pre-built binary (no compile wait):
cargo binstall zpl_toolchain_cli
```

Pre-built binaries with all transports are available from [GitHub Releases](https://github.com/trevordcampbell/zpl-toolchain/releases).

> For a minimal TCP-only build: `cargo install zpl_toolchain_cli --no-default-features --features tcp`.

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

# Print via USB
zpl print label.zpl -p usb

# Print via serial/Bluetooth
zpl print label.zpl -p /dev/rfcomm0 --serial --baud 115200

# Explain a diagnostic code
zpl explain ZPL1201
```

## Global Options

| Flag | Description |
|------|-------------|
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
| `--verify` | Require post-send status verification; fail on status-read failure or hard fault flags. With `--wait`, status is re-queried after completion. |
| `--info` | Query `~HI` printer info before sending |
| `--wait` | Wait for printer to finish all labels |
| `--wait-timeout <SECS>` | Timeout for `--wait` polling (default: 120s; requires `--wait`) |
| `--timeout <SECS>` | Connection timeout in seconds, minimum 1 (default: 5). Also sets write timeout to 6× and read timeout to 2× this value |
| `--serial` | Use serial/Bluetooth SPP transport |
| `--baud <RATE>` | Baud rate for serial connections (default: 9600) |

## Printer Address Formats

| Format | Transport | Example |
|--------|-----------|---------|
| IP or hostname | TCP (port 9100) | `192.168.1.55`, `printer.local` |
| IP:port | TCP (custom port) | `192.168.1.55:6101` |
| `usb` | USB (auto-discover Zebra) | `usb` |
| `usb:VID:PID` | USB (specific device) | `usb:0A5F:0100` |
| Serial path | Serial/BT SPP (with `--serial`) | `/dev/ttyUSB0`, `COM3` |

> **Note:** Serial/Bluetooth addresses require the `--serial` flag. Without it, the CLI assumes TCP.
> There is no separate `--usb` flag: USB is selected with `-p usb` or `-p usb:VID:PID`.
> With `--serial`, pass the OS serial port path (`/dev/cu.*`, `/dev/tty*`, `COM*`, `/dev/rfcomm*`) — not a Bluetooth MAC address.
> Serial/Bluetooth transport is write-only at send time; use `--verify` (or `--status` / `--wait`) when you need stronger verification that the printer processed the job.

### Minimal Feature Builds

Default installs include all transports (`tcp`, `usb`, `serial`). For minimal deployments:

```bash
# TCP only
cargo install zpl_toolchain_cli --no-default-features --features tcp

# USB only
cargo install zpl_toolchain_cli --no-default-features --features usb

# Serial/Bluetooth only
cargo install zpl_toolchain_cli --no-default-features --features serial

# TCP + USB
cargo install zpl_toolchain_cli --no-default-features --features "tcp usb"

# TCP + Serial/Bluetooth
cargo install zpl_toolchain_cli --no-default-features --features "tcp serial"
```

## Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| `USB device not found` | Printer not connected or powered off | Check cable/power; on Linux, add [udev rules](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/PRINT_CLIENT.md#linux-udev-rules) |
| `failed to open device: Access denied` | Insufficient USB permissions | Linux: add udev rule or run with `sudo`; macOS: grant USB access |
| `serial port error: Permission denied` | Insufficient serial port permissions | Add user to `dialout` group (`sudo usermod -aG dialout $USER`) or `chmod 666` the device |
| DNS error on a serial path | Missing `--serial` flag | Add `--serial`: `zpl print label.zpl -p /dev/ttyUSB0 --serial` |
| `sent:` appears but no physical label prints (serial/Bluetooth) | Wrong serial endpoint (BLE pairing without SPP) or serial throughput/settings mismatch | Use an OS serial port path (`/dev/cu.*`, `/dev/tty*`, `/dev/rfcomm*`, `COM*`), test with a tiny probe label first, then use `--verify` (or `--status`/`--wait`) and increase `--timeout` if needed |
| `no parser tables available` | Binary built without embedded tables | Use `cargo install zpl_toolchain_cli` or download from [Releases](https://github.com/trevordcampbell/zpl-toolchain/releases) |

See the full [Print Client Guide](https://github.com/trevordcampbell/zpl-toolchain/blob/main/docs/PRINT_CLIENT.md) for detailed transport setup and troubleshooting.

## License

Dual-licensed under MIT or Apache-2.0.
