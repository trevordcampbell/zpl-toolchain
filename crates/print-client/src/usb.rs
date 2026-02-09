//! USB transport for Zebra printers using the `nusb` crate.
//!
//! Feature-gated behind `usb` — only compiled when `--features usb` is active.
//!
//! Zebra printers expose a USB Printer class (bInterfaceClass = 7) interface
//! with a bulk OUT endpoint for sending ZPL and an optional bulk IN endpoint
//! for reading status responses.

use futures_lite::future::block_on;
use nusb::transfer::{Direction, EndpointType, RequestBuffer};

use crate::frame::{DEFAULT_MAX_FRAME_SIZE, expected_frame_count, read_frames};
use crate::{PrintError, Printer, PrinterConfig, StatusQuery};

/// Zebra Technologies USB Vendor ID.
const ZEBRA_VENDOR_ID: u16 = 0x0A5F;

/// USB Printer class code (bInterfaceClass).
const USB_CLASS_PRINTER: u8 = 7;

/// USB printer connected via USB bulk transfer.
///
/// Discovered automatically by vendor ID or by explicit VID:PID.
/// The struct holds a claimed interface and the discovered endpoint addresses.
pub struct UsbPrinter {
    /// Claimed USB interface handle.
    interface: nusb::Interface,
    /// Bulk OUT endpoint address for sending data to the printer.
    ep_out: u8,
    /// Bulk IN endpoint address for reading responses (if available).
    ep_in: Option<u8>,
    /// Printer configuration (timeouts, retry settings).
    config: PrinterConfig,
}

impl UsbPrinter {
    /// Find the first Zebra printer (VID `0x0A5F`) with a USB Printer class interface.
    ///
    /// This is the most common way to connect — just plug in a Zebra printer
    /// and call this method.
    ///
    /// # Errors
    ///
    /// Returns `PrintError::UsbDeviceNotFound` if no matching device is found.
    /// Returns `PrintError::UsbError` if the device cannot be opened or claimed.
    pub fn find_zebra(config: PrinterConfig) -> Result<Self, PrintError> {
        let devices = nusb::list_devices().map_err(|e| PrintError::UsbError(e.to_string()))?;

        for dev_info in devices {
            if dev_info.vendor_id() != ZEBRA_VENDOR_ID {
                continue;
            }

            // Look for an interface with USB Printer class
            let iface_number = dev_info
                .interfaces()
                .find(|iface| iface.class() == USB_CLASS_PRINTER)
                .map(|iface| iface.interface_number());

            if let Some(iface_number) = iface_number {
                return Self::open_device(&dev_info, iface_number, config);
            }
        }

        Err(PrintError::UsbDeviceNotFound)
    }

    /// Find a USB printer by specific vendor ID and product ID.
    ///
    /// Use this when you have multiple printers or a non-Zebra ZPL-compatible device.
    ///
    /// # Errors
    ///
    /// Returns `PrintError::UsbDeviceNotFound` if no device with the given VID:PID is found.
    /// Returns `PrintError::UsbError` if the device cannot be opened or claimed.
    pub fn find(
        vendor_id: u16,
        product_id: u16,
        config: PrinterConfig,
    ) -> Result<Self, PrintError> {
        let devices = nusb::list_devices().map_err(|e| PrintError::UsbError(e.to_string()))?;

        for dev_info in devices {
            if dev_info.vendor_id() == vendor_id && dev_info.product_id() == product_id {
                let iface_number = dev_info
                    .interfaces()
                    .find(|iface| iface.class() == USB_CLASS_PRINTER)
                    .map(|iface| iface.interface_number())
                    .ok_or_else(|| {
                        PrintError::UsbError(format!(
                            "device {:04X}:{:04X} has no printer-class interface",
                            vendor_id, product_id
                        ))
                    })?;

                return Self::open_device(&dev_info, iface_number, config);
            }
        }

        Err(PrintError::UsbDeviceNotFound)
    }

    /// List all USB devices currently connected.
    ///
    /// Returns a vector of `(vendor_id, product_id, description)` tuples.
    /// Useful for discovery and debugging.
    pub fn list_devices() -> Vec<(u16, u16, String)> {
        let Ok(devices) = nusb::list_devices() else {
            return Vec::new();
        };

        devices
            .map(|dev| {
                let desc = dev.product_string().unwrap_or_default().to_string();
                let desc = if desc.is_empty() {
                    format!(
                        "{} (VID:{:04X} PID:{:04X})",
                        dev.manufacturer_string().unwrap_or_default(),
                        dev.vendor_id(),
                        dev.product_id()
                    )
                } else {
                    desc
                };
                (dev.vendor_id(), dev.product_id(), desc)
            })
            .collect()
    }

    /// Open a specific USB device and claim the given interface.
    ///
    /// Discovers bulk IN/OUT endpoints from the device's active configuration
    /// descriptors. On Linux, detaches any kernel driver (e.g., `usblp`) before
    /// claiming.
    fn open_device(
        dev_info: &nusb::DeviceInfo,
        interface_number: u8,
        config: PrinterConfig,
    ) -> Result<Self, PrintError> {
        let device = dev_info
            .open()
            .map_err(|e| PrintError::UsbError(format!("failed to open device: {}", e)))?;

        // Discover endpoints from the active configuration descriptor before
        // claiming, since we need the Device handle for descriptor access.
        let (ep_out, ep_in) = Self::discover_endpoints(&device, interface_number)?;

        // On Linux, the `usblp` kernel driver may hold the interface.
        // `detach_and_claim_interface` detaches it first, then claims.
        let interface = device
            .detach_and_claim_interface(interface_number)
            .map_err(|e| {
                PrintError::UsbError(format!(
                    "failed to claim interface {}: {}",
                    interface_number, e
                ))
            })?;

        Ok(Self {
            interface,
            ep_out,
            ep_in,
            config,
        })
    }

