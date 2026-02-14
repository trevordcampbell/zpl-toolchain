//! Zebra printer status response parser.
//!
//! Parses `~HS` (Host Status) and `~HI` (Host Identification) responses
//! from Zebra printers into typed Rust structs.

use crate::PrintError;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Parse a single numeric field from a comma-separated response.
fn parse_field<T: std::str::FromStr>(
    fields: &[&str],
    index: usize,
    line: u8,
) -> Result<T, PrintError> {
    let raw = fields
        .get(index)
        .ok_or_else(|| PrintError::MalformedFrame {
            details: format!(
                "~HS line {line}: expected field at index {index}, only got {} fields",
                fields.len()
            ),
        })?
        .trim();

    raw.parse::<T>().map_err(|_| PrintError::MalformedFrame {
        details: format!(
            "~HS line {line}: cannot parse field {index} ({raw:?}) as {}",
            std::any::type_name::<T>()
        ),
    })
}

/// Parse a boolean field: `"0"` → false, anything else → true.
fn parse_bool_field(fields: &[&str], index: usize, line: u8) -> Result<bool, PrintError> {
    let raw = fields
        .get(index)
        .ok_or_else(|| PrintError::MalformedFrame {
            details: format!(
                "~HS line {line}: expected field at index {index}, only got {} fields",
                fields.len()
            ),
        })?
        .trim();

    Ok(raw != "0")
}

/// Parse a memory field from `~HI` that may include unit suffixes.
///
/// Zebra responses are often plain integers (e.g. `131072`), but some models
/// include a suffix like `8176KB`. This parser accepts both forms.
fn parse_memory_kb_field(raw: &str) -> Result<u32, PrintError> {
    let trimmed = raw.trim();
    let digits_len = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits_len == 0 {
        return Err(PrintError::MalformedFrame {
            details: format!("~HI: cannot parse memory_kb ({trimmed:?})"),
        });
    }
    trimmed[..digits_len]
        .parse::<u32>()
        .map_err(|_| PrintError::MalformedFrame {
            details: format!("~HI: cannot parse memory_kb ({trimmed:?})"),
        })
}

// ── PrintMode ───────────────────────────────────────────────────────────

/// Zebra print mode, as reported in `~HS` line 2 field 4.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PrintMode {
    /// Tear-off mode (code 0) — labels are advanced past the tear bar.
    TearOff,
    /// Peel-off mode (code 1) — labels are peeled from the backing.
    PeelOff,
    /// Rewind mode (code 2) — labels are rewound onto a take-up spool.
    Rewind,
    /// Applicator mode (code 3) — labels are applied via an applicator device.
    Applicator,
    /// Cutter mode (code 4) — labels are cut after printing.
    Cutter,
    /// Delayed cutter mode (code 5) — cut is delayed until the next label starts.
    DelayedCutter,
    /// Linerless mode (code 6) — continuous media without a backing liner.
    Linerless,
}

impl PrintMode {
    /// Decode the numeric print-mode value from `~HS` line 2.
    fn from_code(code: u8) -> Result<Self, PrintError> {
        match code {
            0 => Ok(PrintMode::TearOff),
            1 => Ok(PrintMode::PeelOff),
            2 => Ok(PrintMode::Rewind),
            3 => Ok(PrintMode::Applicator),
            4 => Ok(PrintMode::Cutter),
            5 => Ok(PrintMode::DelayedCutter),
            6 => Ok(PrintMode::Linerless),
            _ => Err(PrintError::MalformedFrame {
                details: format!("unknown print mode code: {code}"),
            }),
        }
    }
}

// ── HostStatus ──────────────────────────────────────────────────────────

