//! Integration tests for the print client — uses a mock TCP server.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread;
use std::time::Duration;

use zpl_toolchain_print_client::{PrintError, Printer, PrinterConfig, StatusQuery, TcpPrinter};

// ── Mock printer server ─────────────────────────────────────────────────

/// A mock printer that runs on a background thread, accepts one connection,
/// receives data, and optionally sends back a canned response.
struct MockPrinterServer {
    addr: SocketAddr,
    handle: Option<thread::JoinHandle<Vec<u8>>>,
}

impl MockPrinterServer {
    /// Spawn a mock server that receives all data and optionally sends a response.
    fn start(response: Option<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();

            let mut received = Vec::new();
            let mut buf = [0u8; 4096];

            // Read until connection closes or timeout (for one-shot sends)
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        received.extend_from_slice(&buf[..n]);
                        // If we have a response to send, check if we've received a query
                        if let Some(ref resp) = response {
                            // Check for ~HS or ~HI command
                            if received.ends_with(b"~HS") || received.ends_with(b"~HI") {
                                stream.write_all(resp).unwrap();
                                stream.flush().unwrap();
                                // Keep reading until the client closes
                            }
                        }
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(_) => break,
                }
            }

            received
        });

        Self {
            addr,
            handle: Some(handle),
        }
    }

    /// Wait for the mock server thread to finish and return the received data.
    fn received_data(mut self) -> Vec<u8> {
        self.handle.take().unwrap().join().unwrap()
    }
}

fn fast_config() -> PrinterConfig {
    let mut cfg = PrinterConfig::default();
    cfg.timeouts.connect = Duration::from_secs(2);
    cfg.timeouts.write = Duration::from_secs(2);
    cfg.timeouts.read = Duration::from_secs(2);
    cfg
}

/// Build a realistic ~HS mock response (3 STX/ETX frames).
fn mock_hs_response() -> Vec<u8> {
    let mut data = Vec::new();
    // Line 1: 12 fields
    data.push(0x02);
    data.extend_from_slice(b"030,0,0,1245,000,0,0,0,000,0,0,0");
    data.push(0x03);
    data.extend_from_slice(b"\r\n");
    // Line 2: 10 fields
    data.push(0x02);
    data.extend_from_slice(b"000,0,0,0,0,2,4,0,00000000,1,000");
    data.push(0x03);
    data.extend_from_slice(b"\r\n");
    // Line 3: 2 fields
    data.push(0x02);
    data.extend_from_slice(b"1234,0");
    data.push(0x03);
    data
}

/// Build a realistic ~HI mock response (1 STX/ETX frame).
fn mock_hi_response() -> Vec<u8> {
    let mut data = Vec::new();
    data.push(0x02);
    data.extend_from_slice(b"ZD421-300dpi,V84.20.18,8,8192");
    data.push(0x03);
    data
}

// ── Tests ────────────────────────────────────────────────────────────────

#[test]
fn connect_and_send_zpl() {
    let server = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    printer.send_zpl("^XA^FDHello^FS^XZ").unwrap();
    drop(printer);

    let received = server.received_data();
    assert_eq!(received, b"^XA^FDHello^FS^XZ");
}

#[test]
fn send_multiple_labels() {
    let server = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    printer.send_zpl("^XA^FDLabel1^FS^XZ").unwrap();
    printer.send_zpl("^XA^FDLabel2^FS^XZ").unwrap();
    drop(printer);

    let received = server.received_data();
    assert_eq!(received, b"^XA^FDLabel1^FS^XZ^XA^FDLabel2^FS^XZ");
}

#[test]
fn send_raw_bytes() {
    let server = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let raw = b"\x02raw\x03";
    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    printer.send_raw(raw).unwrap();
    drop(printer);

    let received = server.received_data();
    assert_eq!(received, raw);
}

#[test]
fn query_status_parses_hs_response() {
    let server = MockPrinterServer::start(Some(mock_hs_response()));
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    let status = printer.query_status().unwrap();

    assert!(!status.paper_out);
    assert!(!status.paused);
    assert!(!status.head_up);
    assert_eq!(status.label_length_dots, 1245);
    assert_eq!(status.formats_in_buffer, 0);
    assert!(!status.ribbon_out);
    assert_eq!(status.labels_remaining, 0);
}

#[test]
fn query_info_parses_hi_response() {
    let server = MockPrinterServer::start(Some(mock_hi_response()));
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    let info = printer.query_info().unwrap();

    assert_eq!(info.model, "ZD421-300dpi");
    assert_eq!(info.firmware, "V84.20.18");
    assert_eq!(info.dpi, 8);
    assert_eq!(info.memory_kb, 8192);
}

#[test]
#[ignore] // Port reuse can be flaky in CI environments
fn reconnect_after_drop() {
    // First server accepts and closes
    let server1 = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server1.addr.port());

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    printer.send_zpl("^XA^FDFirst^FS^XZ").unwrap();

    let received = server1.received_data();
    assert_eq!(received, b"^XA^FDFirst^FS^XZ");

    // Start a second server on the same port
    let listener = TcpListener::bind(addr.as_str()).unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
        }
        buf
    });

    printer.reconnect().unwrap();
    printer.send_zpl("^XA^FDSecond^FS^XZ").unwrap();
    drop(printer);

    let received2 = handle.join().unwrap();
    assert_eq!(received2, b"^XA^FDSecond^FS^XZ");
}

#[test]
fn connect_to_nonexistent_server_fails() {
    // Use a port that's very likely not listening
    let result = TcpPrinter::connect("127.0.0.1:19999", fast_config());
    match result {
        Err(PrintError::ConnectionRefused { .. } | PrintError::ConnectionFailed { .. }) => {
            // expected
        }
        Err(other) => panic!("expected connection error, got: {other:?}"),
        Ok(_) => panic!("expected connection error, but connect succeeded"),
    }
}

#[test]
fn remote_addr_returns_connected_address() {
    let server = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    assert_eq!(printer.remote_addr(), server.addr);
}

#[test]
fn send_empty_zpl_is_noop() {
    let server = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    printer.send_zpl("").unwrap();
    drop(printer);

    let received = server.received_data();
    assert!(received.is_empty());
}

#[test]
fn send_large_payload() {
    let server = MockPrinterServer::start(None);
    let addr = format!("127.0.0.1:{}", server.addr.port());

    let large_data = "X".repeat(100_000);
    let zpl = format!(
        "^XA^GFA,{0},{0},100,{1}^FS^XZ",
        large_data.len(),
        large_data
    );

    let mut printer = TcpPrinter::connect(&addr, fast_config()).unwrap();
    printer.send_zpl(&zpl).unwrap();
    drop(printer);

    let received = server.received_data();
    assert_eq!(received.len(), zpl.len());
}
