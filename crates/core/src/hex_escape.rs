//! Hex escape processing for ZPL `^FH` field data.
//!
//! When `^FH` is active, underscore-prefixed hex pairs (`_XX`) in field data
//! represent literal byte values. For example, `_1A` represents byte `0x1A`.
//!
//! The indicator character defaults to `_` but can be changed via the `^FH`
//! command's optional argument.

/// A hex escape validation error at a specific byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexEscapeError {
    /// Byte offset of the indicator character within the content string.
    pub offset: usize,
    /// Human-readable description of the error.
    pub message: String,
}

/// Validate hex escape sequences in field data content.
///
/// Scans `content` for occurrences of `indicator` followed by two hex digits.
/// Returns a list of errors for invalid or incomplete sequences.
///
/// This is a lightweight validation-only function that does not allocate
/// decoded output. Use [`decode_hex_escapes`] when decoded bytes are needed.
pub fn validate_hex_escapes(content: &str, indicator: u8) -> Vec<HexEscapeError> {
    let mut errors = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == indicator {
            if i + 2 < bytes.len() {
                let h1 = bytes[i + 1];
                let h2 = bytes[i + 2];
                if !h1.is_ascii_hexdigit() || !h2.is_ascii_hexdigit() {
                    errors.push(HexEscapeError {
                        offset: i,
                        message: format!(
                            "Invalid hex escape sequence {}{}{} (expected two hex digits 0-9, A-F after '{}')",
                            indicator as char, h1 as char, h2 as char, indicator as char,
                        ),
                    });
                }
                i += 3;
            } else {
                errors.push(HexEscapeError {
                    offset: i,
                    message: format!(
                        "Incomplete hex escape sequence at offset {} (expected '{}XX' but input ends)",
                        i, indicator as char,
                    ),
                });
                break;
            }
        } else {
            i += 1;
        }
    }

    errors
}

/// Decode hex escape sequences in field data content.
///
/// Replaces `indicator + XX` sequences with the corresponding byte value.
/// Non-escaped content is copied through as UTF-8 bytes.
///
/// Returns `Ok(decoded_bytes)` on success, or `Err(errors)` listing all
/// invalid sequences found (processing continues past errors, substituting
/// the raw bytes for invalid sequences).
pub fn decode_hex_escapes(content: &str, indicator: u8) -> Result<Vec<u8>, Vec<HexEscapeError>> {
    let mut output = Vec::with_capacity(content.len());
    let mut errors = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == indicator {
            if i + 2 < bytes.len() {
                let h1 = bytes[i + 1];
                let h2 = bytes[i + 2];
                if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() {
                    // Safe: we've verified both are hex digits
                    let byte = hex_pair_to_byte(h1, h2);
                    output.push(byte);
                } else {
                    errors.push(HexEscapeError {
                        offset: i,
                        message: format!(
                            "Invalid hex escape sequence {}{}{} (expected two hex digits 0-9, A-F after '{}')",
                            indicator as char, h1 as char, h2 as char, indicator as char,
                        ),
                    });
                    // Copy the raw bytes through on error
                    output.push(bytes[i]);
                    output.push(h1);
                    output.push(h2);
                }
                i += 3;
            } else {
                errors.push(HexEscapeError {
                    offset: i,
                    message: format!(
                        "Incomplete hex escape sequence at offset {} (expected '{}XX' but input ends)",
                        i, indicator as char,
                    ),
                });
                // Copy remaining bytes through
                while i < bytes.len() {
                    output.push(bytes[i]);
                    i += 1;
                }
                break;
            }
        } else {
            output.push(bytes[i]);
            i += 1;
        }
    }

    if errors.is_empty() {
        Ok(output)
    } else {
        Err(errors)
    }
}

/// Convert two ASCII hex digit bytes to a single byte value.
fn hex_pair_to_byte(h1: u8, h2: u8) -> u8 {
    (hex_digit_value(h1) << 4) | hex_digit_value(h2)
}

/// Convert a single ASCII hex digit to its numeric value (0-15).
fn hex_digit_value(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'A'..=b'F' => b - b'A' + 10,
        b'a'..=b'f' => b - b'a' + 10,
        _ => unreachable!("hex_digit_value called with non-hex byte: {}", b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_hex_escapes ────────────────────────────────────────────

    #[test]
    fn validate_valid_sequences() {
        assert!(validate_hex_escapes("Hello_20World", b'_').is_empty());
        assert!(validate_hex_escapes("_00_FF_0A_ff", b'_').is_empty());
        assert!(validate_hex_escapes("no escapes here", b'_').is_empty());
        assert!(validate_hex_escapes("", b'_').is_empty());
    }

    #[test]
    fn validate_invalid_hex_digits() {
        let errors = validate_hex_escapes("_GG", b'_');
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].offset, 0);
    }

    #[test]
    fn validate_incomplete_at_end() {
        let errors = validate_hex_escapes("Hello_", b'_');
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].offset, 5);

        let errors = validate_hex_escapes("Hello_A", b'_');
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].offset, 5);
    }

    #[test]
    fn validate_multiple_errors() {
        let errors = validate_hex_escapes("_ZZ_XX", b'_');
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].offset, 0);
        assert_eq!(errors[1].offset, 3);
    }

    #[test]
    fn validate_custom_indicator() {
        // Using '#' instead of '_'
        assert!(validate_hex_escapes("#41#42", b'#').is_empty());
        // '_' should be treated as normal text when indicator is '#'
        assert!(validate_hex_escapes("_GG", b'#').is_empty());
        // '#' without valid hex digits
        let errors = validate_hex_escapes("#ZZ", b'#');
        assert_eq!(errors.len(), 1);
    }

    // ── decode_hex_escapes ──────────────────────────────────────────────

    #[test]
    fn decode_simple() {
        let result = decode_hex_escapes("Hello_20World", b'_').unwrap();
        assert_eq!(result, b"Hello World");
    }

    #[test]
    fn decode_multiple() {
        let result = decode_hex_escapes("_48_65_6C_6C_6F", b'_').unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn decode_no_escapes() {
        let result = decode_hex_escapes("plain text", b'_').unwrap();
        assert_eq!(result, b"plain text");
    }

    #[test]
    fn decode_empty() {
        let result = decode_hex_escapes("", b'_').unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn decode_all_escaped() {
        let result = decode_hex_escapes("_00_FF_7F", b'_').unwrap();
        assert_eq!(result, &[0x00, 0xFF, 0x7F]);
    }

    #[test]
    fn decode_custom_indicator() {
        let result = decode_hex_escapes("#41#42#43", b'#').unwrap();
        assert_eq!(result, b"ABC");
    }

    #[test]
    fn decode_error_returns_raw_bytes() {
        let errors = decode_hex_escapes("_GG", b'_').unwrap_err();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn decode_case_insensitive_hex() {
        let upper = decode_hex_escapes("_4A", b'_').unwrap();
        let lower = decode_hex_escapes("_4a", b'_').unwrap();
        assert_eq!(upper, lower);
        assert_eq!(upper, b"J");
    }

    #[test]
    fn decode_mixed_content_and_escapes() {
        let result = decode_hex_escapes("Price:_20_2410.00", b'_').unwrap();
        // _20 = space, _24 = '$'
        assert_eq!(result, b"Price: $10.00");
    }
}