/// Parsed `~HS` (Host Status) response.
///
/// The Zebra `~HS` command returns three comma-separated lines
/// (each wrapped in STX/ETX framing). This struct contains every
/// field from all three lines.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HostStatus {
    // ── Line 1 ──────────────────────────────────────────────────────
    /// Communication settings (field 0).
    pub communication_flag: u32,
    /// Paper-out flag (field 1).
    pub paper_out: bool,
    /// Printer is paused (field 2).
    pub paused: bool,
    /// Label length in dots (field 3).
    pub label_length_dots: u32,
    /// Number of formats waiting in the receive buffer (field 4).
    pub formats_in_buffer: u32,
    /// Receive-buffer full (field 5).
    pub buffer_full: bool,
    /// Communications diagnostic mode active (field 6).
    pub comm_diag_mode: bool,
    /// Partial format in progress (field 7).
    pub partial_format: bool,
    /// Unused/reserved (field 8 — "000" on most printers).
    pub reserved_1: u32,
    /// Corrupt RAM detected (field 9).
    pub corrupt_ram: bool,
    /// Under-temperature condition (field 10).
    pub under_temperature: bool,
    /// Over-temperature condition (field 11).
    pub over_temperature: bool,

    // ── Line 2 ──────────────────────────────────────────────────────
    /// Function settings bitmask (field 0).
    pub function_settings: u32,
    /// Printhead-up flag (field 1).
    pub head_up: bool,
    /// Ribbon-out flag (field 2).
    pub ribbon_out: bool,
    /// Thermal-transfer mode (field 3).
    pub thermal_transfer_mode: bool,
    /// Current print mode (field 4).
    pub print_mode: PrintMode,
    /// Print-width mode (field 5).
    pub print_width_mode: u32,
    /// Label waiting to be taken (field 6).
    pub label_waiting: bool,
    /// Labels remaining in batch (field 7).
    pub labels_remaining: u32,
    /// Format while printing (field 8 — usually an 8-digit mask).
    pub format_while_printing: u32,
    /// Number of graphics stored in memory (field 9).
    pub graphics_stored_in_memory: u32,

    // ── Line 3 ──────────────────────────────────────────────────────
    /// Password value (field 0).
    pub password: u32,
    /// Static RAM installed flag (field 1).
    pub static_ram_installed: bool,
}

impl HostStatus {
    /// Parse a `~HS` response from STX/ETX frames.
    ///
    /// Expects exactly **3** frames (one per line of the `~HS` response).
    pub fn parse(frames: &[Vec<u8>]) -> Result<HostStatus, PrintError> {
        if frames.len() != 3 {
            return Err(PrintError::MalformedFrame {
                details: format!("~HS requires 3 frames, got {}", frames.len()),
            });
        }

        // ── Line 1 ─────────────────────────────────────────────────
        let line1 = std::str::from_utf8(&frames[0]).map_err(|e| PrintError::MalformedFrame {
            details: format!("~HS line 1: invalid UTF-8: {e}"),
        })?;
        let f1: Vec<&str> = line1.split(',').collect();

        let communication_flag: u32 = parse_field(&f1, 0, 1)?;
        let paper_out = parse_bool_field(&f1, 1, 1)?;
        let paused = parse_bool_field(&f1, 2, 1)?;
        let label_length_dots: u32 = parse_field(&f1, 3, 1)?;
        let formats_in_buffer: u32 = parse_field(&f1, 4, 1)?;
        let buffer_full = parse_bool_field(&f1, 5, 1)?;
        let comm_diag_mode = parse_bool_field(&f1, 6, 1)?;
        let partial_format = parse_bool_field(&f1, 7, 1)?;
        let reserved_1: u32 = parse_field(&f1, 8, 1)?;
        let corrupt_ram = parse_bool_field(&f1, 9, 1)?;
        let under_temperature = parse_bool_field(&f1, 10, 1)?;
        let over_temperature = parse_bool_field(&f1, 11, 1)?;

        // ── Line 2 ─────────────────────────────────────────────────
        let line2 = std::str::from_utf8(&frames[1]).map_err(|e| PrintError::MalformedFrame {
            details: format!("~HS line 2: invalid UTF-8: {e}"),
        })?;
        let f2: Vec<&str> = line2.split(',').collect();

        let function_settings: u32 = parse_field(&f2, 0, 2)?;
        let head_up = parse_bool_field(&f2, 1, 2)?;
        let ribbon_out = parse_bool_field(&f2, 2, 2)?;
        let thermal_transfer_mode = parse_bool_field(&f2, 3, 2)?;
        let print_mode_code: u8 = parse_field(&f2, 4, 2)?;
        let print_mode = PrintMode::from_code(print_mode_code)?;
        let print_width_mode: u32 = parse_field(&f2, 5, 2)?;
        let label_waiting = parse_bool_field(&f2, 6, 2)?;
        let labels_remaining: u32 = parse_field(&f2, 7, 2)?;
        let format_while_printing: u32 = parse_field(&f2, 8, 2)?;
        let graphics_stored_in_memory: u32 = parse_field(&f2, 9, 2)?;

        // ── Line 3 ─────────────────────────────────────────────────
        let line3 = std::str::from_utf8(&frames[2]).map_err(|e| PrintError::MalformedFrame {
            details: format!("~HS line 3: invalid UTF-8: {e}"),
        })?;
        let f3: Vec<&str> = line3.split(',').collect();

        let password: u32 = parse_field(&f3, 0, 3)?;
        let static_ram_installed = parse_bool_field(&f3, 1, 3)?;

        Ok(HostStatus {
            communication_flag,
            paper_out,
            paused,
            label_length_dots,
            formats_in_buffer,
            buffer_full,
            comm_diag_mode,
            partial_format,
            reserved_1,
            corrupt_ram,
            under_temperature,
            over_temperature,

            function_settings,
            head_up,
            ribbon_out,
            thermal_transfer_mode,
            print_mode,
            print_width_mode,
            label_waiting,
            labels_remaining,
            format_while_printing,
            graphics_stored_in_memory,

            password,
            static_ram_installed,
        })
    }
}

