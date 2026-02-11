//! ZPL emitter — converts an AST back into well-formatted ZPL text.
//!
//! All formatting decisions are spec-driven: joiners, split-rule merging,
//! trailing-arg handling, and structural indentation are derived from
//! [`ParserTables`] metadata. Field data and raw payloads are preserved
//! byte-for-byte.

use std::borrow::Cow;

use crate::grammar::ast::{ArgSlot, Ast, Label, Node, Presence};
use zpl_toolchain_diagnostics::Span;
use zpl_toolchain_spec_tables::ParserTables;

// ── Configuration ───────────────────────────────────────────────────────

/// Indentation style for formatted output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Indent {
    /// No indentation (flat). Matches conventional ZPL style.
    #[default]
    None,
    /// 2-space indent for commands inside `^XA`/`^XZ` blocks.
    Label,
    /// Label indent + additional 2-space indent inside field blocks
    /// (`^FO`...`^FS`).
    Field,
}

/// Configuration for the ZPL emitter.
#[derive(Debug, Clone, Default)]
pub struct EmitConfig {
    /// Indentation style.
    pub indent: Indent,
}

// ── Public API ──────────────────────────────────────────────────────────

/// Emit a formatted ZPL string from a parsed AST.
///
/// When `tables` is provided, command reconstruction uses spec-driven
/// metadata (signature joiners, split rules, structural flags). Without
/// tables the emitter falls back to comma-joined args.
pub fn emit_zpl(ast: &Ast, tables: Option<&ParserTables>, config: &EmitConfig) -> String {
    let mut out = String::new();
    for label in &ast.labels {
        emit_label(&mut out, label, tables, config);
    }
    out
}

// ── Label emission ──────────────────────────────────────────────────────

fn emit_label(out: &mut String, label: &Label, tables: Option<&ParserTables>, config: &EmitConfig) {
    let mut in_label = false;
    let mut in_field = false;
    // Track current command prefix (^CC changes it from '^').
    let mut cmd_prefix: char = '^';

    for node in &label.nodes {
        match node {
            Node::Command { code, args, .. } => {
                // The parser normalizes all codes to canonical '^' prefix,
                // so `code` is directly usable for table lookups.
                let is_xa = code == "^XA";
                let is_xz = code == "^XZ";

                // Dedent BEFORE indenting for ^XZ.
                if is_xz {
                    in_field = false;
                    in_label = false;
                }

                // Single table lookup for structural flags.
                let entry = tables.and_then(|t| t.cmd_by_code(code));
                let opens_field = entry.is_some_and(|ce| ce.opens_field);
                let closes_field = entry.is_some_and(|ce| ce.closes_field);

                // Indent BEFORE updating closes_field so that ^FS (the field
                // closer) is still indented at the field level.
                push_indent(out, config, in_label, in_field);

                if closes_field {
                    in_field = false;
                }

                // Emit the command with the current prefix.
                emit_command(out, code, cmd_prefix, args, tables);
                out.push('\n');

                // Track prefix changes: ^CC sets the command (^) prefix.
                if code == "^CC"
                    && let Some(arg) = args.first()
                    && arg.presence == Presence::Value
                    && let Some(val) = &arg.value
                    && let Some(ch) = val.chars().next()
                {
                    cmd_prefix = ch;
                }

                // Update nesting state AFTER emitting.
                if is_xa {
                    in_label = true;
                }
                if opens_field {
                    in_field = true;
                }
            }

            Node::FieldData { content, .. } => {
                // Field data is emitted verbatim directly after its ^FD/^FV
                // command. The preceding Command node already pushed a
                // newline, so we remove it and glue the content inline.
                //
                // AST pattern:  Command(^FD) → FieldData → Command(^FS)
                // Output:       ^FDcontent\n^FS\n
                trim_trailing_newline(out);
                out.push_str(content);
                out.push('\n');
            }

            Node::RawData { data, .. } => {
                // Raw payload data is emitted verbatim. It may contain
                // newlines (multi-line hex data for ^GF).
                if let Some(d) = data {
                    trim_trailing_newline(out);
                    out.push_str(d);
                    if !d.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }

            Node::Trivia { text, .. } => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    // Pure whitespace trivia — skip (formatter controls ws).
                    continue;
                }
                push_indent(out, config, in_label, in_field);
                out.push_str(trimmed);
                out.push('\n');
            }
        }
    }
}

// ── Command emission ────────────────────────────────────────────────────

