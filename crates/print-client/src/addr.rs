//! Printer address resolution.
//!
//! Handles the various formats users pass as printer addresses:
//! `IP`, `IP:PORT`, `hostname`, `hostname:PORT`. Defaults to port 9100.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use crate::PrintError;

/// Default ZPL raw printing port (JetDirect / RAW).
pub const DEFAULT_PORT: u16 = 9100;

/// Resolve a user-provided printer address string to a `SocketAddr`.
///
/// Accepts these formats:
/// - `192.168.1.55:9100` -- IP with explicit port
/// - `192.168.1.55` -- IP without port (defaults to 9100)
/// - `printer01.local:9100` -- hostname with port
/// - `printer01.local` -- hostname without port (defaults to 9100)
///
/// Returns the first resolved address. For hostnames that resolve to
/// multiple addresses (dual-stack), the first result is used.
pub fn resolve_printer_addr(input: &str) -> Result<SocketAddr, PrintError> {
    // 1. Try as SocketAddr (e.g., "192.168.1.55:9100" or "[::1]:9100")
    if let Ok(addr) = input.parse::<SocketAddr>() {
        return Ok(addr);
    }

    // 2. Try as bare IP without port (e.g., "192.168.1.55")
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_PORT));
    }

    // 3. Try as host:port (e.g., "printer01.local:9100")
    if let Ok(mut addrs) = input.to_socket_addrs()
        && let Some(addr) = addrs.next()
    {
        return Ok(addr);
    }

    // 4. Try as hostname without port (e.g., "printer01.local")
    if let Ok(mut addrs) = (input, DEFAULT_PORT).to_socket_addrs()
        && let Some(addr) = addrs.next()
    {
        return Ok(addr);
    }

    // At this point the input is not a valid IP (steps 1-2 failed) and DNS
    // resolution found no addresses (steps 3-4 failed).
    Err(PrintError::NoAddressFound(input.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_with_port() {
        let addr = resolve_printer_addr("192.168.1.55:9100").unwrap();
        assert_eq!(addr.ip().to_string(), "192.168.1.55");
        assert_eq!(addr.port(), 9100);
    }

    #[test]
    fn test_ip_with_custom_port() {
        let addr = resolve_printer_addr("10.0.0.1:6101").unwrap();
        assert_eq!(addr.ip().to_string(), "10.0.0.1");
        assert_eq!(addr.port(), 6101);
    }

    #[test]
    fn test_ip_without_port_defaults_to_9100() {
        let addr = resolve_printer_addr("192.168.1.55").unwrap();
        assert_eq!(addr.ip().to_string(), "192.168.1.55");
        assert_eq!(addr.port(), DEFAULT_PORT);
    }

    #[test]
    fn test_ipv6_with_port() {
        let addr = resolve_printer_addr("[::1]:9100").unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 9100);
    }

    #[test]
    fn test_ipv6_without_port() {
        let addr = resolve_printer_addr("::1").unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), DEFAULT_PORT);
    }

    #[test]
    fn test_localhost_with_port() {
        let addr = resolve_printer_addr("localhost:9100").unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 9100);
    }

    #[test]
    fn test_localhost_without_port() {
        let addr = resolve_printer_addr("localhost").unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), DEFAULT_PORT);
    }

    #[test]
    fn test_unresolvable_hostname() {
        let result = resolve_printer_addr("no-such-host.invalid");
        assert!(result.is_err());
        match result.unwrap_err() {
            PrintError::NoAddressFound(s) => assert_eq!(s, "no-such-host.invalid"),
            other => panic!("expected NoAddressFound, got {:?}", other),
        }
    }

    #[test]
    fn test_garbage_input() {
        let result = resolve_printer_addr("not a valid address!!!");
        assert!(result.is_err());
        match result.unwrap_err() {
            PrintError::NoAddressFound(s) => assert_eq!(s, "not a valid address!!!"),
            other => panic!("expected NoAddressFound, got {:?}", other),
        }
    }
}
