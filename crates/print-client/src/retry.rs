//! Retry wrapper that adds exponential-backoff retry logic to any `Printer`.

use std::time::{Duration, SystemTime};

use crate::config::RetryConfig;
use crate::{PrintError, Printer, Reconnectable, StatusQuery};

/// A wrapper that adds retry-with-backoff to any `Printer` implementation.
///
/// # Reconnection caveat
///
/// `RetryPrinter` retries operations on the **same underlying connection**.
/// If the connection breaks (e.g., TCP disconnect), the retried writes will
/// fail immediately on the dead stream. For transports that support
/// reconnection (e.g., [`TcpPrinter::reconnect()`](crate::TcpPrinter::reconnect)),
/// you should handle reconnection at the call site rather than relying on
/// `RetryPrinter` alone.
///
/// `RetryPrinter` is most useful for **transient** errors (brief network
/// hiccups, printer busy) where the underlying stream remains valid.
pub struct RetryPrinter<P> {
    inner: P,
    retry_config: RetryConfig,
}

impl<P> RetryPrinter<P> {
    /// Create a new `RetryPrinter` wrapping `inner` with the given retry configuration.
    pub fn new(inner: P, retry_config: RetryConfig) -> Self {
        Self {
            inner,
            retry_config,
        }
    }

    /// Unwrap the `RetryPrinter`, returning the inner printer.
    pub fn into_inner(self) -> P {
        self.inner
    }

    /// Get a shared reference to the inner printer.
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Get a mutable reference to the inner printer.
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

impl<P: Printer> Printer for RetryPrinter<P> {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
        retry_op(&self.retry_config, || self.inner.send_raw(data))
    }
}

impl<P: StatusQuery> StatusQuery for RetryPrinter<P> {
    fn query_raw(&mut self, cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
        retry_op(&self.retry_config, || self.inner.query_raw(cmd))
    }
}

// ── Retry helper ───────────────────────────────────────────────────────

/// Execute `op`, retrying on retryable errors with exponential backoff.
///
/// Non-retryable errors are returned immediately. On exhausting all attempts
/// the last retryable error is wrapped in [`PrintError::RetriesExhausted`].
fn retry_op<T, F>(config: &RetryConfig, mut op: F) -> Result<T, PrintError>
where
    F: FnMut() -> Result<T, PrintError>,
{
    if config.max_attempts == 0 {
        return Err(PrintError::InvalidConfig(
            "max_attempts must be >= 1".into(),
        ));
    }

    let mut last_error: Option<PrintError> = None;

    for attempt in 0..config.max_attempts {
        match op() {
            Ok(val) => return Ok(val),
            Err(e) => {
                if !e.is_retryable() {
                    return Err(e);
                }
                last_error = Some(e);

                // Don't sleep after the last attempt.
                if attempt + 1 < config.max_attempts {
                    let delay = compute_delay(config, attempt);
                    std::thread::sleep(delay);
                }
            }
        }
    }

    // We only reach here when every attempt failed with a retryable error.
    Err(PrintError::RetriesExhausted {
        attempts: config.max_attempts,
        last_error: Box::new(
            last_error.unwrap_or_else(|| {
                unreachable!("at least one attempt was made (max_attempts >= 1)")
            }),
        ),
    })
}

/// Compute the backoff delay for the given `attempt` (0-indexed).
///
/// delay = min(initial_delay * 2^attempt, max_delay), optionally with jitter.
fn compute_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let base = config
        .initial_delay
        .saturating_mul(2u32.saturating_pow(attempt));
    let capped = base.min(config.max_delay);

    if config.jitter {
        // Simple jitter: pick a duration in [capped/2, capped] using system
        // time nanoseconds as a cheap entropy source (no external rand crate).
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let half = capped / 2;
        let jitter_range_nanos = capped.as_nanos().saturating_sub(half.as_nanos());
        if jitter_range_nanos == 0 {
            return capped;
        }
        let offset_nanos = (nanos as u128) % jitter_range_nanos;
        half + Duration::from_nanos(offset_nanos as u64)
    } else {
        capped
    }
}

// ── Retry with reconnection ───────────────────────────────────────────

