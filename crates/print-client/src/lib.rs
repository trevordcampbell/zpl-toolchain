//! ZPL Print Client — send ZPL to Zebra and ZPL-compatible printers.
//!
//! Supports TCP (port 9100), USB, and serial/Bluetooth SPP transports.
//! The core API is synchronous (`std::net`), with no async runtime required.
mod addr;
mod config;
mod error;
mod frame;
mod retry;
#[cfg(feature = "serial")]
mod serial;
mod status;
#[cfg(feature = "tcp")]
mod tcp;
#[cfg(feature = "usb")]
mod usb;

#[cfg(feature = "tcp")]
pub use addr::resolve_printer_addr;
pub use config::{BatchOptions, PrinterConfig, PrinterTimeouts, RetryConfig};
pub use error::{PrintError, PrinterErrorKind};
pub use frame::{expected_frame_count, read_frames};
pub use retry::{ReconnectRetryPrinter, RetryPrinter};
#[cfg(feature = "serial")]
pub use serial::{
    SerialDataBits, SerialFlowControl, SerialParity, SerialPrinter, SerialSettings, SerialStopBits,
};
pub use status::{HostStatus, PrintMode, PrinterInfo};
#[cfg(feature = "tcp")]
pub use tcp::TcpPrinter;
#[cfg(feature = "usb")]
pub use usb::UsbPrinter;

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

// ── Traits ──────────────────────────────────────────────────────────────

/// Send data to a printer. All transports implement this.
pub trait Printer: Send {
    /// Send raw bytes to the printer.
    fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError>;

    /// Send a ZPL string to the printer (convenience wrapper over `send_raw`).
    fn send_zpl(&mut self, zpl: &str) -> Result<(), PrintError> {
        self.send_raw(zpl.as_bytes())
    }
}

/// Query printer status. Only bidirectional transports implement this.
pub trait StatusQuery: Printer {
    /// Send a command and read the raw STX/ETX framed response.
    fn query_raw(&mut self, cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError>;

    /// Query printer status via `~HS` and parse the response.
    fn query_status(&mut self) -> Result<HostStatus, PrintError> {
        let frames = self.query_raw(b"~HS")?;
        HostStatus::parse(&frames)
    }

    /// Query printer info via `~HI` and parse the response.
    fn query_info(&mut self) -> Result<PrinterInfo, PrintError> {
        let frames = self.query_raw(b"~HI")?;
        PrinterInfo::parse(&frames)
    }
}

/// A printer that can re-establish its connection after a failure.
///
/// Implementing this trait enables [`ReconnectRetryPrinter`] to automatically
/// reconnect between retry attempts, making retries effective even after a
/// full connection drop (e.g., TCP disconnect, USB unplug/replug).
pub trait Reconnectable {
    /// Re-establish the connection.
    ///
    /// Implementations should close the old connection (if any) and open a
    /// fresh one. Errors during reconnection are non-fatal for the retry
    /// loop — the next operation attempt may still succeed or produce a
    /// more specific error.
    fn reconnect(&mut self) -> Result<(), PrintError>;
}

// ── Batch helpers ───────────────────────────────────────────────────────

/// Progress report for batch printing.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchProgress {
    /// How many labels have been sent so far.
    pub sent: usize,
    /// Total labels in the batch.
    pub total: usize,
    /// Latest printer status (if polling was enabled).
    pub status: Option<HostStatus>,
}

/// Result of a batch print operation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchResult {
    /// Number of labels successfully sent to the printer.
    pub sent: usize,
    /// Total labels in the batch.
    pub total: usize,
}

/// Send a batch of labels with optional progress reporting.
///
/// The `on_progress` callback receives a `BatchProgress` and can return
/// `ControlFlow::Break(())` to abort the batch early.
pub fn send_batch<P, F>(
    printer: &mut P,
    labels: &[impl AsRef<[u8]>],
    mut on_progress: F,
) -> Result<BatchResult, PrintError>
where
    P: Printer,
    F: FnMut(BatchProgress) -> ControlFlow<(), ()>,
{
    let total = labels.len();
    for (i, label) in labels.iter().enumerate() {
        printer.send_raw(label.as_ref())?;

        let status = None; // Status polling requires send_batch_with_status()

        let progress = BatchProgress {
            sent: i + 1,
            total,
            status,
        };

        if let ControlFlow::Break(()) = on_progress(progress) {
            return Ok(BatchResult { sent: i + 1, total });
        }
    }

    Ok(BatchResult { sent: total, total })
}