    /// Walk the active configuration's interface descriptors to find bulk
    /// OUT and optional bulk IN endpoints.
    fn discover_endpoints(
        device: &nusb::Device,
        interface_number: u8,
    ) -> Result<(u8, Option<u8>), PrintError> {
        let config = device.active_configuration().map_err(|e| {
            PrintError::UsbError(format!("failed to read active configuration: {}", e))
        })?;

        let mut ep_out: Option<u8> = None;
        let mut ep_in: Option<u8> = None;

        // Find our interface in the configuration descriptors
        for alt_setting in config.interface_alt_settings() {
            if alt_setting.interface_number() != interface_number {
                continue;
            }
            // Use the first (default) alternate setting
            if alt_setting.alternate_setting() != 0 {
                continue;
            }

            for ep in alt_setting.endpoints() {
                if ep.transfer_type() != EndpointType::Bulk {
                    continue;
                }
                match ep.direction() {
                    Direction::Out => {
                        if ep_out.is_none() {
                            ep_out = Some(ep.address());
                        }
                    }
                    Direction::In => {
                        if ep_in.is_none() {
                            ep_in = Some(ep.address());
                        }
                    }
                }
            }
            break;
        }

        let ep_out = ep_out.ok_or_else(|| {
            PrintError::UsbError("no bulk OUT endpoint found on printer interface".into())
        })?;

        Ok((ep_out, ep_in))
    }

    /// Perform a bulk OUT transfer (send data to the printer).
    ///
    /// Uses `nusb`'s async API with `block_on` for synchronous operation.
    fn bulk_write(&self, data: &[u8]) -> Result<(), PrintError> {
        let future = self.interface.bulk_out(self.ep_out, data.to_vec());
        let completion = block_on(future);

        completion.status.map_err(|e| {
            PrintError::WriteFailed(std::io::Error::other(format!("USB bulk OUT: {}", e)))
        })
    }
}

impl Printer for UsbPrinter {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), PrintError> {
        self.bulk_write(data)
    }
}

impl StatusQuery for UsbPrinter {
    fn query_raw(&mut self, cmd: &[u8]) -> Result<Vec<Vec<u8>>, PrintError> {
        let Some(ep_in) = self.ep_in else {
            return Err(PrintError::UsbError(
                "no bulk IN endpoint — printer does not support status queries".into(),
            ));
        };

        // Send the query command
        self.bulk_write(cmd)?;

        // Read the response. Zebra printers respond with STX/ETX framed data.
        let expected_frames = expected_frame_count(cmd);
        let timeout = self.config.timeouts.read;

        // Create a reader adapter over bulk IN transfers for the frame parser.
        let mut reader = UsbBulkReader {
            interface: &self.interface,
            ep_in,
            buffer: Vec::new(),
            pos: 0,
        };

        read_frames(
            &mut reader,
            expected_frames,
            timeout,
            DEFAULT_MAX_FRAME_SIZE,
        )
    }
}

/// Adapter that implements `std::io::Read` over USB bulk IN transfers.
///
/// This allows the `read_frames` parser to work transparently with USB,
/// just as it does with TCP streams and serial ports.
///
/// # Timeout behaviour
///
/// Individual bulk IN transfers are dispatched through the OS USB stack via
/// `nusb` and completed with `block_on()`. The OS USB host controller
/// provides its own timeout for pending bulk transfers (typically 5–30 s
/// depending on the platform). The higher-level `read_frames` function
/// imposes its own wall-clock deadline and will return `ReadTimeout` when
/// the deadline elapses *between* calls to `read()`. In the unlikely event
/// that a single `read()` call blocks longer than the `read_frames`
/// deadline (e.g., device firmware hang with no OS-level USB timeout), the
/// thread will block until the OS times out the transfer.
struct UsbBulkReader<'a> {
    interface: &'a nusb::Interface,
    ep_in: u8,
    buffer: Vec<u8>,
    pos: usize,
}

impl std::io::Read for UsbBulkReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // If we have buffered data from a previous bulk transfer, return that first.
        if self.pos < self.buffer.len() {
            let available = &self.buffer[self.pos..];
            let to_copy = available.len().min(buf.len());
            buf[..to_copy].copy_from_slice(&available[..to_copy]);
            self.pos += to_copy;
            return Ok(to_copy);
        }

        // Perform a new bulk IN transfer.
        let request = RequestBuffer::new(512);
        let future = self.interface.bulk_in(self.ep_in, request);
        let completion = block_on(future);

        match completion.status {
            Ok(()) => {
                let data = completion.data;
                if data.is_empty() {
                    // A zero-length packet (ZLP) can occur at USB packet
                    // boundaries and does not indicate EOF. Return TimedOut
                    // so the caller (read_frames) retries instead of treating
                    // this as a closed connection.
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "zero-length USB transfer (retrying)",
                    ));
                }
                let to_copy = data.len().min(buf.len());
                buf[..to_copy].copy_from_slice(&data[..to_copy]);

                // Buffer any remaining data for the next read call.
                if to_copy < data.len() {
                    self.buffer = data;
                    self.pos = to_copy;
                } else {
                    self.buffer.clear();
                    self.pos = 0;
                }

                Ok(to_copy)
            }
            Err(e) => Err(std::io::Error::other(e)),
        }
    }
}
