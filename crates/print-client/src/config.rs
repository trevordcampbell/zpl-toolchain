//! Configuration types for the print client.

use std::time::Duration;

/// Complete printer configuration: timeouts + retry settings.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct PrinterConfig {
    /// Network/transport timeout settings.
    pub timeouts: PrinterTimeouts,
    /// Retry settings for transient failures.
    pub retry: RetryConfig,
    /// Enable transport-level byte tracing for diagnostics.
    ///
    /// When enabled, transports may emit hex/ASCII byte dumps to stderr.
    pub trace_io: bool,
}

/// Timeout settings for printer connections.
///
/// Defaults are tuned for LAN-connected label printers:
/// - `connect`: 5s (generous for LAN, might be tight for VPN)
/// - `write`: 30s (labels with embedded ^GF graphics can be 500KB+)
/// - `read`: 10s (~HS response can be delayed while printer is mid-print)
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PrinterTimeouts {
    /// Maximum time to wait for TCP/USB/serial connection to establish.
    pub connect: Duration,
    /// Maximum time to wait for a write to complete.
    pub write: Duration,
    /// Maximum time to wait for a response after sending a query command.
    pub read: Duration,
}

impl Default for PrinterTimeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(5),
            write: Duration::from_secs(30),
            read: Duration::from_secs(10),
        }
    }
}

/// Retry settings for transient failures.
///
/// Uses exponential backoff with optional jitter. Only errors where
/// `PrintError::is_retryable()` returns `true` are retried.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the initial attempt).
    pub max_attempts: u32,
    /// Initial delay between retries.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Whether to add random jitter to retry delays.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            jitter: true,
        }
    }
}

/// Options for batch printing operations.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct BatchOptions {
    /// Poll ~HS every N labels to track progress.
    /// `None` disables status polling.
    pub status_interval: Option<std::num::NonZeroUsize>,
}
