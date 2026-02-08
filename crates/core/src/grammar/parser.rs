use super::{
    ast::{ArgSlot, Ast, Label, Node, Presence},
    diag::{Diagnostic, Span, codes},
    lexer::{TokKind, tokenize},
    tables::ParserTables,
};
use zpl_toolchain_spec_tables::CommandEntry;

/// Shorthand for building a `BTreeMap<String, String>` context from key-value pairs.
macro_rules! ctx {
    ($($k:expr => $v:expr),+ $(,)?) => {
        std::collections::BTreeMap::from([$(($k.into(), $v.into())),+])
    };
}

/// Result of parsing a ZPL input string.
#[derive(serde::Serialize)]
pub struct ParseResult {
    /// The parsed abstract syntax tree.
    pub ast: Ast,
    /// Diagnostics (errors, warnings, info) produced during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

// ─── Parser Mode State Machine ──────────────────────────────────────────────

/// The parser operates in one of several modes, driven by command type.
enum Mode {
    /// Standard command parsing (default).
    Normal,
    /// Field data collection after ^FD or ^FV.
    /// Accumulates raw text until ^FS is encountered.
    FieldData {
        /// Byte offset where field data content begins.
        content_start: usize,
        /// Whether ^FH was seen in the current field, enabling hex escape processing.
        hex_escape: bool,
    },
    /// Raw payload collection after a raw_payload command (e.g., ^GF, ~DG).
    /// Collects data until a command leader (^ or ~) or end of input.
    RawData {
        /// The command code that started raw data mode (e.g., "^GF").
        command: String,
        /// Byte offset where raw data content begins.
        content_start: usize,
    },
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Parse a ZPL input string without spec tables (heuristic mode).
pub fn parse_str(input: &str) -> ParseResult {
    parse_with_tables(input, None)
}

/// Parse a ZPL input string with optional spec tables for opcode recognition.
pub fn parse_with_tables(input: &str, tables: Option<&ParserTables>) -> ParseResult {
    Parser::new(input, tables).parse()
}

// ─── Parser Implementation ─────────────────────────────────────────────────

struct Parser<'a> {
    input: &'a str,
    tables: Option<&'a ParserTables>,
    toks: Vec<super::lexer::Token<'a>>,
    pos: usize,
    diags: Vec<Diagnostic>,
    labels: Vec<Label>,
    nodes: Vec<Node>,
    in_label: bool,
    mode: Mode,
    /// Whether ^FH was seen in the current field group (between field-opening and ^FS).
    fh_active: bool,
    /// Current format command prefix character (default `^`).
    command_prefix: char,
    /// Current control command prefix character (default `~`).
    control_prefix: char,
    /// Current argument delimiter character (default `,`).
    delimiter: char,
}

impl<'a> Parser<'a> {
    /// Return the smallest index >= `pos` that is a valid UTF-8 char boundary,
    /// clamped to `s.len()`.
    fn next_char_boundary(s: &str, pos: usize) -> usize {
        let mut p = pos;
        while p < s.len() && !s.is_char_boundary(p) {
            p += 1;
        }
        p.min(s.len())
    }

    fn new(input: &'a str, tables: Option<&'a ParserTables>) -> Self {
        Self {
            input,
            tables,
            toks: tokenize(input),
            pos: 0,
            diags: Vec::new(),
            labels: Vec::new(),
            nodes: Vec::new(),
            in_label: false,
            mode: Mode::Normal,
            fh_active: false,
            command_prefix: '^',
            control_prefix: '~',
            delimiter: ',',
        }
    }

    // ── Lookup helpers (O(1) via ParserTables cached index) ─────────────

    fn lookup_command(&self, code: &str) -> Option<&'a CommandEntry> {
        self.tables.and_then(|t| t.cmd_by_code(code))
    }

    fn is_field_data_command(&self, code: &str) -> bool {
        self.lookup_command(code).is_some_and(|ce| ce.field_data)
    }

    fn is_raw_payload_command(&self, code: &str) -> bool {
        self.lookup_command(code).is_some_and(|ce| ce.raw_payload)
    }

    fn is_known_code(&self, code: &str) -> bool {
        self.tables.is_some_and(|t| t.code_set().contains(code))
    }

