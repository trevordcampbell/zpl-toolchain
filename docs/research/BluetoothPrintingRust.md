# Bluetooth Printing from Rust — Research (Feb 2026)

## Executive Summary

**Recommendation: Defer native Bluetooth to post-v1. Use TCP 9100 and USB as v1 transports. Offer the `serialport` crate as an escape hatch for BT SPP via OS-paired virtual serial ports.**

Bluetooth printing from Rust is technically possible but suffers from severe platform fragmentation — especially for Bluetooth Classic (SPP/RFCOMM), which is what Zebra recommends for printing. There is no single cross-platform crate that covers BT Classic on Linux + macOS + Windows. BLE printing is more tractable via `btleplug` but is slow (2–4 KB/s) and Zebra themselves advise it only for "light-duty" use.

---

## 1. Zebra Printer Bluetooth Architecture

Zebra printers expose **two** Bluetooth transports:

### Bluetooth Classic (SPP/RFCOMM) — Preferred for Printing
- Uses Serial Port Profile (SPP) over RFCOMM
- Behaves like an RS-232 serial connection
- Full bandwidth (~2–3 Mbit/s practical throughput)
- Zebra **recommends SPP for printing** workloads

### Bluetooth Low Energy (BLE/GATT) — Secondary
- Uses Zebra's proprietary **"Zebra Bluetooth LE Parser Service"**
- Service UUID: `38eb4a80-c570-11e3-9507-0002a5d5c51b`
- "To Printer Data" characteristic: `38eb4a82-c570-11e3-9507-0002a5d5c51b` (write ZPL here)
- "From Printer Data" characteristic: `38eb4a81-c570-11e3-9507-0002a5d5c51b` (receive responses via indications)
- Data must be chunked to min(ATT MTU, 512 bytes)
- Write methods:
  - `writeWithResponse`: ~2 KB/s (reliable, ACK'd)
  - `writeWithoutResponse`: ~4.2 KB/s (faster, risk of packet loss; not supported on all models e.g., ZD421)
- Zebra's own guidance: *"BLE is recommended for printer configuration. Use SPP for printing."*
- A 50 KB label would take ~12–25 seconds over BLE vs <1 second over SPP

---

## 2. Rust Crate Landscape

### 2.1 `bluer` — BlueZ Bindings (Linux Only)

| Property | Value |
|----------|-------|
| Version | 0.17.4 (June 2025) |
| Repository | `bluez/bluer` (official BlueZ project) |
| Platform | **Linux only** (requires BlueZ D-Bus) |
| License | BSD-2-Clause |
| Documentation | 99.98% coverage |
| Downloads | Active, well-maintained |

**Capabilities:**
- Full RFCOMM support via `bluer::rfcomm` module (feature-gated)
- `Listener` / `Stream` API (Tokio-compatible async)
- Profile-based connections with SDP record discovery
- Also supports BLE GATT, L2CAP
- Companion `bluer-tools` crate includes `rfcat` (netcat for RFCOMM)

**Assessment:** Excellent for Linux. The best RFCOMM option on that platform. Async-first, well-documented, officially maintained by the BlueZ project. But Linux-only — no path to macOS/Windows.

### 2.2 `btleplug` — Cross-Platform BLE

| Property | Value |
|----------|-------|
| Version | 0.11.8 (April 2025) |
| Repository | `deviceplug/btleplug` |
| Platforms | **Windows 10+, macOS, Linux, iOS, Android** |
| License | MIT / Apache-2.0 |
| GitHub Stars | ~1,100 |
| Downloads | ~21,000/month |

**Capabilities:**
- BLE central/host mode (connect to peripherals)
- GATT service discovery, read, write, subscribe (notifications/indications)
- Write characteristic supported on all platforms ✓
- Async (Tokio-based)

**Limitations:**
- **BLE only** — does NOT support Bluetooth Classic/RFCOMM at all
- No MTU retrieval (not implemented on any platform)
- No automatic chunking/fragmentation — you must implement your own
- No connection interval control

**Assessment:** The most mature cross-platform BLE crate in the Rust ecosystem. Sending ZPL via GATT write to Zebra printers is feasible — you'd discover the Zebra Parser Service UUID, then chunk-write to the "To Printer Data" characteristic. But throughput is limited to 2–4 KB/s, and you need manual chunking logic. Good enough for small labels, not practical for large print jobs.

### 2.3 `bluetooth-serial-port` — BT Classic RFCOMM

| Property | Value |
|----------|-------|
| Version | 0.6.0 |
| Repository | `Dushistov/bluetooth-serial-port` |
| Platforms | **Linux only** (despite docs suggesting Windows) |
| GitHub Stars | 11 |
| License | MIT |
| Dependencies | mio 0.6 (outdated), nix 0.13 (outdated) |

**Assessment:** Effectively **abandoned/unmaintained**. Uses very old dependency versions (mio 0.6 is from 2019, nix 0.13 likewise). README still says `0.5.1` in the Cargo.toml example. Linux-only in practice. Not suitable for production use. Only 11 GitHub stars.

### 2.4 `windows` Crate — Windows Bluetooth APIs

| Property | Value |
|----------|-------|
| Version | 0.58+ (actively maintained by Microsoft) |
| Namespace | `windows::Devices::Bluetooth::Rfcomm` |

**Capabilities:**
- Full WinRT Bluetooth API access
- `BluetoothDevice` — enumerate, discover, connect
- `RfcommDeviceService` — RFCOMM service discovery and connection
- `GetRfcommServicesAsync()` — async service enumeration
- Also has Win32-level access via `windows::Win32::Devices::Bluetooth`

**Assessment:** The `windows` crate gives complete access to Windows Bluetooth RFCOMM APIs, but you'd be writing Windows-specific code. This is the right approach for a Windows-only BT Classic implementation, but it doesn't help cross-platform.

### 2.5 macOS — CoreBluetooth / IOBluetooth

| Crate | Version | Notes |
|-------|---------|-------|
| `objc2-core-bluetooth` | 0.3.2 | Low-level bindings to CoreBluetooth framework |
| `corebluetooth` | 0.1.0 (July 2025) | Higher-level wrapper, very new |
| `core_bluetooth` | 0.1.0 | Older, less maintained |

**Critical Limitation:** Apple's **CoreBluetooth framework only supports BLE, not Bluetooth Classic**. For BT Classic RFCOMM on macOS, you need `IOBluetooth.framework`, which:
- Is a legacy Objective-C framework
- Has no modern Rust bindings
- Requires `objc2` FFI to call into it manually
- Apple has been deprioritizing it in favor of BLE

**Assessment:** macOS is the hardest platform for BT Classic from Rust. No usable crate exists. You'd have to write raw Objective-C FFI bindings to IOBluetooth, which is fragile and poorly documented.

### 2.6 `serialport` — Cross-Platform Serial Ports

| Property | Value |
|----------|-------|
| Version | 4.8.1 |
| Platforms | **Windows, macOS, Linux** |
| License | MPL-2.0 |
| Maturity | Very mature, widely used |

**Capabilities:**
- Enumerate serial ports (`available_ports()`)
- Open, configure, read, write serial ports
- Windows: COM ports; macOS: `/dev/cu.*`; Linux: `/dev/tty*`
- Blocking I/O (async via `tokio-serial` / `mio-serial`)

**BT SPP Virtual Serial Port Strategy:**
When a Bluetooth SPP device is paired at the OS level, the OS creates a virtual serial port:
- **Windows**: `COM3`, `COM4`, etc. (visible in Device Manager)
- **Linux**: `/dev/rfcomm0`, `/dev/rfcomm1` (via `rfcomm bind`)
- **macOS**: `/dev/cu.PrinterName-SerialPort` (via Bluetooth preferences)

The `serialport` crate can open these virtual serial ports and write ZPL data directly — the OS handles the Bluetooth transport transparently. This is the simplest path and is **fully cross-platform**.

**Caveats:**
- Requires the user to pair the printer at the OS level first (outside your app)
- `serialport` can enumerate ports but can't distinguish BT virtual ports from USB/physical serial ports without heuristics
- Connection management (reconnection, error handling) is your responsibility
- No device discovery — the user must pair the printer before your app can see it

---

## 3. Platform Support Matrix

| Capability | Linux | macOS | Windows |
|------------|-------|-------|---------|
| **BT Classic RFCOMM (native)** | `bluer` ✅ | No crate ❌ (needs IOBluetooth FFI) | `windows` crate ✅ |
| **BLE GATT write** | `btleplug` ✅ | `btleplug` ✅ | `btleplug` ✅ |
| **BT SPP via virtual serial port** | `serialport` ✅ (after `rfcomm bind`) | `serialport` ✅ (auto-created on pair) | `serialport` ✅ (auto-created on pair) |
| **Device discovery** | `bluer` (BT+BLE), `btleplug` (BLE) | `btleplug` (BLE only) | `windows` (BT+BLE), `btleplug` (BLE) |
| **Cross-platform crate** | — | — | `btleplug` (BLE only) |

### Cross-Platform BT Classic: Does It Exist?

**No.** There is no single Rust crate that provides Bluetooth Classic (RFCOMM/SPP) across Linux, macOS, and Windows. The `bluetooth-serial-port` crate claims cross-platform support but is effectively Linux-only and unmaintained. Building a cross-platform BT Classic abstraction would require:
- `bluer` on Linux
- `windows` crate on Windows  
- Raw `IOBluetooth` FFI on macOS

This is a substantial engineering effort with ongoing maintenance burden.

---

## 4. Practical Assessment

### Is BLE printing viable for v1?

**Marginally.** Using `btleplug`:
- ✅ Cross-platform (Linux, macOS, Windows)
- ✅ Can discover Zebra printers and write ZPL to GATT characteristic
- ❌ Slow: 2–4 KB/s (a 50 KB label = 12–25 seconds)
- ❌ No MTU negotiation — must implement chunking manually
- ❌ Some printer models don't support `writeWithoutResponse`
- ❌ Zebra themselves recommend BLE for configuration only, not printing
- ❌ Adds significant complexity for a transport that's worse than TCP or USB

### Is BT Classic printing viable for v1?

**No.** The platform fragmentation is too severe:
- Linux: good (`bluer`)
- Windows: possible but Windows-specific code (`windows` crate)
- macOS: no viable crate; requires unsafe Objective-C FFI

Building and maintaining three platform-specific Bluetooth implementations is not justified for a v1 release.

### Is the `serialport` escape hatch viable?

**Yes, with caveats.** This is the pragmatic approach:
- ✅ Fully cross-platform via one crate
- ✅ Full SPP bandwidth (not limited to BLE speeds)
- ✅ Well-maintained, mature crate
- ❌ Requires OS-level pairing before use (can't discover/pair from your app)
- ❌ User must know which serial port corresponds to their printer
- ⚠️ Port enumeration works but identifying BT vs. USB vs. physical serial requires heuristics

---

## 5. Recommendation

### v1 Transport Strategy

| Transport | Priority | Crate | Status |
|-----------|----------|-------|--------|
| **TCP 9100** | P0 (primary) | `tokio::net::TcpStream` | Trivial, cross-platform, standard |
| **USB** | P1 | TBD (platform-specific USB or `serialport`) | Cross-platform, common in desktop scenarios |
| **BT SPP via serial port** | P2 (escape hatch) | `serialport` 4.x | Works if user pairs printer at OS level first |
| **BLE GATT** | Deferred | `btleplug` | Possible but slow; add in v2 if demand exists |
| **Native BT Classic** | Deferred | Platform-specific | Too fragmented for v1; revisit if ecosystem matures |

### Architecture for Future Bluetooth Support

Design the transport layer as a trait:

```rust
#[async_trait]
pub trait PrintTransport: Send + Sync {
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;
    async fn receive(&mut self, buf: &mut [u8]) -> Result<usize, TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
}
```

Implement `TcpTransport` and `SerialTransport` for v1. BLE and native BT Classic can be added later as additional implementors without changing the API.

### If BLE Is Needed Sooner

If a user specifically needs BLE printing before v2:

1. Use `btleplug` 0.11.x
2. Discover the Zebra Parser Service by UUID `38eb4a80-c570-11e3-9507-0002a5d5c51b`
3. Write ZPL data in chunks (≤512 bytes) to characteristic `38eb4a82-...`
4. Use `writeWithResponse` for reliability, `writeWithoutResponse` for speed (if supported)
5. Optionally subscribe to indications on characteristic `38eb4a81-...` to read printer responses
6. Implement your own retry/flow-control logic

This is ~200–400 lines of code and is not difficult, but the resulting UX (slow, requires manual chunking, model-dependent behavior) is not suitable as a primary transport.

---

## 6. Key References

| Resource | URL |
|----------|-----|
| `bluer` docs | https://docs.rs/bluer/latest/bluer |
| `bluer` RFCOMM module | https://docs.rs/bluer/latest/bluer/rfcomm |
| `btleplug` GitHub | https://github.com/deviceplug/btleplug |
| `btleplug` platform matrix | https://thedocumentation.org/btleplug |
| `serialport` docs | https://docs.rs/serialport/latest/serialport |
| `windows` Bluetooth API | https://microsoft.github.io/windows-docs-rs/doc/windows/Devices/Bluetooth |
| Zebra BLE AppNote (PDF) | https://www.zebra.com/content/dam/zebra/software/en/application-notes/AppNote-BlueToothLE-v4.pdf |
| Zebra BLE printing forum | https://developer.zebra.com/content/printing-webapp-using-webbluetooth |
| Zebra BLE throughput forum | https://developer.zebra.com/content/maximising-blegatt-transfer-rate |
| Zebra Bluetooth User Guide | https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/bluetooth-ug-en.pdf |