// ── PrinterInfo ─────────────────────────────────────────────────────────

/// Parsed `~HI` (Host Identification) response.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrinterInfo {
    /// Printer model string (e.g. `"ZTC ZD421-300dpi ZPL"`).
    pub model: String,
    /// Firmware version (e.g. `"V85.20.19"`).
    pub firmware: String,
    /// Print resolution in DPI.
    pub dpi: u32,
    /// Installed memory in kilobytes.
    pub memory_kb: u32,
}

impl PrinterInfo {
    /// Parse a `~HI` response from STX/ETX frames.
    ///
    /// Expects exactly **1** frame containing comma-separated fields:
    /// `model,firmware,dpi,memory_kb`.
    pub fn parse(frames: &[Vec<u8>]) -> Result<PrinterInfo, PrintError> {
        if frames.len() != 1 {
            return Err(PrintError::MalformedFrame {
                details: format!("~HI requires 1 frame, got {}", frames.len()),
            });
        }

        let text = std::str::from_utf8(&frames[0]).map_err(|e| PrintError::MalformedFrame {
            details: format!("~HI: invalid UTF-8: {e}"),
        })?;
        let fields: Vec<&str> = text.split(',').collect();

        if fields.len() < 4 {
            return Err(PrintError::MalformedFrame {
                details: format!("~HI: expected at least 4 fields, got {}", fields.len()),
            });
        }

        let model = fields[0].trim().to_string();
        let firmware = fields[1].trim().to_string();

        let dpi: u32 = fields[2]
            .trim()
            .parse()
            .map_err(|_| PrintError::MalformedFrame {
                details: format!("~HI: cannot parse DPI ({:?})", fields[2].trim()),
            })?;

        let memory_kb = parse_memory_kb_field(fields[3])?;

        Ok(PrinterInfo {
            model,
            firmware,
            dpi,
            memory_kb,
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a Vec<Vec<u8>> from string slices (one per frame).
    fn frames(strings: &[&str]) -> Vec<Vec<u8>> {
        strings.iter().map(|s| s.as_bytes().to_vec()).collect()
    }

    // ── HostStatus ──────────────────────────────────────────────────

    #[test]
    fn parse_host_status_normal() {
        let input = frames(&[
            "030,0,0,1245,000,0,0,0,000,0,0,0",
            "000,0,0,0,0,2,4,0,00000000,1,000",
            "1234,0",
        ]);

        let hs = HostStatus::parse(&input).expect("should parse");

        // Line 1
        assert_eq!(hs.communication_flag, 30);
        assert!(!hs.paper_out);
        assert!(!hs.paused);
        assert_eq!(hs.label_length_dots, 1245);
        assert_eq!(hs.formats_in_buffer, 0);
        assert!(!hs.buffer_full);
        assert!(!hs.comm_diag_mode);
        assert!(!hs.partial_format);
        assert_eq!(hs.reserved_1, 0);
        assert!(!hs.corrupt_ram);
        assert!(!hs.under_temperature);
        assert!(!hs.over_temperature);

        // Line 2
        assert_eq!(hs.function_settings, 0);
        assert!(!hs.head_up);
        assert!(!hs.ribbon_out);
        assert!(!hs.thermal_transfer_mode);
        assert_eq!(hs.print_mode, PrintMode::TearOff);
        assert_eq!(hs.print_width_mode, 2);
        assert!(hs.label_waiting);
        assert_eq!(hs.labels_remaining, 0);
        assert_eq!(hs.format_while_printing, 0);
        assert_eq!(hs.graphics_stored_in_memory, 1);

        // Line 3
        assert_eq!(hs.password, 1234);
        assert!(!hs.static_ram_installed);
    }

    #[test]
    fn parse_host_status_with_errors() {
        let input = frames(&[
            "030,1,1,1245,002,1,0,0,000,1,1,1",
            "000,1,1,1,4,2,0,5,00000000,0,000",
            "0000,1",
        ]);

        let hs = HostStatus::parse(&input).expect("should parse");

        // Line 1 — error flags set
        assert!(hs.paper_out);
        assert!(hs.paused);
        assert_eq!(hs.formats_in_buffer, 2);
        assert!(hs.buffer_full);
        assert!(hs.corrupt_ram);
        assert!(hs.under_temperature);
        assert!(hs.over_temperature);

        // Line 2 — error flags set
        assert!(hs.head_up);
        assert!(hs.ribbon_out);
        assert!(hs.thermal_transfer_mode);
        assert_eq!(hs.print_mode, PrintMode::Cutter);
        assert_eq!(hs.labels_remaining, 5);

        // Line 3
        assert_eq!(hs.password, 0);
        assert!(hs.static_ram_installed);
    }

    #[test]
    fn parse_host_status_wrong_frame_count() {
        // Too few
        let input = frames(&["030,0,0,1245,000,0,0,0,000,0,0,0"]);
        let err = HostStatus::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("3 frames"), "unexpected error: {msg}");

        // Too many
        let input = frames(&["a", "b", "c", "d"]);
        let err = HostStatus::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("3 frames"), "unexpected error: {msg}");

        // Empty
        let err = HostStatus::parse(&[]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("3 frames"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_host_status_malformed_field() {
        // Line 1 field 0 is not numeric
        let input = frames(&[
            "abc,0,0,1245,000,0,0,0,000,0,0,0",
            "000,0,0,0,0,2,4,0,00000000,1,000",
            "1234,0",
        ]);
        let err = HostStatus::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("cannot parse"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_host_status_missing_fields() {
        // Line 1 only has 5 fields instead of 12
        let input = frames(&[
            "030,0,0,1245,000",
            "000,0,0,0,0,2,4,0,00000000,1,000",
            "1234,0",
        ]);
        let err = HostStatus::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("expected field"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_host_status_all_print_modes() {
        let base_l1 = "030,0,0,1245,000,0,0,0,000,0,0,0";
        let base_l3 = "1234,0";

        let cases: &[(u8, PrintMode)] = &[
            (0, PrintMode::TearOff),
            (1, PrintMode::PeelOff),
            (2, PrintMode::Rewind),
            (3, PrintMode::Applicator),
            (4, PrintMode::Cutter),
            (5, PrintMode::DelayedCutter),
            (6, PrintMode::Linerless),
        ];

        for &(code, expected_mode) in cases {
            let line2 = format!("000,0,0,0,{code},2,0,0,00000000,0,000");
            let input = frames(&[base_l1, &line2, base_l3]);
            let hs =
                HostStatus::parse(&input).unwrap_or_else(|e| panic!("failed for code {code}: {e}"));
            assert_eq!(hs.print_mode, expected_mode, "mode code {code}");
        }
    }

    #[test]
    fn parse_host_status_invalid_print_mode() {
        let input = frames(&[
            "030,0,0,1245,000,0,0,0,000,0,0,0",
            "000,0,0,0,9,2,0,0,00000000,0,000",
            "1234,0",
        ]);
        let err = HostStatus::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("unknown print mode"),
            "unexpected error: {msg}"
        );
    }

    // ── PrinterInfo ─────────────────────────────────────────────────

    #[test]
    fn parse_printer_info_normal() {
        let input = frames(&["ZTC ZD421-300dpi ZPL,V85.20.19,300,131072"]);

        let info = PrinterInfo::parse(&input).expect("should parse");
        assert_eq!(info.model, "ZTC ZD421-300dpi ZPL");
        assert_eq!(info.firmware, "V85.20.19");
        assert_eq!(info.dpi, 300);
        assert_eq!(info.memory_kb, 131072);
    }

    #[test]
    fn parse_printer_info_wrong_frame_count() {
        // Zero frames
        let err = PrinterInfo::parse(&[]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("1 frame"), "unexpected error: {msg}");

        // Two frames
        let input = frames(&["a", "b"]);
        let err = PrinterInfo::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("1 frame"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_printer_info_too_few_fields() {
        let input = frames(&["ZTC ZD421-300dpi ZPL,V85.20.19"]);
        let err = PrinterInfo::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("at least 4 fields"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_printer_info_malformed_dpi() {
        let input = frames(&["ZTC ZD421-300dpi ZPL,V85.20.19,abc,131072"]);
        let err = PrinterInfo::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("DPI"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_printer_info_malformed_memory() {
        let input = frames(&["ZTC ZD421-300dpi ZPL,V85.20.19,12,xyz"]);
        let err = PrinterInfo::parse(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("memory_kb"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_printer_info_memory_with_kb_suffix() {
        let input = frames(&["ZD621-300dpi,V93.21.26Z,12,8176KB"]);
        let info = PrinterInfo::parse(&input).expect("should parse");
        assert_eq!(info.memory_kb, 8176);
    }

    // ── Serialization (serde feature only) ────────────────────────────

    #[cfg(feature = "serde")]
    #[test]
    fn host_status_serializes_to_json() {
        let input = frames(&[
            "030,0,0,1245,000,0,0,0,000,0,0,0",
            "000,0,0,0,0,2,4,0,00000000,1,000",
            "1234,0",
        ]);

        let hs = HostStatus::parse(&input).unwrap();
        let json = serde_json::to_string(&hs).expect("should serialize");
        assert!(json.contains("\"paper_out\":false"));
        assert!(json.contains("\"label_length_dots\":1245"));
        assert!(json.contains("\"print_mode\":\"TearOff\""));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn printer_info_serializes_to_json() {
        let input = frames(&["ZTC ZD421-300dpi ZPL,V85.20.19,12,131072"]);

        let info = PrinterInfo::parse(&input).unwrap();
        let json = serde_json::to_string(&info).expect("should serialize");
        assert!(json.contains("\"model\":\"ZTC ZD421-300dpi ZPL\""));
        assert!(json.contains("\"dpi\":12"));
    }

    /// Proves HostStatus and PrinterInfo parse successfully without serde feature.
    #[cfg(not(feature = "serde"))]
    #[test]
    fn parse_works_without_serde() {
        let hs_input = frames(&[
            "030,0,0,1245,000,0,0,0,000,0,0,0",
            "000,0,0,0,0,2,4,0,00000000,1,000",
            "1234,0",
        ]);
        let hs = HostStatus::parse(&hs_input).unwrap();
        assert_eq!(hs.label_length_dots, 1245);
        assert_eq!(hs.print_mode, PrintMode::TearOff);

        let info_input = frames(&["ZTC ZD421-300dpi ZPL,V85.20.19,300,131072"]);
        let info = PrinterInfo::parse(&info_input).unwrap();
        assert_eq!(info.dpi, 300);
    }
}
