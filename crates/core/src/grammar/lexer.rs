/// Classification of a ZPL lexer token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokKind {
    /// Command leader character (`^` or `~`).
    Leader,
    /// Argument delimiter (`,` by default).
    Comma,
    /// A run of non-delimiter, non-leader characters.
    Value,
    /// A line feed character.
    Newline,
    /// One or more whitespace characters (excluding newlines).
    Whitespace,
}

/// A token that borrows its text directly from the source input — zero allocation.
///
/// `text` is always exactly `&input[start..end]`. The `start`/`end` byte offsets
/// are stored alongside for consumers that need numeric positions (spans, slicing).
#[derive(Debug)]
pub struct Token<'a> {
    /// The classification of this token.
    pub kind: TokKind,
    /// Borrowed slice of the source input for this token.
    pub text: &'a str,
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset one past the last character.
    pub end: usize,
}

/// Tokenize ZPL input into a sequence of borrowed tokens.
///
/// Every token's `text` field borrows directly from `input`, so the returned
/// `Vec<Token<'_>>` is valid for as long as `input` is alive. No heap
/// allocations are made for token text.
///
/// Uses the default command prefix (`^`), control prefix (`~`), and
/// argument delimiter (`,`).
pub fn tokenize(input: &str) -> Vec<Token<'_>> {
    tokenize_with_config(input, '^', '~', ',')
}

/// Tokenize ZPL input with configurable prefix and delimiter characters.
///
/// `cmd_prefix` is the format command prefix (default `^`).
/// `ctrl_prefix` is the control command prefix (default `~`).
/// `delimiter` is the argument separator (default `,`).
///
/// # Safety of `b[i] as char`
///
/// All delimiter characters and the whitespace test (`is_ascii_whitespace`)
/// operate on ASCII values (0x00–0x7F). UTF-8 continuation bytes are in
/// the range 0x80–0xBF, so they never match any of these tests. This makes
/// the `b[i] as char` cast safe for delimiter detection without full UTF-8
/// decoding.
pub fn tokenize_with_config(
    input: &str,
    cmd_prefix: char,
    ctrl_prefix: char,
    delimiter: char,
) -> Vec<Token<'_>> {
    let mut toks = Vec::new();
    let mut i = 0usize;
    let b = input.as_bytes();
    while i < b.len() {
        let c = b[i] as char;
        let start = i;
        if c == cmd_prefix || c == ctrl_prefix {
            // Leader
            i += 1;
            toks.push(Token {
                kind: TokKind::Leader,
                text: &input[start..i],
                start,
                end: i,
            });
        } else if c == delimiter {
            i += 1;
            toks.push(Token {
                kind: TokKind::Comma,
                text: &input[start..i],
                start,
                end: i,
            });
        } else if c == '\n' || c == '\r' {
            // Normalize CRLF/CR/LF into a single Newline token.
            if c == '\r' && i + 1 < b.len() && b[i + 1] as char == '\n' {
                i += 2;
            } else {
                i += 1;
            }
            toks.push(Token {
                kind: TokKind::Newline,
                text: &input[start..i],
                start,
                end: i,
            });
        } else if c.is_ascii_whitespace() {
            i += 1;
            while i < b.len()
                && (b[i] as char).is_ascii_whitespace()
                && (b[i] as char) != '\n'
                && (b[i] as char) != '\r'
            {
                i += 1;
            }
            toks.push(Token {
                kind: TokKind::Whitespace,
                text: &input[start..i],
                start,
                end: i,
            });
        } else {
            // Value run — stop on delimiter, configured prefixes, or newline
            i += 1;
            while i < b.len() {
                let ch = b[i] as char;
                if ch == delimiter
                    || ch == cmd_prefix
                    || ch == ctrl_prefix
                    || ch == '\n'
                    || ch == '\r'
                {
                    break;
                }
                i += 1;
            }
            toks.push(Token {
                kind: TokKind::Value,
                text: &input[start..i],
                start,
                end: i,
            });
        }
    }
    toks
}