/// Execute `op`, retrying with reconnection on retryable errors.
///
/// Between retry attempts, calls [`Reconnectable::reconnect()`] on `inner`
/// to re-establish the connection. Reconnection errors are silently ignored
/// — the next operation attempt may still succeed or produce a more specific
/// error.
fn retry_op_with_reconnect<T, P, F>(
    config: &RetryConfig,
    inner: &mut P,
    mut op: F,
) -> Result<T, PrintError>
where
    P: Reconnectable,
    F: FnMut(&mut P) -> Result<T, PrintError>,
{
    if config.max_attempts == 0 {
        return Err(PrintError::InvalidConfig(
            "max_attempts must be >= 1".into(),
        ));
    }

    let mut last_error: Option<PrintError> = None;

    for attempt in 0..config.max_attempts {
        match op(inner) {
            Ok(val) => return Ok(val),
            Err(e) => {
                if !e.is_retryable() {
                    return Err(e);
                }
                last_error = Some(e);

                // Don't sleep or reconnect after the last attempt.
                if attempt + 1 < config.max_attempts {
                    let delay = compute_delay(config, attempt);
                    std::thread::sleep(delay);
                    // Best-effort reconnection before the next retry.
                    let _ = inner.reconnect();
                }
            }
        }
    }

    // We only reach here when every attempt failed with a retryable error.
    Err(PrintError::RetriesExhausted {
        attempts: config.max_attempts,
        last_error: Box::new(
            last_error.unwrap_or_else(|| {
                unreachable!("at least one attempt was made (max_attempts >= 1)")
            }),
        ),
    })
}

/// A retry wrapper that **reconnects** between attempts.
///
/// Unlike [`RetryPrinter`], which retries on the same (possibly broken)
/// connection, `ReconnectRetryPrinter` calls
/// [`Reconnectable::reconnect()`] before each retry attempt.
/// This makes it effective for recovering from full connection drops
/// (TCP disconnect, USB unplug, serial port reset).
///
/// # Example
///
/// ```rust,no_run
/// use zpl_toolchain_print_client::{
///     TcpPrinter, ReconnectRetryPrinter, Printer, RetryConfig, PrinterConfig,
/// };
///
/// let tcp = TcpPrinter::connect("192.168.1.100:9100", PrinterConfig::default()).unwrap();
/// let mut printer = ReconnectRetryPrinter::new(tcp, RetryConfig::default());
/// printer.send_zpl("^XA^FDHello^FS^XZ").unwrap();
/// ```
pub struct ReconnectRetryPrinter<P> {
    inner: P,
    retry_config: RetryConfig,
}

impl<P> ReconnectRetryPrinter<P> {
    /// Create a new retry-with-reconnect wrapper.
    pub fn new(inner: P, retry_config: RetryConfig) -> Self {
        Self {
            inner,
            retry_config,
        }
    }

    /// Unwrap the wrapper, returning the inner printer.
    pub fn into_inner(self) -> P {
        self.inner
    }

    /// Get a mutable reference to the inner printer.
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

impl<P: Printer + Reconnectable> Printer for ReconnectRetryPrinter<P> {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
        retry_op_with_reconnect(&self.retry_config, &mut self.inner, |p| p.send_raw(data))
    }
}

impl<P: StatusQuery + Reconnectable> StatusQuery for ReconnectRetryPrinter<P> {
    fn query_raw(&mut self, cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
        retry_op_with_reconnect(&self.retry_config, &mut self.inner, |p| p.query_raw(cmd))
    }
}

impl<P: Reconnectable> Reconnectable for ReconnectRetryPrinter<P> {
    fn reconnect(&mut self) -> Result<(), PrintError> {
        self.inner.reconnect()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // -- Mock printer ---------------------------------------------------

    /// A mock printer that returns errors from a pre-loaded sequence, then
    /// succeeds with `Ok(())`.
    struct MockPrinter {
        /// Remaining results to return (popped from the front).
        send_results: Arc<Mutex<Vec<Result<(), PrintError>>>>,
        /// Count of calls to `send_raw`.
        send_call_count: Arc<Mutex<u32>>,
    }

    impl MockPrinter {
        fn new(results: Vec<Result<(), PrintError>>) -> Self {
            Self {
                send_results: Arc::new(Mutex::new(results)),
                send_call_count: Arc::new(Mutex::new(0)),
            }
        }

        fn call_count(&self) -> u32 {
            *self.send_call_count.lock().unwrap()
        }
    }

    impl Printer for MockPrinter {
        fn send_raw(&mut self, _data: &[u8]) -> Result<(), PrintError> {
            *self.send_call_count.lock().unwrap() += 1;
            let mut results = self.send_results.lock().unwrap();
            if results.is_empty() {
                Ok(())
            } else {
                results.remove(0)
            }
        }
    }

    fn retryable_error() -> PrintError {
        PrintError::WriteFailed(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "mock write error",
        ))
    }

    fn non_retryable_error() -> PrintError {
        PrintError::InvalidAddress("bad-address".into())
    }