    fn has_tables(&self) -> bool {
        self.tables.is_some()
    }

    // ── Token navigation ────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    /// Advance `pos` to the next `Leader` token or end of input.
    ///
    /// This is the primary recovery strategy: when the parser encounters
    /// malformed input, skip ahead to the next command boundary (a `^` or `~`
    /// leader) so parsing can resume at a known-good synchronization point.
    fn skip_to_next_leader(&mut self) {
        while !self.at_end() && !matches!(self.toks[self.pos].kind, TokKind::Leader) {
            self.pos += 1;
        }
    }

    // ── Main parse loop ─────────────────────────────────────────────────

    fn parse(mut self) -> ParseResult {
        while !self.at_end() {
            match self.mode {
                Mode::Normal => self.parse_normal(),
                Mode::FieldData { .. } => self.parse_field_data(),
                Mode::RawData { .. } => self.parse_raw_data(),
            }
        }

        // Handle unterminated mode at end of input.
        // Only one mode can be active; `match` makes this mutual exclusivity explicit.
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::RawData {
                command,
                content_start,
            } => {
                let span = Span::new(content_start, self.input.len());
                // Emit the diagnostic first (borrows command), then move into node.
                self.diags.push(
                    Diagnostic::error(
                        codes::PARSER_MISSING_FIELD_SEPARATOR,
                        format!("unterminated raw data for {} at end of input", &command),
                        Some(span),
                    )
                    .with_context(ctx!("command" => command.clone(), "expected" => "^FS")),
                );
                let data = self.input[content_start..].to_string();
                if !data.is_empty() {
                    self.nodes.push(Node::RawData {
                        command,
                        data: Some(data),
                        span,
                    });
                }
            }
            Mode::FieldData {
                content_start,
                hex_escape,
            } => {
                let content = self.input[content_start..].to_string();
                if !content.is_empty() {
                    self.nodes.push(Node::FieldData {
                        content,
                        hex_escaped: hex_escape,
                        span: Span::new(content_start, self.input.len()),
                    });
                }
                self.diags.push(
                    Diagnostic::error(
                        codes::PARSER_MISSING_FIELD_SEPARATOR,
                        "missing field separator (^FS) before end of input",
                        Some(Span::new(content_start, self.input.len())),
                    )
                    .with_context(ctx!("expected" => "^FS")),
                );
            }
            Mode::Normal => {} // nothing to clean up
        }

        if self.in_label {
            self.diags.push(
                Diagnostic::error(
                    codes::PARSER_MISSING_TERMINATOR,
                    "missing terminator (^XZ)",
                    Some(Span::new(self.input.len(), self.input.len())),
                )
                .with_context(ctx!("expected" => "^XZ")),
            );
            self.labels.push(Label {
                nodes: std::mem::take(&mut self.nodes),
            });
        } else if !self.nodes.is_empty() {
            self.labels.push(Label {
                nodes: std::mem::take(&mut self.nodes),
            });
        }

        if self.labels.is_empty() {
            let span = if self.input.is_empty() {
                Span::empty(0)
            } else {
                Span::new(0, self.input.len())
            };
            self.diags.push(Diagnostic::info(
                codes::PARSER_NO_LABELS,
                "no labels detected",
                Some(span),
            ));
        }