/// Emit a single command with its args.
///
/// `code` is the canonical code (e.g., `"^FO"`).
/// `prefix` is the current command prefix character (default `'^'`).
fn emit_command(
    out: &mut String,
    code: &str,
    prefix: char,
    args: &[ArgSlot],
    tables: Option<&ParserTables>,
) {
    // Emit the command code, remapping the prefix if ^CC changed it.
    let display_code = remap_prefix(code, prefix);
    out.push_str(&display_code);

    if args.is_empty() {
        return;
    }

    // Look up spec metadata using the canonical code.
    let entry = tables.and_then(|t| t.cmd_by_code(code));
    let sig = entry.and_then(|e| {
        e.signature_overrides
            .as_ref()
            .and_then(|ov| ov.get(code))
            .or(e.signature.as_ref())
    });

    let joiner = sig.map_or(",", |s| s.joiner.as_str());
    let split_rule = sig.and_then(|s| s.split_rule.as_ref());
    let no_space_after_opcode = sig.is_none_or(|s| s.no_space_after_opcode);
    if !no_space_after_opcode {
        out.push(' ');
    }

    // Convert args to string values, handling presence.
    let arg_values: Vec<&str> = args
        .iter()
        .map(|a| match a.presence {
            Presence::Value => a.value.as_deref().unwrap_or(""),
            Presence::Empty | Presence::Unset => "",
        })
        .collect();

    // Apply split-rule merging or pass through without allocating.
    let (merged, merged_len) = if let Some(rule) = split_rule {
        let m = merge_split_args(&arg_values, rule.param_index, rule.char_counts.len());
        let len = m.len();
        (MergedArgs::Owned(m), len)
    } else {
        (MergedArgs::Borrowed(&arg_values), arg_values.len())
    };

    // Trim trailing empty args after the last Value arg.
    let merged_idx_of = |orig_i: usize| -> usize {
        if let Some(rule) = split_rule {
            let split_count = rule.char_counts.len();
            let split_idx = rule.param_index;
            if orig_i < split_idx {
                orig_i
            } else if orig_i < split_idx + split_count {
                split_idx
            } else {
                orig_i - (split_count - 1)
            }
        } else {
            orig_i
        }
    };

    let last_value = args
        .iter()
        .enumerate()
        .rev()
        .find(|(_, a)| a.presence == Presence::Value)
        .map(|(i, _)| merged_idx_of(i));

    let trim_to = match last_value {
        Some(idx) => (idx + 1).min(merged_len),
        None => return, // No values at all — emit no args.
    };

    // Join and write.
    for i in 0..trim_to {
        if i > 0 {
            out.push_str(joiner);
        }
        out.push_str(merged.get(i));
    }
}

/// Either borrowed `&[&str]` (no split rule) or owned `Vec<String>` (after merge).
/// Avoids allocating a `Vec<String>` in the common case.
enum MergedArgs<'a> {
    Borrowed(&'a [&'a str]),
    Owned(Vec<String>),
}

impl MergedArgs<'_> {
    fn get(&self, i: usize) -> &str {
        match self {
            MergedArgs::Borrowed(s) => s[i],
            MergedArgs::Owned(v) => &v[i],
        }
    }
}

/// Merge split-rule expanded args back into a single glued value.
///
/// During parsing, a split rule at `param_index` expands one raw param
/// into `split_count` args. We reverse this by concatenating them.
fn merge_split_args(values: &[&str], param_index: usize, split_count: usize) -> Vec<String> {
    let mut result = Vec::with_capacity(values.len().saturating_sub(split_count - 1));

    for (i, val) in values.iter().enumerate() {
        if i == param_index {
            let end = (param_index + split_count).min(values.len());
            result.push(values[param_index..end].concat());
        } else if i > param_index && i < param_index + split_count {
            continue; // Interior of split group — already merged.
        } else {
            result.push(val.to_string());
        }
    }

    result
}

// ── Indentation helpers ─────────────────────────────────────────────────

fn push_indent(out: &mut String, config: &EmitConfig, in_label: bool, in_field: bool) {
    match config.indent {
        Indent::None => {}
        Indent::Label => {
            if in_label {
                out.push_str("  ");
            }
        }
        Indent::Field => {
            if in_label {
                out.push_str("  ");
            }
            if in_field {
                out.push_str("  ");
            }
        }
    }
}

fn trim_trailing_newline(out: &mut String) {
    if out.ends_with('\n') {
        out.truncate(out.len() - 1);
    }
}

// ── Prefix helpers ──────────────────────────────────────────────────────

/// Replace the leading `^` in a canonical code with the current command
/// prefix. Returns a borrowed reference in the common case (prefix == `^`)
/// to avoid allocation.
fn remap_prefix<'a>(code: &'a str, prefix: char) -> Cow<'a, str> {
    if prefix == '^' {
        return Cow::Borrowed(code);
    }
    if let Some(rest) = code.strip_prefix('^') {
        let mut result = String::with_capacity(code.len());
        result.push(prefix);
        result.push_str(rest);
        Cow::Owned(result)
    } else {
        Cow::Borrowed(code)
    }
}

// ── Helpers for comparing ASTs without spans ────────────────────────────

/// Strip all spans from an AST for comparison (used in round-trip tests).
///
/// Sets all spans to the sentinel value `Span { start: 0, end: 0 }`.
pub fn strip_spans(ast: &Ast) -> Ast {
    let sentinel = Span::new(0, 0);
    Ast {
        labels: ast
            .labels
            .iter()
            .map(|label| Label {
                nodes: label
                    .nodes
                    .iter()
                    .map(|node| match node {
                        Node::Command { code, args, .. } => Node::Command {
                            code: code.clone(),
                            args: args.clone(),
                            span: sentinel,
                        },
                        Node::FieldData {
                            content,
                            hex_escaped,
                            ..
                        } => Node::FieldData {
                            content: content.clone(),
                            hex_escaped: *hex_escaped,
                            span: sentinel,
                        },
                        Node::RawData { command, data, .. } => Node::RawData {
                            command: command.clone(),
                            data: data.clone(),
                            span: sentinel,
                        },
                        Node::Trivia { text, .. } => Node::Trivia {
                            text: text.clone(),
                            span: sentinel,
                        },
                    })
                    .collect(),
            })
            .collect(),
    }
}
