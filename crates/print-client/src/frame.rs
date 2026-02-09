//! STX/ETX frame parser -- byte-level state machine for Zebra printer responses.
//!
//! Zebra printers frame their responses (e.g., ~HS, ~HI) between
//! STX (0x02) and ETX (0x03) bytes. A ~HS response has 3 frames;
//! a ~HI response has 1 frame.
//!
//! Frames can split across TCP segments, so this parser operates
//! byte-by-byte and handles partial reads correctly.

use std::io::Read;
use std::time::{Duration, Instant};

use crate::PrintError;

/// ASCII Start of Text
const STX: u8 = 0x02;
/// ASCII End of Text
const ETX: u8 = 0x03;

/// Default maximum frame size (1 KB). ~HS responses are about 100 bytes
/// per frame; this guard prevents runaway reads from a misbehaving printer.
pub(crate) const DEFAULT_MAX_FRAME_SIZE: usize = 1024;

/// Internal state of the frame parser.
enum FrameState {
    /// Waiting for a STX byte; skip any garbage (CR, LF, etc.) between frames.
    WaitingForStx,
    /// Inside a frame -- collecting bytes until ETX.
    ReadingFrame,
}

/// Read exactly `expected_count` STX/ETX framed responses from a stream.
///
/// # Arguments
///
/// * `stream` -- Any `Read` source (TCP stream, serial port, etc.)
/// * `expected_count` -- Number of frames to collect (3 for ~HS, 1 for ~HI)
/// * `timeout` -- Maximum wall-clock time to wait for all frames
/// * `max_frame_size` -- Maximum bytes per frame (guard against runaway reads)
///
/// # Returns
///
/// A `Vec` of frame payloads (bytes between STX and ETX, exclusive).
/// Each frame is the raw comma-separated data -- the caller is responsible
/// for parsing the fields.
pub fn read_frames(
    stream: &mut impl Read,
    expected_count: usize,
    timeout: Duration,
    max_frame_size: usize,
) -> Result<Vec<Vec<u8>>, PrintError> {
    let now = Instant::now();
    let deadline = now
        .checked_add(timeout)
        .unwrap_or_else(|| now + Duration::from_secs(86400));
    let mut frames: Vec<Vec<u8>> = Vec::with_capacity(expected_count);
    let mut current_frame: Vec<u8> = Vec::with_capacity(256);
    let mut state = FrameState::WaitingForStx;
    let mut buf = [0u8; 512];

    while frames.len() < expected_count {
        // Check timeout before each read
        if Instant::now() >= deadline {
            return Err(PrintError::ReadTimeout);
        }

        let n = match stream.read(&mut buf) {
            Ok(0) => return Err(PrintError::ConnectionClosed),
            Ok(n) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if Instant::now() >= deadline {
                    return Err(PrintError::ReadTimeout);
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(PrintError::ReadTimeout);
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(e) => {
                return Err(PrintError::ReadFailed(e));
            }
        };

        for &byte in &buf[..n] {
            match (&state, byte) {
                (FrameState::WaitingForStx, STX) => {
                    current_frame.clear();
                    state = FrameState::ReadingFrame;
                }
                (FrameState::WaitingForStx, _) => {
                    // Skip CR, LF, and any garbage between frames
                }
                (FrameState::ReadingFrame, ETX) => {
                    frames.push(std::mem::take(&mut current_frame));
                    state = FrameState::WaitingForStx;
                    if frames.len() >= expected_count {
                        return Ok(frames);
                    }
                }
                (FrameState::ReadingFrame, _) => {
                    if current_frame.len() >= max_frame_size {
                        return Err(PrintError::FrameTooLarge {
                            size: current_frame.len() + 1,
                            max: max_frame_size,
                        });
                    }
                    current_frame.push(byte);
                }
            }
        }
    }

    Ok(frames)
}

/// Determine the expected number of STX/ETX frames for a given command.
pub fn expected_frame_count(cmd: &[u8]) -> usize {
    if cmd.starts_with(b"~HS") { 3 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_single_frame() {
        let data = [0x02, b'H', b'e', b'l', b'l', b'o', 0x03];
        let mut cursor = Cursor::new(data);
        let frames = read_frames(
            &mut cursor,
            1,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        )
        .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], b"Hello");
    }

    #[test]
    fn test_three_frames_like_hs() {
        let mut data = Vec::new();
        data.push(STX);
        data.extend_from_slice(b"030,0,0,1245,000,0,0,0,000,0,0,0");
        data.push(ETX);
        data.extend_from_slice(b"\r\n");
        data.push(STX);
        data.extend_from_slice(b"000,0,0,0,0,2,4,0,00000000,1,000");
        data.push(ETX);
        data.extend_from_slice(b"\r\n");
        data.push(STX);
        data.extend_from_slice(b"1234,0");
        data.push(ETX);

        let mut cursor = Cursor::new(data);
        let frames = read_frames(
            &mut cursor,
            3,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        )
        .unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], b"030,0,0,1245,000,0,0,0,000,0,0,0");
        assert_eq!(frames[1], b"000,0,0,0,0,2,4,0,00000000,1,000");
        assert_eq!(frames[2], b"1234,0");
    }

    #[test]
    fn test_garbage_before_first_frame() {
        let mut data = Vec::new();
        data.extend_from_slice(b"\r\n\r\n");
        data.push(STX);
        data.extend_from_slice(b"data");
        data.push(ETX);

        let mut cursor = Cursor::new(data);
        let frames = read_frames(
            &mut cursor,
            1,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        )
        .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], b"data");
    }

    #[test]
    fn test_frame_too_large() {
        let mut data = Vec::new();
        data.push(STX);
        data.extend(vec![b'X'; 2000]);
        data.push(ETX);

        let mut cursor = Cursor::new(data);
        let result = read_frames(&mut cursor, 1, Duration::from_secs(1), 1024);
        assert!(result.is_err());
        match result.unwrap_err() {
            PrintError::FrameTooLarge { max, .. } => assert_eq!(max, 1024),
            other => panic!("expected FrameTooLarge, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_frame() {
        let data = [STX, ETX];
        let mut cursor = Cursor::new(data);
        let frames = read_frames(
            &mut cursor,
            1,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        )
        .unwrap();
        assert_eq!(frames.len(), 1);
        assert!(frames[0].is_empty());
    }

    #[test]
    fn test_empty_input() {
        let data: &[u8] = &[];
        let mut cursor = Cursor::new(data);
        let result = read_frames(
            &mut cursor,
            1,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        );
        assert!(matches!(result, Err(PrintError::ConnectionClosed)));
    }

    #[test]
    fn test_expected_count_zero() {
        let data = [STX, b'A', ETX];
        let mut cursor = Cursor::new(data);
        let frames = read_frames(
            &mut cursor,
            0,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        )
        .unwrap();
        assert!(frames.is_empty());
    }

    #[test]
    fn test_back_to_back_frames() {
        let data = [STX, b'A', ETX, STX, b'B', ETX];
        let mut cursor = Cursor::new(data);
        let frames = read_frames(
            &mut cursor,
            2,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        )
        .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], b"A");
        assert_eq!(frames[1], b"B");
    }

    #[test]
    fn test_garbage_only_no_stx() {
        let data = [0x0D, 0x0A, b'x', b'y'];
        let mut cursor = Cursor::new(data);
        let result = read_frames(
            &mut cursor,
            1,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        );
        assert!(matches!(result, Err(PrintError::ConnectionClosed)));
    }

    #[test]
    fn test_frame_at_exact_max_size() {
        let mut data = Vec::new();
        data.push(STX);
        data.extend(vec![b'X'; 1024]);
        data.push(ETX);

        let mut cursor = Cursor::new(data);
        let frames = read_frames(&mut cursor, 1, Duration::from_secs(1), 1024).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), 1024);
    }

    #[test]
    fn test_frame_one_byte_over_max() {
        let mut data = Vec::new();
        data.push(STX);
        data.extend(vec![b'X'; 1025]);
        data.push(ETX);

        let mut cursor = Cursor::new(data);
        let result = read_frames(&mut cursor, 1, Duration::from_secs(1), 1024);
        assert!(matches!(result, Err(PrintError::FrameTooLarge { .. })));
    }

    #[test]
    fn test_connection_closed_mid_frame() {
        let data = [STX, b'p', b'a', b'r', b't', b'i', b'a', b'l'];
        let mut cursor = Cursor::new(data);
        let result = read_frames(
            &mut cursor,
            1,
            Duration::from_secs(1),
            DEFAULT_MAX_FRAME_SIZE,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PrintError::ConnectionClosed => {}
            other => panic!("expected ConnectionClosed, got {:?}", other),
        }
    }
}
