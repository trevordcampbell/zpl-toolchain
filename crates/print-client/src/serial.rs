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

/// Serial line settings used to open a serial port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SerialSettings {
    /// Data bits per symbol (typically 8 for Zebra).
    pub data_bits: SerialDataBits,
    /// Parity mode (typically none for Zebra).
    pub parity: SerialParity,
    /// Stop bits (typically 1 for Zebra).
    pub stop_bits: SerialStopBits,
    /// Flow control mode (often software/XON-XOFF on Zebra serial links).
    pub flow_control: SerialFlowControl,
}

impl Default for SerialSettings {
    fn default() -> Self {
        Self {
            data_bits: SerialDataBits::Eight,
            parity: SerialParity::None,
            stop_bits: SerialStopBits::One,
            flow_control: SerialFlowControl::Software,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Supported serial data-bit settings.
pub enum SerialDataBits {
    /// 7 data bits.
    Seven,
    /// 8 data bits.
    Eight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Supported parity settings.
pub enum SerialParity {
    /// No parity bit.
    None,
    /// Even parity.
    Even,
    /// Odd parity.
    Odd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Supported stop-bit settings.
pub enum SerialStopBits {
    /// 1 stop bit.
    One,
    /// 2 stop bits.
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Supported flow-control settings.
pub enum SerialFlowControl {
    /// No line-level flow control.
    None,
    /// Software flow control (XON/XOFF).
    Software,
    /// Hardware flow control (RTS/CTS).
    Hardware,
}

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
        Self::open_with_settings(path, baud, SerialSettings::default(), config)
    }

    /// Open a serial port with explicit serial line settings.
    ///
    /// Use this when printer-side serial config is not known or must be overridden.
    pub fn open_with_settings(
        path: &str,
        baud: u32,
        settings: SerialSettings,
        config: PrinterConfig,
    ) -> Result<Self, PrintError> {
        let timeout = config.timeouts.read.max(config.timeouts.write);
        let port = serialport::new(path, baud)
            .data_bits(map_data_bits(settings.data_bits))
            .parity(map_parity(settings.parity))
            .stop_bits(map_stop_bits(settings.stop_bits))
            .flow_control(map_flow_control(settings.flow_control))
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
        if self.config.trace_io {
            trace_bytes("serial tx", data);
        }
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
        let frames = read_frames(
            &mut self.port,
            expected_frames,
            timeout,
            DEFAULT_MAX_FRAME_SIZE,
        )?;

        if self.config.trace_io {
            for frame in &frames {
                trace_bytes("serial rx", frame);
            }
        }
        Ok(frames)
    }
}

fn map_data_bits(bits: SerialDataBits) -> serialport::DataBits {
    match bits {
        SerialDataBits::Seven => serialport::DataBits::Seven,
        SerialDataBits::Eight => serialport::DataBits::Eight,
    }
}

fn map_parity(parity: SerialParity) -> serialport::Parity {
    match parity {
        SerialParity::None => serialport::Parity::None,
        SerialParity::Even => serialport::Parity::Even,
        SerialParity::Odd => serialport::Parity::Odd,
    }
}

fn map_stop_bits(bits: SerialStopBits) -> serialport::StopBits {
    match bits {
        SerialStopBits::One => serialport::StopBits::One,
        SerialStopBits::Two => serialport::StopBits::Two,
    }
}

fn map_flow_control(flow: SerialFlowControl) -> serialport::FlowControl {
    match flow {
        SerialFlowControl::None => serialport::FlowControl::None,
        SerialFlowControl::Software => serialport::FlowControl::Software,
        SerialFlowControl::Hardware => serialport::FlowControl::Hardware,
    }
}

fn trace_bytes(label: &str, bytes: &[u8]) {
    let hex = bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");
    let ascii = bytes
        .iter()
        .map(|b| {
            if b.is_ascii_graphic() || *b == b' ' {
                char::from(*b)
            } else {
                '.'
            }
        })
        .collect::<String>();
    eprintln!(
        "[trace-io] {label} len={} hex=[{}] ascii='{}'",
        bytes.len(),
        hex,
        ascii
    );
}