        ParseResult {
            ast: Ast {
                labels: self.labels,
            },
            diagnostics: self.diags,
        }
    }

    // ── Normal mode ─────────────────────────────────────────────────────

    fn parse_normal(&mut self) {
        let tok = &self.toks[self.pos];
        match tok.kind {
            TokKind::Leader => self.parse_command(),
            TokKind::Comment => {
                let text = self.toks[self.pos].text.to_owned();
                let start = self.toks[self.pos].start;
                let end = self.toks[self.pos].end;
                self.pos += 1;
                self.nodes.push(Node::Trivia {
                    text,
                    span: Span::new(start, end),
                });
            }
            // Whitespace and newlines between commands are expected; skip silently.
            TokKind::Whitespace | TokKind::Newline => {
                self.pos += 1;
            }
            // Value or Comma tokens outside a command context are stray content.
            // Coalesce adjacent stray tokens into a single diagnostic to avoid
            // flooding the output on e.g. a block of plain text.
            _ => {
                let start = self.toks[self.pos].start;
                let mut end = self.toks[self.pos].end;
                self.pos += 1;
                while !self.at_end() {
                    match self.toks[self.pos].kind {
                        TokKind::Value | TokKind::Comma => {
                            end = self.toks[self.pos].end;
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                self.diags.push(Diagnostic::warn(
                    codes::PARSER_STRAY_CONTENT,
                    "stray content outside of command context",
                    Some(Span::new(start, end)),
                ));
            }
        }
    }

    // ── Command parsing (within Normal mode) ────────────────────────────

    fn parse_command(&mut self) {
        let leader_start = self.toks[self.pos].start;
        let leader_text = self.toks[self.pos].text;
        // Map the actual leader to its canonical form for downstream lookups.
        // After a prefix change (^CC/~CT), the leader character may differ from
        // the default ^ or ~, but all opcode tables use canonical prefixes.
        let canonical_leader = if leader_text.starts_with(self.command_prefix) {
            "^"
        } else {
            "~"
        };
        self.pos += 1;

        // Next token must be a Value starting the command code.
        // If not, emit an error and resync to the next leader so we don't
        // waste time advancing one token at a time through stray content.
        if self.at_end() || !matches!(self.toks[self.pos].kind, TokKind::Value) {
            self.diags.push(
                Diagnostic::error(
                    codes::PARSER_INVALID_COMMAND,
                    "invalid command: expected command code after leader",
                    Some(Span::new(leader_start, leader_start + leader_text.len())),
                )
                .with_context(ctx!("command" => leader_text)),
            );
            self.skip_to_next_leader();
            return;
        }

        let code_tok_start = self.toks[self.pos].start;

        // ── Opcode recognition (trie → known-set → heuristic) ──────
        // Always use canonical leader for trie/set lookups so they match
        // the spec tables regardless of the current prefix character.
        let head = self.recognize_opcode(canonical_leader, code_tok_start);

        if head.is_empty() {
            // Snap span end to the next char boundary to avoid panics on multi-byte UTF-8.
            let span_end = Self::next_char_boundary(self.input, code_tok_start + 1);
            self.diags.push(
                Diagnostic::error(
                    codes::PARSER_INVALID_COMMAND,
                    "missing command code after leader",
                    Some(Span::new(leader_start, span_end)),
                )
                .with_context(ctx!("command" => leader_text)),
            );
            // Resync to next leader — skip past the bad token(s).
            self.skip_to_next_leader();
            return;
        }

        let code = format!("{}{}", canonical_leader, head);

        // ── Prefix/delimiter change commands (^CC, ~CC, ^CT, ~CT, ^CD, ~CD) ──
        // These take a single character as their argument and must be handled
        // BEFORE general argument collection. After parsing, the remaining
        // input is re-tokenized with the new prefix characters.
        if matches!(code.as_str(), "^CC" | "~CC" | "^CT" | "~CT" | "^CD" | "~CD") {
            let rem_start = Self::next_char_boundary(self.input, code_tok_start + head.len());
            // The argument is the very next character in the input stream.
            let arg_char = self.input[rem_start..].chars().next();
            let arg_end = rem_start + arg_char.map_or(0, |c| c.len_utf8());
            let cmd_span = Span::new(leader_start, arg_end);

            let args = if let Some(ch) = arg_char {
                vec![ArgSlot {
                    key: Some("x".into()),
                    presence: Presence::Value,
                    value: Some(ch.to_string()),
                }]
            } else {
                Vec::new()
            };

            // Apply the prefix/delimiter change (only ASCII characters allowed)
            if let Some(ch) = arg_char {
                if !ch.is_ascii() {
                    self.diags.push(Diagnostic::error(
                        codes::PARSER_NON_ASCII_ARG,
                        format!("{} argument must be an ASCII character, got '{}'", code, ch),
                        Some(cmd_span),
                    ));
                } else {
                    match code.as_str() {
                        "^CC" | "~CC" => {
                            if ch != self.command_prefix {
                                self.command_prefix = ch;
                            }
                        }
                        "^CT" | "~CT" => {
                            if ch != self.control_prefix {
                                self.control_prefix = ch;
                            }
                        }
                        "^CD" | "~CD" => {
                            self.delimiter = ch;
                        }
                        _ => unreachable!(
                            "prefix/delimiter command matched but no handler: code={code:?} — this indicates a bug in the opcode classification"
                        ),
                    }
                }
            }

            self.nodes.push(Node::Command {
                code,
                args,
                span: cmd_span,
            });

            // Re-tokenize remaining input starting after the single-char argument
            // with the (potentially updated) prefix characters.
            // First, advance past all current tokens that cover positions <= arg_end.
            while self.pos < self.toks.len() && self.toks[self.pos].start < arg_end {
                self.pos += 1;
            }
            // Re-tokenize from arg_end onward with updated prefixes and delimiter.
            let remaining = &self.input[arg_end..];
            if !remaining.is_empty() {
                self.toks.truncate(self.pos);
                let new_toks = super::lexer::tokenize_with_config(
                    remaining,
                    self.command_prefix,
                    self.control_prefix,
                    self.delimiter,
                );
                for t in new_toks {
                    let abs_start = arg_end + t.start;
                    let abs_end = arg_end + t.end;
                    self.toks.push(super::lexer::Token {
                        kind: t.kind,
                        text: &self.input[abs_start..abs_end],
                        start: abs_start,
                        end: abs_end,
                    });
                }
            }
            return;
        }

        // Collect raw argument text (remainder of current token + subsequent tokens)
        let mut raw = String::new();
        let rem_start = Self::next_char_boundary(self.input, code_tok_start + head.len());
        let rem_end = self.toks[self.pos].end;
        if rem_start < rem_end {
            let rem = &self.input[rem_start..rem_end];
            if !rem.starts_with(self.command_prefix) && !rem.starts_with(self.control_prefix) {
                raw.push_str(rem);
            }
        }
        self.pos += 1;

        // Continue collecting until next leader, newline, or comment
        while !self.at_end() {
            match self.toks[self.pos].kind {
                TokKind::Leader => break,
                TokKind::Newline => {
                    self.pos += 1;
                    break;
                }
                TokKind::Comment => break,
                TokKind::Whitespace | TokKind::Value | TokKind::Comma => {
                    raw.push_str(self.toks[self.pos].text);
                    self.pos += 1;
                }
            }
        }

        let command_end = if self.pos > 0 {
            self.toks[self.pos - 1].end
        } else {
            rem_end
        };
        let cmd_span = Span::new(leader_start, command_end);

        // ── Emit unknown-command warning (distinct code: ZPL.PARSER.1002) ──
        if self.has_tables() && !self.is_known_code(&code) {
            self.diags.push(
                Diagnostic::warn(
                    codes::PARSER_UNKNOWN_COMMAND,
                    format!("unknown command {}", code),
                    Some(cmd_span),
                )
                .with_context(ctx!("command" => code.clone())),
            );
        }

        // ── Label delimiters (^XA / ^XZ) ───────────────────────────
        if code == "^XA" {
            if self.in_label {
                self.labels.push(Label {
                    nodes: std::mem::take(&mut self.nodes),
                });
            }
            self.in_label = true;
            // nodes is already empty after `take` above; no need to reallocate
            self.fh_active = false;
            self.mode = Mode::Normal;
            self.nodes.push(Node::Command {
                code,
                args: Vec::new(),
                span: cmd_span,
            });
            return;
        }
        if code == "^XZ" {
            // Note: if Mode::FieldData is active, parse_field_data() handles the
            // interruption and switches back to Normal before we get here.
            // This check is a safety net for edge cases.
            if matches!(self.mode, Mode::FieldData { .. }) {
                self.diags.push(
                    Diagnostic::error(
                        codes::PARSER_MISSING_FIELD_SEPARATOR,
                        "missing field separator (^FS) before ^XZ",
                        Some(cmd_span),
                    )
                    .with_context(ctx!("expected" => "^FS")),
                );
                self.mode = Mode::Normal;
                self.fh_active = false;
            }
            self.nodes.push(Node::Command {
                code,
                args: Vec::new(),
                span: cmd_span,
            });
            self.labels.push(Label {
                nodes: std::mem::take(&mut self.nodes),
            });
            self.in_label = false;
            return;
        }

        // ── Track hex escape activation (spec-driven) ───────────────
        if self
            .lookup_command(&code)
            .is_some_and(|ce| ce.hex_escape_modifier)
        {
            self.fh_active = true;
        }

        // ── Handle field data commands (^FD, ^FV): entire raw content is a single arg ──
        let is_field_data = self.is_field_data_command(&code);
        let args = if is_field_data {
            // Field data: entire raw content is literal text, not comma-separated
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![ArgSlot {
                    key: Some("data".into()),
                    presence: Presence::Value,
                    value: Some(trimmed.to_string()),
                }]
            }
        } else {
            self.parse_args(&code, &raw)
        };

        // ── Handle field close — resets field tracking (spec-driven) ──
        if self.lookup_command(&code).is_some_and(|ce| ce.closes_field) {
            self.fh_active = false;
        }

        // Determine the post-command mode before pushing the node, so we can
        // move `code` into either the node or the RawData mode without cloning.
        let raw_payload = !is_field_data && self.is_raw_payload_command(&code);

        if raw_payload {
            // RawData mode needs ownership of `code`, so clone into the node.
            self.nodes.push(Node::Command {
                code: code.clone(),
                args,
                span: cmd_span,
            });
            let content_start = if self.at_end() {
                self.input.len()
            } else {
                self.toks[self.pos].start
            };
            self.mode = Mode::RawData {
                command: code,
                content_start,
            };
        } else {
            // Common path: move `code` directly into the node — zero clones.
            self.nodes.push(Node::Command {
                code,
                args,
                span: cmd_span,
            });
            if is_field_data {
                let content_start = if self.at_end() {
                    self.input.len()
                } else {
                    self.toks[self.pos].start
                };
                self.mode = Mode::FieldData {
                    content_start,
                    hex_escape: self.fh_active,
                };
            }
        }
    }

    // ── Opcode recognition ──────────────────────────────────────────────

    /// Recognize a command opcode starting at `start_pos` in the input.
    ///
    /// SAFETY: We only compare against ASCII characters. UTF-8 multi-byte
    /// sequences have continuation bytes in 0x80..=0xBF which never match
    /// ASCII letters/digits, so `bytes[idx] as char` is safe for these checks.
    fn recognize_opcode(&self, leader: &str, start_pos: usize) -> String {
        let bytes = self.input.as_bytes();

        // Strategy 1: Opcode trie (longest match)
        if let Some(trie) = self.tables.and_then(|t| t.opcode_trie.as_ref())
            && let Some(leader_ch) = leader.chars().next()
            && let Some(node_leader) = trie.children.get(&leader_ch)
        {
            let mut node = node_leader;
            let mut last_term_len: Option<usize> = None;
            let mut k = 0usize;
            while k < 3 {
                let idx = start_pos + k;
                if idx >= bytes.len() {
                    break;
                }
                let ch = bytes[idx] as char;
                if let Some(next) = node.children.get(&ch) {
                    node = next;
                    if node.terminal {
                        last_term_len = Some(k + 1);
                    }
                    k += 1;
                } else {
                    break;
                }
            }
            if let Some(len) = last_term_len {
                return self.input[start_pos..start_pos + len].to_string();
            }
        }

        // Strategy 2: Known-set longest match (fallback when trie doesn't match)
        if let Some(set) = self.tables.map(|t| t.code_set()) {
            let mut cand = String::new();
            for k in 0..3 {
                let idx = start_pos + k;
                if idx >= bytes.len() {
                    break;
                }
                let ch = bytes[idx] as char;
                if ch.is_ascii_alphanumeric() || ch == '@' {
                    cand.push(ch);
                } else {
                    break;
                }
            }
            for len in (1..=cand.len()).rev() {
                let h = &cand[..len];
                let code_try = format!("{}{}", leader, h);
                if set.contains(&code_try) {
                    return h.to_string();
                }
            }
        }

        // Strategy 3: Heuristic (no tables available)
        let c1 = bytes.get(start_pos).map(|b| *b as char).unwrap_or('\0');
        let c2 = bytes.get(start_pos + 1).map(|b| *b as char).unwrap_or('\0');
        let c3 = bytes.get(start_pos + 2).map(|b| *b as char).unwrap_or('\0');

        let mut head = String::new();
        if c1.is_ascii_alphabetic() && c2.is_ascii_alphabetic() && c3.is_ascii_alphabetic() {
            head.push(c1);
            head.push(c2);
            head.push(c3);
        } else if c1.is_ascii_alphabetic()
            && (c2.is_ascii_alphabetic() || c2.is_ascii_digit() || c2 == '@')
        {
            head.push(c1);
            head.push(c2);
        } else if c1 != '\0' {
            head.push(c1);
        }
        head
    }

    // ── Argument parsing ────────────────────────────────────────────────

    fn parse_args(&self, code: &str, raw: &str) -> Vec<ArgSlot> {
        let (sig_joiner, param_keys) = self.get_signature(code);

        // If the command's signature uses the default comma joiner, apply
        // any active delimiter change from ^CD/~CD.  Commands with custom
        // joiners (":", ".", etc.) are not affected by the delimiter change —
        // they use a fundamentally different separator syntax.
        let joiner = if sig_joiner == "," {
            self.delimiter.to_string()
        } else {
            sig_joiner
        };

        let raw_trimmed = raw.trim();

        let mut parts: Vec<String> = if raw_trimmed.is_empty() {
            Vec::new()
        } else {
            raw_trimmed.split(&joiner).map(|s| s.to_string()).collect()
        };

        // Spec-driven parameter splitting (e.g., ^A font+orientation → two parts)
        if let Some(split_rule) = self
            .lookup_command(code)
            .and_then(|ce| ce.signature.as_ref())
            .and_then(|sig| sig.split_rule.as_ref())
        {
            let idx = split_rule.param_index;
            if idx < parts.len() {
                let s = parts[idx].trim().to_string();
                let chars: Vec<char> = s.chars().collect();
                let total_chars: usize = split_rule.char_counts.iter().sum();
                if chars.len() >= total_chars {
                    let mut new_parts =
                        Vec::with_capacity(parts.len() + split_rule.char_counts.len() - 1);
                    // Parts before the split target
                    for p in &parts[..idx] {
                        new_parts.push(p.clone());
                    }
                    // Split the target param by char counts
                    let mut offset = 0;
                    for &count in &split_rule.char_counts {
                        let end = (offset + count).min(chars.len());
                        new_parts.push(chars[offset..end].iter().collect());
                        offset = end;
                    }
                    // Any remaining chars after the last split go with the last split part
                    if offset < chars.len()
                        && let Some(last) = new_parts.last_mut()
                    {
                        let remaining: String = chars[offset..].iter().collect();
                        last.push_str(&remaining);
                    }
                    // Parts after the split target
                    for p in parts.iter().skip(idx + 1) {
                        new_parts.push(p.trim().to_string());
                    }
                    parts = new_parts;
                }
            }
        }

        // Pad to param count if allow_empty_trailing
        if !param_keys.is_empty() {
            let allow_trailing = self
                .lookup_command(code)
                .and_then(|ce| ce.signature.as_ref())
                .map(|s| s.allow_empty_trailing)
                .unwrap_or(false);
            if allow_trailing && parts.len() < param_keys.len() {
                let missing = param_keys.len() - parts.len();
                for _ in 0..missing {
                    parts.push(String::new());
                }
            }
        }

        let mut args = Vec::new();
        for (idx, p) in parts.iter().enumerate() {
            let vtrim = p.trim();
            if vtrim.is_empty() {
                args.push(ArgSlot {
                    key: param_keys.get(idx).cloned(),
                    presence: Presence::Empty,
                    value: None,
                });
            } else {
                args.push(ArgSlot {
                    key: param_keys.get(idx).cloned(),
                    presence: Presence::Value,
                    value: Some(vtrim.to_string()),
                });
            }
        }
        args
    }

    fn get_signature(&self, code: &str) -> (String, Vec<String>) {
        if let Some(cmd) = self.lookup_command(code)
            && let Some(sig) = cmd.signature.as_ref()
        {
            return (sig.joiner.clone(), sig.params.clone());
        }
        (",".into(), Vec::new())
    }

    // ── Field data mode ─────────────────────────────────────────────────

    fn parse_field_data(&mut self) {
        let (content_start, hex_escape) = match self.mode {
            Mode::FieldData {
                content_start,
                hex_escape,
            } => (content_start, hex_escape),
            _ => unreachable!("parse_field_data called while not in FieldData mode"),
        };

        // Scan forward looking for ^FS (the field separator).
        // In field data mode, ALL content (including commas, values, whitespace) is field data.
        // Only a Leader token (^/~) can end field data mode.
        while !self.at_end() {
            match self.toks[self.pos].kind {
                TokKind::Leader => {
                    let leader_start = self.toks[self.pos].start;

                    // Check if the next token forms ^FS (using current command prefix)
                    let leader_char = self.toks[self.pos].text.chars().next().unwrap_or('\0');
                    let is_cmd_leader = leader_char == self.command_prefix;
                    if is_cmd_leader && self.pos + 1 < self.toks.len() {
                        let next = &self.toks[self.pos + 1];
                        if next.kind == TokKind::Value {
                            // Always use canonical "^" for spec lookups
                            let head = self.recognize_opcode("^", next.start);
                            let candidate = format!("^{}", head);
                            if self
                                .lookup_command(&candidate)
                                .is_some_and(|ce| ce.closes_field)
                            {
                                // Emit field data content (from content_start to leader_start)
                                let content = self.input[content_start..leader_start].to_string();
                                if !content.is_empty() {
                                    self.nodes.push(Node::FieldData {
                                        content,
                                        hex_escaped: hex_escape,
                                        span: Span::new(content_start, leader_start),
                                    });
                                }
                                // Switch back to normal mode and let the main loop process ^FS
                                self.mode = Mode::Normal;
                                self.fh_active = false;
                                return;
                            }
                        }
                    }

                    // Not ^FS — some other command is interrupting the field data.
                    // Identify the interrupting command for a clear diagnostic message.
                    // Use canonical prefix for spec lookups.
                    let interrupt_canonical = if leader_char == self.command_prefix {
                        "^"
                    } else {
                        "~"
                    };
                    let interrupter = if self.pos + 1 < self.toks.len()
                        && self.toks[self.pos + 1].kind == TokKind::Value
                    {
                        let head = self
                            .recognize_opcode(interrupt_canonical, self.toks[self.pos + 1].start);
                        if head.is_empty() {
                            interrupt_canonical.to_owned()
                        } else {
                            format!("{}{}", interrupt_canonical, head)
                        }
                    } else {
                        interrupt_canonical.to_owned()
                    };

                    // Emit what we have as field data
                    let content = self.input[content_start..leader_start].to_string();
                    if !content.is_empty() {
                        self.nodes.push(Node::FieldData {
                            content,
                            hex_escaped: hex_escape,
                            span: Span::new(content_start, leader_start),
                        });
                    }
                    self.diags.push(
                        Diagnostic::warn(
                            codes::PARSER_FIELD_DATA_INTERRUPTED,
                            format!("field data interrupted by {} before ^FS", interrupter),
                            Some(Span::new(leader_start, leader_start + 1)),
                        )
                        .with_context(ctx!("command" => interrupter)),
                    );
                    self.mode = Mode::Normal;
                    self.fh_active = false; // Reset ^FH on interruption
                    return;
                }
                _ => {
                    // All other tokens are part of the field data content
                    self.pos += 1;
                }
            }
        }

        // End of input without ^FS — handled by the main parse() cleanup
    }

    // ── Raw data mode ──────────────────────────────────────────────────

    /// Collect raw payload data until a command leader or end of input.
    fn parse_raw_data(&mut self) {
        // Extract mode state via replace to avoid borrowing `self` while mutating.
        let (command, content_start) = match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::RawData {
                command,
                content_start,
            } => (command, content_start),
            other => {
                // Restore the mode and bail — should never happen.
                self.mode = other;
                return;
            }
        };

        // Scan forward looking for a command leader (^ or ~) that starts a new command.
        while !self.at_end() {
            if self.toks[self.pos].kind == TokKind::Leader {
                let leader_start = self.toks[self.pos].start;
                let data = self.input[content_start..leader_start].to_string();
                if !data.is_empty() {
                    self.nodes.push(Node::RawData {
                        command,
                        data: Some(data),
                        span: Span::new(content_start, leader_start),
                    });
                }
                // mode is already Normal from the replace above
                return;
            }
            self.pos += 1;
        }

        // End of input: restore RawData mode so the main parse() cleanup handles it.
        self.mode = Mode::RawData {
            command,
            content_start,
        };
    }
}
