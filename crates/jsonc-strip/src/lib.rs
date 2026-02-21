//! Shared JSONC comment stripping utility.
//!
//! Supports:
//! - `//` line comments
//! - `/* ... */` block comments
//! - string literal preservation (including escapes)

/// Strip `//` and `/* */` comments from JSONC input.
///
/// Correctly handles escaped quotes inside strings and comment-like sequences
/// embedded in string literals.
#[must_use]
pub fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    let mut in_str = false;

    while i < len {
        let c = chars[i];

        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < len {
                // Escaped character inside string: keep next char verbatim.
                i += 1;
                out.push(chars[i]);
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == '/' && i + 1 < len {
            let c2 = chars[i + 1];
            if c2 == '/' {
                i += 2;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if c2 == '*' {
                i += 2;
                while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(len);
                continue;
            }
        }

        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_jsonc;

    #[test]
    fn strips_line_and_block_comments() {
        let input = r#"
{
  // comment
  "a": 1, /* inline */ "b": 2
}
"#;
        let stripped = strip_jsonc(input);
        assert!(!stripped.contains("comment"));
        assert!(!stripped.contains("inline"));
        assert!(stripped.contains("\"a\": 1"));
        assert!(stripped.contains("\"b\": 2"));
    }

    #[test]
    fn preserves_comment_like_text_in_strings() {
        let input = r#"{ "url": "http://example.com/*x*/", "note":"//keep" }"#;
        let stripped = strip_jsonc(input);
        assert!(stripped.contains("http://example.com/*x*/"));
        assert!(stripped.contains("\"note\":\"//keep\""));
    }
}