/// Send a batch of labels with status polling (requires bidirectional transport).
pub fn send_batch_with_status<P, F>(
    printer: &mut P,
    labels: &[impl AsRef<[u8]>],
    opts: &BatchOptions,
    mut on_progress: F,
) -> Result<BatchResult, PrintError>
where
    P: StatusQuery,
    F: FnMut(BatchProgress) -> ControlFlow<(), ()>,
{
    let total = labels.len();
    for (i, label) in labels.iter().enumerate() {
        printer.send_raw(label.as_ref())?;

        let status = if let Some(interval) = opts.status_interval {
            if (i + 1) % interval.get() == 0 {
                printer.query_status().ok()
            } else {
                None
            }
        } else {
            None
        };

        let progress = BatchProgress {
            sent: i + 1,
            total,
            status,
        };

        if let ControlFlow::Break(()) = on_progress(progress) {
            return Ok(BatchResult { sent: i + 1, total });
        }
    }

    Ok(BatchResult { sent: total, total })
}

// ── Completion polling ─────────────────────────────────────────────────

/// Poll `~HS` until the printer reports no formats in buffer and no
/// labels remaining, or until the timeout elapses.
///
/// This is useful after sending a batch of labels to wait until the
/// physical printer has finished printing all of them.
///
/// Works with any transport that implements [`StatusQuery`].
pub fn wait_for_completion<S: StatusQuery>(
    printer: &mut S,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<(), PrintError> {
    let now = Instant::now();
    let deadline = now
        .checked_add(timeout)
        .unwrap_or_else(|| now + Duration::from_secs(86400));

    loop {
        let status = printer.query_status()?;

        if status.formats_in_buffer == 0 && status.labels_remaining == 0 {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(PrintError::CompletionTimeout {
                formats_in_buffer: status.formats_in_buffer,
                labels_remaining: status.labels_remaining,
            });
        }

        std::thread::sleep(poll_interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::ControlFlow;

    struct MockBatchPrinter {
        sent: Vec<Vec<u8>>,
        fail_on: Option<usize>,
    }

    impl Printer for MockBatchPrinter {
        fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
            if Some(self.sent.len()) == self.fail_on {
                return Err(PrintError::WriteFailed(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "mock error",
                )));
            }
            self.sent.push(data.to_vec());
            Ok(())
        }
    }

    #[test]
    fn batch_happy_path() {
        let mut printer = MockBatchPrinter {
            sent: Vec::new(),
            fail_on: None,
        };
        let labels = vec!["^XA^FDOne^FS^XZ", "^XA^FDTwo^FS^XZ", "^XA^FDThree^FS^XZ"];
        let result = send_batch(&mut printer, &labels, |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(result.sent, 3);
        assert_eq!(result.total, 3);
        assert_eq!(printer.sent.len(), 3);
    }

    #[test]
    fn batch_empty() {
        let mut printer = MockBatchPrinter {
            sent: Vec::new(),
            fail_on: None,
        };
        let labels: Vec<&str> = vec![];
        let result = send_batch(&mut printer, &labels, |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(result.sent, 0);
        assert_eq!(result.total, 0);
    }

    #[test]
    fn batch_early_abort() {
        let mut printer = MockBatchPrinter {
            sent: Vec::new(),
            fail_on: None,
        };
        let labels = vec!["one", "two", "three", "four", "five"];
        let result = send_batch(&mut printer, &labels, |progress| {
            if progress.sent >= 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .unwrap();
        assert_eq!(result.sent, 2);
        assert_eq!(result.total, 5);
    }

    #[test]
    fn batch_error_propagates() {
        let mut printer = MockBatchPrinter {
            sent: Vec::new(),
            fail_on: Some(1),
        };
        let labels = vec!["ok", "fail", "never"];
        let result = send_batch(&mut printer, &labels, |_| ControlFlow::Continue(()));
        assert!(result.is_err());
        assert_eq!(printer.sent.len(), 1);
    }

    // ── MockStatusPrinter (for send_batch_with_status tests) ─────────

    struct MockStatusPrinter {
        sent: Vec<Vec<u8>>,
        fail_on: Option<usize>,
        status_queries: usize,
    }

    impl Printer for MockStatusPrinter {
        fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
            if Some(self.sent.len()) == self.fail_on {
                return Err(PrintError::WriteFailed(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "mock error",
                )));
            }
            self.sent.push(data.to_vec());
            Ok(())
        }
    }

    impl StatusQuery for MockStatusPrinter {
        fn query_raw(&mut self, _cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
            self.status_queries += 1;
            // Return a valid ~HS response (3 frames)
            Ok(vec![
                b"030,0,0,1245,000,0,0,0,000,0,0,0".to_vec(),
                b"000,0,0,0,0,2,0,0,00000000,0,000".to_vec(),
                b"1234,0".to_vec(),
            ])
        }
    }

    #[test]
    fn batch_with_status_happy_path() {
        use std::num::NonZeroUsize;

        let mut printer = MockStatusPrinter {
            sent: Vec::new(),
            fail_on: None,
            status_queries: 0,
        };
        let labels: Vec<&str> = vec!["L1", "L2", "L3", "L4", "L5"];
        let opts = BatchOptions {
            status_interval: Some(NonZeroUsize::new(2).unwrap()),
            ..BatchOptions::default()
        };

        let mut progresses = Vec::new();
        let result = send_batch_with_status(&mut printer, &labels, &opts, |p| {
            progresses.push(p.clone());
            ControlFlow::Continue(())
        })
        .unwrap();

        assert_eq!(result.sent, 5);
        assert_eq!(result.total, 5);
        assert_eq!(printer.sent.len(), 5);
        // Status polled after label 2 and 4 (every 2 labels)
        assert_eq!(printer.status_queries, 2);

        // Labels at sent=2 and sent=4 should have status
        assert!(progresses[1].status.is_some()); // sent=2
        assert!(progresses[3].status.is_some()); // sent=4

        // Labels at sent=1, 3, 5 should NOT have status
        assert!(progresses[0].status.is_none()); // sent=1
        assert!(progresses[2].status.is_none()); // sent=3
        assert!(progresses[4].status.is_none()); // sent=5
    }

    #[test]
    fn batch_with_status_no_interval() {
        let mut printer = MockStatusPrinter {
            sent: Vec::new(),
            fail_on: None,
            status_queries: 0,
        };
        let labels: Vec<&str> = vec!["L1", "L2", "L3"];
        let opts = BatchOptions {
            status_interval: None,
            ..BatchOptions::default()
        };

        let result =
            send_batch_with_status(&mut printer, &labels, &opts, |_| ControlFlow::Continue(()))
                .unwrap();

        assert_eq!(result.sent, 3);
        assert_eq!(result.total, 3);
        assert_eq!(printer.status_queries, 0);
    }

    #[test]
    fn batch_with_status_early_abort() {
        use std::num::NonZeroUsize;

        let mut printer = MockStatusPrinter {
            sent: Vec::new(),
            fail_on: None,
            status_queries: 0,
        };
        let labels: Vec<&str> = vec!["L1", "L2", "L3", "L4", "L5"];
        let opts = BatchOptions {
            status_interval: Some(NonZeroUsize::new(2).unwrap()),
            ..BatchOptions::default()
        };

        let result = send_batch_with_status(&mut printer, &labels, &opts, |p| {
            if p.sent >= 3 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .unwrap();

        assert_eq!(result.sent, 3);
        assert_eq!(result.total, 5);
        assert_eq!(printer.sent.len(), 3);
        assert_eq!(printer.status_queries, 1); // polled after label 2 only
    }

    #[test]
    fn batch_with_status_error_propagates() {
        use std::num::NonZeroUsize;

        let mut printer = MockStatusPrinter {
            sent: Vec::new(),
            fail_on: Some(1), // fail on second label
            status_queries: 0,
        };
        let labels: Vec<&str> = vec!["L1", "L2", "L3"];
        let opts = BatchOptions {
            status_interval: Some(NonZeroUsize::new(1).unwrap()),
            ..BatchOptions::default()
        };

        let result =
            send_batch_with_status(&mut printer, &labels, &opts, |_| ControlFlow::Continue(()));

        assert!(result.is_err());
        assert_eq!(printer.sent.len(), 1);
    }

    // ── MockCompletionPrinter (for wait_for_completion tests) ────────

    struct MockCompletionPrinter {
        polls: usize,
        complete_after: usize,
    }

    impl Printer for MockCompletionPrinter {
        fn send_raw(&mut self, _data: &[u8]) -> Result<(), PrintError> {
            Ok(())
        }
    }

    impl StatusQuery for MockCompletionPrinter {
        fn query_raw(&mut self, _cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
            self.polls += 1;
            let remaining = if self.polls >= self.complete_after {
                0
            } else {
                5
            };
            Ok(vec![
                b"030,0,0,1245,000,0,0,0,000,0,0,0".to_vec(),
                format!("000,0,0,0,0,2,0,{remaining},00000000,0,000").into_bytes(),
                b"1234,0".to_vec(),
            ])
        }
    }

    #[test]
    fn wait_for_completion_immediate() {
        let mut printer = MockCompletionPrinter {
            polls: 0,
            complete_after: 1, // complete on first poll
        };
        let result = wait_for_completion(
            &mut printer,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        assert!(result.is_ok());
        assert_eq!(printer.polls, 1);
    }

    #[test]
    fn wait_for_completion_after_polls() {
        let mut printer = MockCompletionPrinter {
            polls: 0,
            complete_after: 3, // complete on third poll
        };
        let result = wait_for_completion(
            &mut printer,
            Duration::from_millis(10), // short interval for test speed
            Duration::from_secs(5),
        );
        assert!(result.is_ok());
        assert_eq!(printer.polls, 3);
    }

    #[test]
    fn wait_for_completion_timeout() {
        let mut printer = MockCompletionPrinter {
            polls: 0,
            complete_after: 999, // never completes
        };
        let result = wait_for_completion(
            &mut printer,
            Duration::from_millis(1),
            Duration::from_millis(10),
        );
        match result {
            Err(PrintError::CompletionTimeout {
                formats_in_buffer,
                labels_remaining,
            }) => {
                assert_eq!(formats_in_buffer, 0);
                assert_eq!(labels_remaining, 5);
            }
            other => panic!("expected CompletionTimeout, got {:?}", other),
        }
    }

    // ── MockFormatsInBufferPrinter (formats_in_buffer blocks completion) ──

    struct MockFormatsInBufferPrinter {
        polls: usize,
    }

    impl Printer for MockFormatsInBufferPrinter {
        fn send_raw(&mut self, _data: &[u8]) -> Result<(), PrintError> {
            Ok(())
        }
    }

    impl StatusQuery for MockFormatsInBufferPrinter {
        fn query_raw(&mut self, _cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
            self.polls += 1;
            // labels_remaining always 0, but formats_in_buffer > 0 until poll 3
            let formats = if self.polls >= 3 { 0 } else { 2 };
            Ok(vec![
                format!("030,0,0,1245,{formats:03},0,0,0,000,0,0,0").into_bytes(),
                b"000,0,0,0,0,2,0,0,00000000,0,000".to_vec(),
                b"1234,0".to_vec(),
            ])
        }
    }

    #[test]
    fn wait_for_completion_waits_for_formats_in_buffer() {
        let mut printer = MockFormatsInBufferPrinter { polls: 0 };
        let result = wait_for_completion(
            &mut printer,
            Duration::from_millis(1),
            Duration::from_secs(5),
        );
        assert!(result.is_ok());
        assert!(
            printer.polls >= 3,
            "should have polled until formats cleared"
        );
    }
}
