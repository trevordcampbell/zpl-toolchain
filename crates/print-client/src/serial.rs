//! Serial/Bluetooth SPP transport for Zebra printers using the `serialport` crate.
//!
//! Feature-gated behind the `serial` Cargo feature (enabled by default in the CLI).
//!
//! Serial connections are always bidirectional, so `SerialPrinter` implements
//! both `Printer` and `StatusQuery` traits.

use std::io::Write;

use crate::frame::{DEFAULT_MAX_FRAME_SIZE, expected_frame_count, read_frames};
use crate::{PrintError, Printer, PrinterConfig, StatusQuery};

/// Default baud rate for Zebra label printers (9600 8N1).
const DEFAULT_BAUD: u32 = 9600;

/// A Zebra printer connected over a serial port (RS-232, USB-serial, or Bluetooth SPP).
///
/// Serial connections are inherently bidirectional, so this type supports
/// both sending ZPL and querying printer status.
pub struct SerialPrinter {
    /// The underlying serial port handle.
    port: Box<dyn serialport::SerialPort>,
    /// Printer configuration (timeouts, retry settings).
    config: PrinterConfig,
}

impl SerialPrinter {
    /// Open a serial port at the given path and baud rate.
    ///
    /// # Arguments
    ///
    /// * `path` — Serial port path, e.g. `/dev/ttyUSB0`, `/dev/tty.ZebraPrinter`, `COM3`.
    /// * `baud` — Baud rate. Zebra printers default to 9600, but some may be configured
    ///   for 19200, 38400, or 115200.
    /// * `config` — Printer configuration with timeout and retry settings.
    ///
    /// # Errors
    ///
    /// Returns `PrintError::SerialError` if the port cannot be opened.
    pub fn open(path: &str, baud: u32, config: PrinterConfig) -> Result<Self, PrintError> {
        let timeout = config.timeouts.read.max(config.timeouts.write);
        let port = serialport::new(path, baud)
            .timeout(timeout)
            .open()
            .map_err(|e| PrintError::SerialError(e.to_string()))?;

        Ok(Self { port, config })
    }

    /// Open a serial port with the Zebra default baud rate (9600 8N1).
    ///
    /// This is a convenience wrapper over [`open`](Self::open) for the common case.
    ///
    /// # Errors
    ///
    /// Returns `PrintError::SerialError` if the port cannot be opened.
    pub fn open_default(path: &str, config: PrinterConfig) -> Result<Self, PrintError> {
        Self::open(path, DEFAULT_BAUD, config)
    }

    /// List available serial port names on the system.
    ///
    /// Returns port paths like `/dev/ttyUSB0`, `/dev/tty.usbserial-*`, or `COM3`.
    /// This is useful for discovery and user-facing port selection.
    ///
    /// **Note:** On Linux, this crate is built with `serialport`'s default features
    /// disabled (no `libudev`). Port enumeration still works via a sysfs fallback
    /// but may return fewer details than the libudev backend.
    pub fn list_ports() -> Vec<String> {
        serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.port_name)
            .collect()
    }
}

impl Printer for SerialPrinter {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
        self.port.write_all(data).map_err(PrintError::WriteFailed)?;

        self.port.flush().map_err(PrintError::WriteFailed)?;

        Ok(())
    }
}

impl StatusQuery for SerialPrinter {
    fn query_raw(&mut self, cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
        // Send the query command
        self.send_raw(cmd)?;

        let expected_frames = expected_frame_count(cmd);
        let timeout = self.config.timeouts.read;

        // The serial port already implements `std::io::Read`, so we can
        // pass it directly to the frame parser. The port's read timeout
        // is set during open, and `read_frames` handles its own deadline.
        read_frames(
            &mut self.port,
            expected_frames,
            timeout,
            DEFAULT_MAX_FRAME_SIZE,
        )
    }
}
