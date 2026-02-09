//! TCP transport for ZPL printers (port 9100 / JetDirect / RAW).
//!
//! Provides [`TcpPrinter`], a synchronous TCP transport that implements
//! both [`Printer`] (send ZPL) and [`StatusQuery`] (query ~HS / ~HI).

use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};

use crate::addr::resolve_printer_addr;
use crate::frame::{DEFAULT_MAX_FRAME_SIZE, expected_frame_count, read_frames};
use crate::{PrintError, Printer, PrinterConfig, StatusQuery};

/// A synchronous TCP connection to a ZPL printer.
///
/// Connects to the printer's RAW port (typically 9100) and sends ZPL
/// commands as raw bytes. Supports bidirectional communication for
/// status queries (`~HS`, `~HI`).
pub struct TcpPrinter {
    stream: TcpStream,
    config: PrinterConfig,
    addr: SocketAddr,
}

impl TcpPrinter {
    /// Connect to a printer at the given address.
    ///
    /// The address can be any format accepted by [`resolve_printer_addr`]:
    /// `IP`, `IP:PORT`, `hostname`, `hostname:PORT`. Port defaults to 9100.
    ///
    /// Configures the socket with TCP_NODELAY, TCP keepalive (60s interval),
    /// and the write/read timeouts from [`PrinterConfig`].
    pub fn connect(addr: &str, config: PrinterConfig) -> Result<Self, PrintError> {
        let socket_addr = resolve_printer_addr(addr)?;

        // Connect with timeout
        let stream = Self::open_stream(&socket_addr, &config)?;

        Ok(Self {
            stream,
            config,
            addr: socket_addr,
        })
    }

    /// Re-establish the TCP connection after a drop or error.
    ///
    /// Shuts down the old stream (ignoring errors) and opens a fresh
    /// connection to the same address with the same configuration.
    pub fn reconnect(&mut self) -> Result<(), PrintError> {
        // Best-effort shutdown of the old stream
        let _ = self.stream.shutdown(Shutdown::Both);

        self.stream = Self::open_stream(&self.addr, &self.config)?;
        Ok(())
    }

    /// Open a TCP connection and configure the stream (nodelay, keepalive, timeouts).
    fn open_stream(addr: &SocketAddr, config: &PrinterConfig) -> Result<TcpStream, PrintError> {
        let stream =
            TcpStream::connect_timeout(addr, config.timeouts.connect).map_err(|e| {
                match e.kind() {
                    io::ErrorKind::ConnectionRefused => PrintError::ConnectionRefused {
                        addr: addr.to_string(),
                        source: e,
                    },
                    io::ErrorKind::TimedOut => PrintError::ConnectionTimeout {
                        addr: addr.to_string(),
                        timeout: config.timeouts.connect,
                        source: e,
                    },
                    _ => PrintError::ConnectionFailed {
                        addr: addr.to_string(),
                        source: e,
                    },
                }
            })?;

        configure_stream(&stream, addr, config)?;
        Ok(stream)
    }

    /// Return the resolved socket address this printer is connected to.
    pub fn remote_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Convenience wrapper around [`wait_for_completion()`].
    ///
    /// See the standalone function for full documentation.
    pub fn wait_for_completion(
        &mut self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<(), PrintError> {
        crate::wait_for_completion(self, poll_interval, timeout)
    }
}

impl Printer for TcpPrinter {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
        self.stream
            .write_all(data)
            .map_err(PrintError::WriteFailed)?;
        self.stream.flush().map_err(PrintError::WriteFailed)?;
        Ok(())
    }
}

impl StatusQuery for TcpPrinter {
    fn query_raw(&mut self, cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
        // Send the query command
        self.stream
            .write_all(cmd)
            .map_err(PrintError::WriteFailed)?;
        self.stream.flush().map_err(PrintError::WriteFailed)?;

        let expected_frames = expected_frame_count(cmd);

        read_frames(
            &mut self.stream,
            expected_frames,
            self.config.timeouts.read,
            DEFAULT_MAX_FRAME_SIZE,
        )
    }
}

impl Drop for TcpPrinter {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl crate::Reconnectable for TcpPrinter {
    fn reconnect(&mut self) -> Result<(), crate::PrintError> {
        TcpPrinter::reconnect(self)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Configure TCP_NODELAY, keepalive, and read/write timeouts on a stream.
fn configure_stream(
    stream: &TcpStream,
    addr: &SocketAddr,
    config: &PrinterConfig,
) -> Result<(), PrintError> {
    // TCP_NODELAY -- disable Nagle's algorithm for low-latency sends
    stream
        .set_nodelay(true)
        .map_err(|e| PrintError::ConnectionFailed {
            addr: addr.to_string(),
            source: e,
        })?;

    // TCP keepalive via socket2 (60 second interval)
    configure_keepalive(stream, Duration::from_secs(60)).map_err(|e| {
        PrintError::ConnectionFailed {
            addr: addr.to_string(),
            source: e,
        }
    })?;

    // Write timeout
    stream
        .set_write_timeout(Some(config.timeouts.write))
        .map_err(|e| PrintError::ConnectionFailed {
            addr: addr.to_string(),
            source: e,
        })?;

    // Read timeout
    stream
        .set_read_timeout(Some(config.timeouts.read))
        .map_err(|e| PrintError::ConnectionFailed {
            addr: addr.to_string(),
            source: e,
        })?;

    Ok(())
}

/// Configure TCP keepalive on a `TcpStream` via `socket2`.
fn configure_keepalive(stream: &TcpStream, interval: Duration) -> io::Result<()> {
    let keepalive = TcpKeepalive::new().with_time(interval);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let keepalive = keepalive.with_interval(interval);

    SockRef::from(stream).set_tcp_keepalive(&keepalive)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::frame::expected_frame_count;

    #[test]
    fn expected_frame_count_for_commands() {
        assert_eq!(expected_frame_count(b"~HS"), 3);
        assert_eq!(expected_frame_count(b"~HI"), 1);
        assert_eq!(expected_frame_count(b"~HD"), 1);
    }
}