    fn fast_retry_config(max_attempts: u32) -> RetryConfig {
        RetryConfig {
            max_attempts,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            jitter: false,
            ..RetryConfig::default()
        }
    }

    // -- Tests ----------------------------------------------------------

    #[test]
    fn non_retryable_error_returned_immediately() {
        let mock = MockPrinter::new(vec![Err(non_retryable_error())]);
        let mut printer = RetryPrinter::new(mock, fast_retry_config(3));

        let result = printer.send_raw(b"^XA^XZ");
        assert!(result.is_err());

        // Should have been called exactly once — no retries.
        assert_eq!(printer.inner.call_count(), 1);

        // The error should be the original, NOT RetriesExhausted.
        match result.unwrap_err() {
            PrintError::InvalidAddress(addr) => assert_eq!(addr, "bad-address"),
            other => panic!("expected InvalidAddress, got: {other:?}"),
        }
    }

    #[test]
    fn retryable_error_retried_up_to_max_attempts() {
        let mock = MockPrinter::new(vec![
            Err(retryable_error()),
            Err(retryable_error()),
            Err(retryable_error()),
        ]);
        let mut printer = RetryPrinter::new(mock, fast_retry_config(3));

        let result = printer.send_raw(b"^XA^XZ");
        assert!(result.is_err());

        // All 3 attempts should have been made.
        assert_eq!(printer.inner.call_count(), 3);

        match result.unwrap_err() {
            PrintError::RetriesExhausted {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 3);
                assert!(matches!(*last_error, PrintError::WriteFailed(_)));
            }
            other => panic!("expected RetriesExhausted, got: {other:?}"),
        }
    }

    #[test]
    fn succeeds_on_retry() {
        // Fail twice, then succeed on the third attempt.
        let mock = MockPrinter::new(vec![Err(retryable_error()), Err(retryable_error())]);
        let mut printer = RetryPrinter::new(mock, fast_retry_config(5));

        let result = printer.send_raw(b"^XA^XZ");
        assert!(result.is_ok());

        // Two failures + one success = 3 calls.
        assert_eq!(printer.inner.call_count(), 3);
    }

    #[test]
    fn into_inner_returns_wrapped_printer() {
        let mock = MockPrinter::new(vec![]);
        let printer = RetryPrinter::new(mock, fast_retry_config(1));
        let inner = printer.into_inner();
        assert_eq!(inner.call_count(), 0);
    }

    #[test]
    fn compute_delay_respects_max_delay() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            jitter: false,
            ..RetryConfig::default()
        };

        // attempt 0: min(1*1, 5) = 1s
        assert_eq!(compute_delay(&config, 0), Duration::from_secs(1));
        // attempt 1: min(1*2, 5) = 2s
        assert_eq!(compute_delay(&config, 1), Duration::from_secs(2));
        // attempt 2: min(1*4, 5) = 4s
        assert_eq!(compute_delay(&config, 2), Duration::from_secs(4));
        // attempt 3: min(1*8, 5) = 5s (capped)
        assert_eq!(compute_delay(&config, 3), Duration::from_secs(5));
        // attempt 10: still capped at 5s
        assert_eq!(compute_delay(&config, 10), Duration::from_secs(5));
    }

    #[test]
    fn max_attempts_zero_returns_error() {
        let mock = MockPrinter::new(vec![]);
        let mut printer = RetryPrinter::new(mock, fast_retry_config(0));
        let result = printer.send_raw(b"test");
        assert!(result.is_err());
        match result.unwrap_err() {
            PrintError::InvalidConfig(msg) => {
                assert!(
                    msg.contains("max_attempts"),
                    "expected max_attempts error, got: {msg}"
                );
            }
            other => panic!("expected InvalidConfig, got: {other:?}"),
        }
        // Should not have called send_raw at all.
        assert_eq!(printer.inner.call_count(), 0);
    }

    #[test]
    fn max_attempts_one_no_retry() {
        let mock = MockPrinter::new(vec![Err(retryable_error())]);
        let mut printer = RetryPrinter::new(mock, fast_retry_config(1));
        let result = printer.send_raw(b"test");
        assert!(result.is_err());
        match result.unwrap_err() {
            PrintError::RetriesExhausted { attempts, .. } => assert_eq!(attempts, 1),
            other => panic!("expected RetriesExhausted, got: {other:?}"),
        }
        assert_eq!(printer.inner.call_count(), 1);
    }

    #[test]
    fn compute_delay_with_jitter_stays_in_range() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            jitter: true,
            ..RetryConfig::default()
        };

        // Run several times — the jittered delay should always be in [base/2, base].
        for attempt in 0..4 {
            let base = config
                .initial_delay
                .saturating_mul(2u32.saturating_pow(attempt))
                .min(config.max_delay);
            let half = base / 2;

            for _ in 0..10 {
                let d = compute_delay(&config, attempt);
                assert!(
                    d >= half && d <= base,
                    "attempt {attempt}: delay {d:?} not in [{half:?}, {base:?}]",
                );
            }
        }
    }

    // -- ReconnectRetryPrinter tests ------------------------------------

    /// A mock printer that tracks reconnect and send calls.
    struct ReconnectMock {
        send_results: Vec<Result<(), PrintError>>,
        reconnect_count: Arc<Mutex<u32>>,
        send_count: Arc<Mutex<u32>>,
    }

    impl ReconnectMock {
        fn new(
            send_results: Vec<Result<(), PrintError>>,
            reconnect_count: Arc<Mutex<u32>>,
            send_count: Arc<Mutex<u32>>,
        ) -> Self {
            Self {
                send_results,
                reconnect_count,
                send_count,
            }
        }
    }

    impl Printer for ReconnectMock {
        fn send_raw(&mut self, _data: &[u8]) -> Result<(), PrintError> {
            *self.send_count.lock().unwrap() += 1;
            if self.send_results.is_empty() {
                Ok(())
            } else {
                self.send_results.remove(0)
            }
        }
    }

    impl Reconnectable for ReconnectMock {
        fn reconnect(&mut self) -> Result<(), PrintError> {
            *self.reconnect_count.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn reconnect_retry_calls_reconnect_between_attempts() {
        let reconnect_count = Arc::new(Mutex::new(0u32));
        let send_count = Arc::new(Mutex::new(0u32));

        let mock = ReconnectMock::new(
            vec![Err(retryable_error()), Err(retryable_error())],
            Arc::clone(&reconnect_count),
            Arc::clone(&send_count),
        );

        let mut printer = ReconnectRetryPrinter::new(mock, fast_retry_config(5));
        let result = printer.send_raw(b"^XA^XZ");
        assert!(result.is_ok());

        // 3 send attempts: 2 failures + 1 success
        assert_eq!(*send_count.lock().unwrap(), 3);
        // 2 reconnects: one before each retry (not before the first attempt)
        assert_eq!(*reconnect_count.lock().unwrap(), 2);
    }

    #[test]
    fn reconnect_retry_non_retryable_error_no_reconnect() {
        let reconnect_count = Arc::new(Mutex::new(0u32));
        let send_count = Arc::new(Mutex::new(0u32));

        let mock = ReconnectMock::new(
            vec![Err(non_retryable_error())],
            Arc::clone(&reconnect_count),
            Arc::clone(&send_count),
        );

        let mut printer = ReconnectRetryPrinter::new(mock, fast_retry_config(3));
        let result = printer.send_raw(b"^XA^XZ");
        assert!(result.is_err());

        // Non-retryable error: no reconnect calls
        assert_eq!(*reconnect_count.lock().unwrap(), 0);
    }

    #[test]
    fn reconnect_retry_exhausted_reports_all_attempts() {
        let reconnect_count = Arc::new(Mutex::new(0u32));
        let send_count = Arc::new(Mutex::new(0u32));

        let mock = ReconnectMock::new(
            vec![
                Err(retryable_error()),
                Err(retryable_error()),
                Err(retryable_error()),
            ],
            Arc::clone(&reconnect_count),
            Arc::clone(&send_count),
        );

        let mut printer = ReconnectRetryPrinter::new(mock, fast_retry_config(3));
        let result = printer.send_raw(b"^XA^XZ");
        assert!(result.is_err());

        match result.unwrap_err() {
            PrintError::RetriesExhausted {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 3);
                assert!(matches!(*last_error, PrintError::WriteFailed(_)));
            }
            other => panic!("expected RetriesExhausted, got: {other:?}"),
        }

        // All 3 attempts made, 2 reconnects (not after last failure)
        assert_eq!(*send_count.lock().unwrap(), 3);
        assert_eq!(*reconnect_count.lock().unwrap(), 2);
    }

    #[test]
    fn reconnect_retry_into_inner() {
        let reconnect_count = Arc::new(Mutex::new(0u32));
        let send_count = Arc::new(Mutex::new(0u32));

        let mock = ReconnectMock::new(vec![], reconnect_count, Arc::clone(&send_count));

        let printer = ReconnectRetryPrinter::new(mock, fast_retry_config(1));
        let inner = printer.into_inner();
        assert_eq!(*inner.send_count.lock().unwrap(), 0);
    }
}
