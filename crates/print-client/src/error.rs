//! Typed error types for the print client.

use std::fmt;
use std::io;
use std::time::Duration;

/// Printer error conditions, categorized by type.
///
/// Each variant carries enough context to produce a helpful error message.
/// Use [`PrintError::is_retryable()`] to classify transient vs permanent failures.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    // -- Connection --
    /// The printer actively refused the connection (e.g. port not open).
    #[error("connection refused: {addr}")]
    ConnectionRefused {
        /// The address that was attempted.
        addr: String,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },

    /// TCP connect timed out before the printer responded.
    #[error("connection timed out: {addr} ({timeout:?})")]
    ConnectionTimeout {
        /// The address that was attempted.
        addr: String,
        /// The configured timeout that elapsed.
        timeout: Duration,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },

    /// Connection failed for a reason other than refusal or timeout.
    #[error("connection failed: {addr}")]
    ConnectionFailed {
        /// The address that was attempted.
        addr: String,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },

    /// The printer closed the connection unexpectedly.
    #[error("connection closed by printer")]
    ConnectionClosed,

    // -- Address --
    /// The provided address string could not be parsed.
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// DNS resolution found no addresses for the given hostname.
    #[error("no address found for hostname: {0}")]
    NoAddressFound(String),

    // -- I/O --
    /// Writing data to the printer failed.
    #[error("write failed: {0}")]
    WriteFailed(#[source] io::Error),

    /// Reading data from the printer failed.
    #[error("read failed: {0}")]
    ReadFailed(#[source] io::Error),

    /// The printer did not respond within the read timeout.
    #[error("read timed out waiting for response")]
    ReadTimeout,

    // -- Protocol / Framing --
    /// The response from the printer could not be parsed as valid STX/ETX frames.
    #[error("malformed response frame: {details}")]
    MalformedFrame {
        /// Human-readable description of the parsing failure.
        details: String,
    },

    /// A response frame exceeded the maximum allowed size.
    #[error("frame too large ({size} bytes, max {max})")]
    FrameTooLarge {
        /// Actual size of the oversized frame in bytes.
        size: usize,
        /// Configured maximum frame size in bytes.
        max: usize,
    },

    // -- Printer state errors --
    /// The printer reported a hardware/media error via `~HS`.
    #[error("printer error: {0}")]
    PrinterError(PrinterErrorKind),

    // -- Retry --
    /// All retry attempts have been exhausted.
    #[error("retries exhausted after {attempts} attempts")]
    RetriesExhausted {
        /// Total number of attempts made.
        attempts: u32,
        /// The error from the final attempt.
        #[source]
        last_error: Box<PrintError>,
    },

    // -- Preflight validation --
    /// Pre-print validation (linting) detected errors in the ZPL.
    #[error("preflight validation failed")]
    PreflightFailed,

    // -- Configuration --
    /// An invalid configuration was provided.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    // -- USB-specific --
    /// No USB printer matching the requested criteria was found.
    #[error("USB device not found")]
    UsbDeviceNotFound,

    /// A USB transport error occurred.
    #[error("USB error: {0}")]
    UsbError(String),

    // -- Serial-specific --
    /// A serial port transport error occurred.
    #[error("serial port error: {0}")]
    SerialError(String),

    // -- Completion tracking --
    /// Timed out waiting for the printer to finish processing.
    #[error(
        "timeout waiting for completion ({formats_in_buffer} formats in buffer, {labels_remaining} labels remaining)"
    )]
    CompletionTimeout {
        /// Number of formats still in the printer's receive buffer.
        formats_in_buffer: u32,
        /// Number of labels still in the printer's queue when the timeout fired.
        labels_remaining: u32,
    },
}

impl PrintError {
    /// Returns `true` if this error is transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PrintError::ConnectionTimeout { .. }
                | PrintError::ConnectionClosed
                | PrintError::WriteFailed(_)
                | PrintError::ReadFailed(_)
                | PrintError::ReadTimeout
                | PrintError::CompletionTimeout { .. }
        )
    }
}

/// Specific printer error conditions derived from `~HS` status flags.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterErrorKind {
    /// Media (label stock) is depleted or not detected.
    PaperOut,
    /// Ribbon cartridge is depleted or missing.
    RibbonOut,
    /// Print head is open / not latched.
    HeadOpen,
    /// Print head temperature is above the safe operating range.
    OverTemperature,
    /// Print head temperature is below the safe operating range.
    UnderTemperature,
    /// Printer RAM integrity check failed.
    CorruptRam,
    /// The printer's receive buffer is full.
    BufferFull,
}

impl fmt::Display for PrinterErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrinterErrorKind::PaperOut => write!(f, "paper out"),
            PrinterErrorKind::RibbonOut => write!(f, "ribbon out"),
            PrinterErrorKind::HeadOpen => write!(f, "head open"),
            PrinterErrorKind::OverTemperature => write!(f, "over temperature"),
            PrinterErrorKind::UnderTemperature => write!(f, "under temperature"),
            PrinterErrorKind::CorruptRam => write!(f, "corrupt RAM"),
            PrinterErrorKind::BufferFull => write!(f, "buffer full"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_errors() {
        assert!(
            PrintError::ConnectionTimeout {
                addr: "x".into(),
                timeout: Duration::from_secs(1),
                source: io::Error::new(io::ErrorKind::TimedOut, "test"),
            }
            .is_retryable()
        );
        assert!(PrintError::ConnectionClosed.is_retryable());
        assert!(
            PrintError::WriteFailed(io::Error::new(io::ErrorKind::BrokenPipe, "test"))
                .is_retryable()
        );
        assert!(PrintError::ReadFailed(io::Error::other("test")).is_retryable());
        assert!(PrintError::ReadTimeout.is_retryable());
        assert!(
            PrintError::CompletionTimeout {
                formats_in_buffer: 0,
                labels_remaining: 5
            }
            .is_retryable()
        );
    }

    #[test]
    fn non_retryable_errors() {
        assert!(
            !PrintError::ConnectionRefused {
                addr: "x".into(),
                source: io::Error::new(io::ErrorKind::ConnectionRefused, "test"),
            }
            .is_retryable()
        );
        assert!(
            !PrintError::ConnectionFailed {
                addr: "x".into(),
                source: io::Error::other("test"),
            }
            .is_retryable()
        );
        assert!(!PrintError::InvalidAddress("x".into()).is_retryable());
        assert!(!PrintError::NoAddressFound("x".into()).is_retryable());
        assert!(
            !PrintError::MalformedFrame {
                details: "x".into()
            }
            .is_retryable()
        );
        assert!(
            !PrintError::FrameTooLarge {
                size: 2000,
                max: 1024
            }
            .is_retryable()
        );
        assert!(!PrintError::PrinterError(PrinterErrorKind::PaperOut).is_retryable());
        assert!(!PrintError::PreflightFailed.is_retryable());
        assert!(!PrintError::UsbDeviceNotFound.is_retryable());
        assert!(!PrintError::UsbError("x".into()).is_retryable());
        assert!(!PrintError::SerialError("x".into()).is_retryable());
        assert!(!PrintError::InvalidConfig("test".into()).is_retryable());
        assert!(
            !PrintError::RetriesExhausted {
                attempts: 3,
                last_error: Box::new(PrintError::ReadTimeout),
            }
            .is_retryable()
        );
    }
}
