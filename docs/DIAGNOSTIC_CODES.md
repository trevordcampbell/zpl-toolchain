# ZPL Diagnostic Codes Reference

This document provides a comprehensive reference for all diagnostic codes emitted by the ZPL toolchain validator and parser.

## Numbering Scheme

The diagnostic code numbering follows a hierarchical scheme:

- **`ZPL1xxx`** — Single-command validation checks (arity, type, range, profile, presence)
- **`ZPL2xxx`** — Multi-command and structural checks (cross-command constraints, structure, semantics)
- **`ZPL3xxx`** — Informational notes
- **`ZPL.PARSER.xxxx`** — Parser-level diagnostics (syntax errors, missing terminators, etc.)

## Structured Context

Every diagnostic may carry an optional **`context`** field — a `BTreeMap<String, String>` of machine-readable key-value pairs that provide structured detail about what triggered the diagnostic. Context is designed for tooling, filtering, and programmatic consumption; it supplements (not replaces) the human-readable `message`.

When serialized to JSON, `context` appears only when present and keys are in deterministic alphabetical order (BTreeMap).

Common context keys include:

| Key | Description |
|-----|-------------|
| `command` | The ZPL command code (e.g., `^PW`, `^BC`) |
| `arg` | The argument name that triggered the diagnostic |
| `value` | The actual value that caused the issue |
| `field` | Profile field path (for profile constraints) |
| `op` | Comparison operator (for profile constraints) |
| `limit` | Profile limit value (for profile constraints) |
| `gate` | Required hardware gate name (for printer gates) |
| `level` | Gate level: `"command"` or `"enum"` |
| `kind` | Category: `"mode"`, `"tracking"`, `"method"`, `"requires"`, `"incompatible"`, `"order"` |
| `min` / `max` | Range bounds (for range checks) |
| `supported` | List of supported values (for media checks) |
| `profile` | Profile ID |
| `expected` | Expected token (for parser diagnostics) |
| `epsilon` | Rounding tolerance threshold (for rounding policy checks) |

## Diagnostic Codes

### 11xx: Arity & Value Validation

#### ZPL1101 — Too Many Arguments
- **Severity**: Error
- **Category**: Arity
- **Description**: Too many arguments provided compared to the command's arity.
- **Example**: `^BC,N,10,Y,N,N,extra` — ^BC has arity 6, but 7 arguments given
- **Fix**: Remove extra arguments.
- **Context keys**: `command`, `arity`, `actual`

#### ZPL1103 — Invalid Enumerated Value
- **Severity**: Error
- **Category**: Value Validation
- **Description**: Argument value is not one of the allowed enumerated values.
- **Example**: `^BY3,X` — ^BY expects 'A', 'B', or 'N' for the second argument, but 'X' was provided
- **Fix**: Use one of the allowed enumerated values.
- **Context keys**: `command`, `arg`, `value`

#### ZPL1104 — Empty Field Data
- **Severity**: Error
- **Category**: Value Validation
- **Description**: Field data (^FD/^FV) is present but empty; often unintended.
- **Example**: `^FO10,10^FD^FS` — ^FD has no content
- **Fix**: Add content to the field data or remove the empty ^FD/^FV command.
- **Context keys**: `command`

#### ZPL1105 — String Too Short
- **Severity**: Error
- **Category**: Value Validation
- **Description**: String is shorter than the minimum length allowed.
- **Example**: `^FN1` — ^FN requires at least 1 character, but empty string provided
- **Fix**: Ensure the string meets the minimum length requirement.
- **Context keys**: `command`, `arg`, `value`, `min_length`, `actual_length`

#### ZPL1106 — String Too Long
- **Severity**: Error
- **Category**: Value Validation
- **Description**: String exceeds the maximum length allowed.
- **Example**: A field data string that exceeds the maximum allowed length for the command
- **Fix**: Truncate or shorten the string to meet the maximum length requirement.
- **Context keys**: `command`, `arg`, `value`, `max_length`, `actual_length`

#### ZPL1107 — Expected Integer
- **Severity**: Error
- **Category**: Value Validation
- **Description**: Argument expected an integer value but received a non-integer string.
- **Example**: `^FO10.5,20` — ^FO expects integer coordinates, but '10.5' was provided
- **Fix**: Provide an integer value.
- **Context keys**: `command`, `arg`, `value`

#### ZPL1108 — Expected Numeric
- **Severity**: Error
- **Category**: Value Validation
- **Description**: Argument expected a numeric value but received a non-numeric string.
- **Example**: `^LLabc` — ^LL expects a numeric label length, but 'abc' was provided
- **Fix**: Provide a numeric value.
- **Context keys**: `command`, `arg`, `value`

#### ZPL1109 — Expected Single Character
- **Severity**: Error
- **Category**: Value Validation
- **Description**: Argument expected a single character but received a multi-character or empty string.
- **Example**: `^AAB,10,10` — ^AA expects a single character font identifier, but 'AB' was provided
- **Fix**: Provide exactly one character.
- **Context keys**: `command`, `arg`, `value`

### 12xx: Range & Rounding

#### ZPL1201 — Value Outside Range
- **Severity**: Error
- **Category**: Range
- **Description**: Value is outside the allowed numeric range for this argument.
- **Example**: `^LL10000` — ^LL value exceeds the maximum allowed label length
- **Fix**: Adjust the value to be within the allowed range.
- **Context keys**: `command`, `arg`, `value`, `min`, `max`

#### ZPL1202 — Rounding Policy Violation
- **Severity**: Warn
- **Category**: Rounding
- **Description**: Value does not conform to the rounding policy (e.g., not a multiple).
- **Example**: `^PW203` — ^PW value should be a multiple of 8, but 203 is not
- **Fix**: Round the value to conform to the rounding policy.
- **Context keys**: `command`, `arg`, `value`, `multiple`, `epsilon`

### 14xx: Profile Constraints

#### ZPL1401 — Profile Constraint Violation
- **Severity**: Error
- **Category**: Profile Constraints
- **Description**: Value violates a profile constraint (e.g., exceeds page width).
- **Example**: `^PW1000` — Page width exceeds the configured profile maximum
- **Fix**: Adjust the value to comply with the profile constraints.
- **Context keys**: `command`, `arg`, `field`, `op`, `limit`, `actual`

#### ZPL1402 — Printer Gate Violation
- **Severity**: Error (command-level) / Warn (enum value-level)
- **Category**: Profile Constraints
- **Description**: A command, argument, or enum value requires a printer capability (`printerGate`) that is not declared in the loaded profile's feature set.
- **Example**: `^RF` with `rfid: false` — RFID command used with a profile that lacks RFID hardware
- **Fix**: Use a different command/value supported by your printer, or update the profile's `features` section.
- **Context keys**: `command`, `gate`, `level` (`"command"` or `"enum"`), `profile` (+ `arg`, `value` for enum-level)

#### ZPL1403 — Media Mode Unsupported
- **Severity**: Warn
- **Category**: Profile Constraints
- **Description**: A command selects a media mode, tracking method, or print type that is not listed in the loaded profile's media capabilities.
- **Example**: `^MMC` — Cutter mode selected but profile's `supported_modes` only includes `["T"]`
- **Fix**: Select a media mode/tracking/type that is supported by your profile's `media` configuration.
- **Context keys**: `command`, `kind` (`"mode"`, `"tracking"`, `"method"`), `value`, `supported` / `profile_method`, `profile`

### 15xx: Presence

#### ZPL1501 — Required Argument Missing
- **Severity**: Error
- **Category**: Presence
- **Description**: A required argument is missing or unset.
- **Example**: `^BC` — ^BC requires at least one argument, but none provided
- **Fix**: Provide all required arguments.
- **Context keys**: `command`, `arg`

#### ZPL1502 — Empty Required Value
- **Severity**: Warn
- **Category**: Presence
- **Description**: An argument is empty but required to have a value.
- **Example**: `^FN` — ^FN requires a field number value, but empty string provided
- **Fix**: Provide a non-empty value for the required argument.
- **Context keys**: `command`, `arg`

### 21xx: Cross-Command Constraints

#### ZPL2101 — Required Command Missing
- **Severity**: Warn
- **Category**: Cross-Command Constraints
- **Description**: A required command was not found in the label where expected.
- **Example**: Using ^FD without a preceding ^FO or ^FT
- **Fix**: Add the required command in the correct location.
- **Context keys**: `command`, `target`, `kind` (`"requires"`), `scope` (`"label"` or `"field"`)

#### ZPL2102 — Incompatible Commands
- **Severity**: Warn
- **Category**: Cross-Command Constraints
- **Description**: This command is incompatible with another present in the label.
- **Example**: Using both ^FO and ^FT in conflicting ways
- **Fix**: Remove one of the incompatible commands or restructure the label.
- **Context keys**: `command`, `target`, `kind` (`"incompatible"`), `scope` (`"label"` or `"field"`)

#### ZPL2103 — Ordering Violation (Before)
- **Severity**: Warn
- **Category**: Cross-Command Constraints
- **Description**: Command ordering rule violated: this command should appear before the referenced one.
- **Example**: ^LL appears after ^FO when it should appear before
- **Fix**: Reorder commands to satisfy the ordering constraint.
- **Context keys**: `command`, `target`, `kind` (`"order"`), `scope` (`"label"` or `"field"`)

#### ZPL2104 — Ordering Violation (After)
- **Severity**: Warn
- **Category**: Cross-Command Constraints
- **Description**: Command ordering rule violated: this command should appear after the referenced one.
- **Example**: ^FD appears before ^FO when it should appear after
- **Fix**: Reorder commands to satisfy the ordering constraint.
- **Context keys**: `command`, `target`, `kind` (`"order"`), `scope` (`"label"` or `"field"`)

### 22xx: Structural Validation

#### ZPL2201 — Field Data Without Origin
- **Severity**: Warn
- **Category**: Structural Validation
- **Description**: Field data command (^FD/^FV) without a preceding field origin (^FO/^FT).
- **Example**: `^XA^FDHello^FS^XZ` — ^FD appears without a preceding ^FO or ^FT
- **Fix**: Add a ^FO or ^FT command before the ^FD/^FV command.
- **Context keys**: `command`

#### ZPL2202 — Empty Label
- **Severity**: Info
- **Category**: Structural Validation
- **Description**: Empty label with no commands between ^XA and ^XZ.
- **Example**: `^XA^XZ` — No commands between label start and end
- **Fix**: Add commands to the label or remove the empty label.

#### ZPL2203 — Field Origin Before Previous Closed
- **Severity**: Warn
- **Category**: Structural Validation
- **Description**: Field origin (^FO/^FT) opens a new field before the previous field was closed with ^FS.
- **Example**: `^FO10,10^FDHello^FO20,20^FDWorld^FS` — Second ^FO appears before first field is closed
- **Fix**: Add ^FS to close the previous field before starting a new one.
- **Context keys**: `command`

#### ZPL2204 — Field Separator Without Origin
- **Severity**: Warn
- **Category**: Structural Validation
- **Description**: Field separator (^FS) without a preceding field origin (^FO/^FT).
- **Example**: `^XA^FS^XZ` — ^FS appears without a preceding ^FO or ^FT
- **Fix**: Add a ^FO or ^FT command before the ^FS command.
- **Context keys**: `command`

#### ZPL2205 — Host Command in Label
- **Severity**: Warn
- **Category**: Structural Validation
- **Description**: Host or device command appearing inside a label (between ^XA and ^XZ).
- **Example**: `^XA~TA^XZ` — ~TA (host command) should not appear inside a label
- **Fix**: Move the host/device command outside the label boundaries.
- **Context keys**: `command`, `plane`

### 23xx: Semantic Validation

#### ZPL2301 — Duplicate Field Number
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: Duplicate field number (^FN) — same number used multiple times in a label.
- **Example**: `^FN1^FN1` — Field number 1 is assigned twice
- **Fix**: Use unique field numbers or remove the duplicate assignment.
- **Context keys**: `command`, `field_number`

#### ZPL2302 — Position Exceeds Dimensions
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: Field position (^FO/^FT) exceeds label dimensions set by ^PW/^LL or profile.
- **Example**: `^PW100^FO150,10` — X coordinate 150 exceeds page width of 100
- **Fix**: Adjust the field position to be within label dimensions.
- **Context keys**: `command`, `axis` (`"x"` or `"y"`), `value`, `limit`

#### ZPL2303 — Font Not Loaded
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: Font referenced by ^A is not a built-in font (A-Z, 0-9) and has not been loaded via ^CW.
- **Example**: `^A@,10,10` — Font '@' is not a built-in font and hasn't been loaded
- **Fix**: Use a built-in font or load the custom font with ^CW before use.
- **Context keys**: `command`, `font`

#### ZPL2304 — Invalid Hex Escape Sequence
- **Severity**: Error
- **Category**: Semantic Validation
- **Description**: Invalid hex escape sequence in field data when ^FH is active. The indicator character defaults to `_` but can be changed via the `^FH` command's optional argument (e.g., `^FH#` sets `#` as the indicator). Sequences must be `{indicator}XX` where XX are hex digits (0-9, A-F).
- **Example**: `^FH^FD_XY^FS` — '_XY' is not a valid hex escape (should be _XX where XX are hex digits)
- **Fix**: Use valid hex escape sequences (_00 through _FF) or disable ^FH.
- **Context keys**: `command` (`"^FH"`), `indicator` (the active indicator character, e.g. `"_"` or `"#"`)

#### ZPL2305 — State Override Unused
- **Severity**: Info
- **Category**: Semantic Validation
- **Description**: State-setting command overrides a previous one without any consumer using the earlier value.
- **Example**: `^CFA,10^CFB,12` — First ^CF is overridden before being used
- **Fix**: Remove the unused state-setting command or use it before overriding.
- **Context keys**: `command`

#### ZPL2306 — Serialization Without Field Number
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: Serialization command (^SN/^SF) used in a field without a ^FN field number assignment.
- **Example**: `^FO10,10^FDHello^SN^FS` — ^SN used without a preceding ^FN
- **Fix**: Add a ^FN command before the serialization command.
- **Context keys**: `command` (`"^SN/^SF"`)

#### ZPL2307 — Graphic Data Length Mismatch
- **Severity**: Error
- **Category**: Semantic Validation
- **Description**: ^GF graphic data length does not match declared binary_byte_count for the given compression format.
- **Example**: `^GFA,100,100,50,data` — Data length doesn't match the declared byte count
- **Fix**: Ensure the graphic data length matches the declared binary_byte_count.
- **Context keys**: `command`, `format`, `declared`, `actual`, `expected`

#### ZPL2308 — Graphic Field Exceeds Label Bounds
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: The ^GF graphic at the current ^FO position would extend beyond the effective label dimensions. This may cause truncated or misaligned output.
- **Example**: `^PW400^FO300,0^GFA,100,100,10,data` — Graphic width (10×8=80 dots) at x=300 exceeds label width of 400
- **Fix**: Adjust the ^FO position or reduce the graphic size to fit within label bounds.
- **Context keys**: `command`, `x`, `y`, `graphic_width`, `graphic_height`, `label_width`, `label_height`

#### ZPL2309 — Graphic Memory Usage Exceeds Available RAM
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: Total graphic field data in this label exceeds the printer's available RAM. This may cause print failures or data loss.
- **Example**: Multiple `^GF` commands whose combined `graphic_field_count` exceeds `memory.ram_kb * 1024`
- **Fix**: Reduce the number or size of graphic fields, or use a printer with more memory.
- **Context keys**: `command`, `total_bytes`, `ram_bytes`

#### ZPL2310 — Missing Explicit Label Dimensions
- **Severity**: Info
- **Category**: Semantic Validation
- **Description**: Label uses profile-provided dimensions but does not contain explicit ^PW or ^LL commands. Adding explicit dimension commands makes the label self-contained and portable across printers.
- **Example**: A label validated with a profile that has `page.width_dots` but no `^PW` command in the label
- **Fix**: Add explicit `^PW` and/or `^LL` commands to the label for portability.
- **Context keys**: `missing_commands`

#### ZPL2311 — Text/Barcode Extends Beyond Label Bounds
- **Severity**: Warn
- **Category**: Semantic Validation
- **Description**: A text field or barcode at the current ^FO/^FT position would extend beyond the effective label dimensions. Content may be clipped or misaligned on print.
- **Implementation note (current)**: This preflight is estimate-based (heuristic), not pixel-accurate rendering. It is designed to be fast and renderer-independent, so edge-case false positives/negatives are possible.
- **Confidence model**: Marginal estimated overflow is tagged as low-confidence and downgraded to informational severity with "may extend" wording; larger overflow remains warn-level with high confidence.
- **Example**: Text at x=50 with 30×30 font and 20 chars (600 dots wide) on a 100-dot label; barcode at y=50 with 30-dot height on a 60-dot label
- **Fix**: Reduce font size, shorten text, move origin, or increase label dimensions.
- **Context keys**: `object_type`, `x`, `y`, `estimated_width`, `estimated_height`, `label_width`, `label_height`, `overflow_x`, `overflow_y`, `overflow_x_ratio`, `overflow_y_ratio`, `confidence`, `audience`

### 24xx: Barcode Field Data Validation

#### ZPL2401 — Invalid Barcode Data Character
- **Severity**: Error
- **Category**: Barcode Validation
- **Description**: Field data contains characters not allowed by the active barcode's character set.
- **Example**: `^BE,50^FDABCDEF123456^FS` — EAN-13 only allows digits 0-9, but 'A' was found
- **Fix**: Use only characters allowed by the barcode symbology.
- **Context keys**: `command`, `character`, `position`, `allowedSet`

#### ZPL2402 — Barcode Data Length Violation
- **Severity**: Warn
- **Category**: Barcode Validation
- **Description**: Field data length violates the active barcode's length requirements (exact, min/max, or parity).
- **Example**: `^BE,50^FD12345^FS` — EAN-13 requires exactly 12 digits, but only 5 provided
- **Fix**: Adjust field data to meet the barcode's length requirements.
- **Context keys**: `command`, `actual`, `expected` / `min` / `max` / `parity` / `actualParity`

### 30xx: Notes

#### ZPL3001 — Informational Note
- **Severity**: Info
- **Category**: Notes
- **Description**: Informational note about command usage or behavior.
- **Example**: Various informational messages about command behavior or best practices
- **Fix**: Review the note and adjust code if needed.
- **Context keys**: `command`

### Parser Diagnostics

#### ZPL.PARSER.0001 — No Labels Detected
- **Severity**: Info
- **Category**: Parser
- **Description**: No labels detected in the input.
- **Example**: Empty input or input without ^XA/^XZ pairs
- **Fix**: Ensure the input contains at least one label (^XA...^XZ).

#### ZPL.PARSER.1001 — Invalid or Missing Command Code
- **Severity**: Error
- **Category**: Parser
- **Description**: Invalid or missing command code after leader (^ or ~), spacing violation between opcode and arguments, or reserved raw leader characters used inside command-defined free-form text segments (for example, inside `^FX` comment bodies).
- **Example**: `^X` (incomplete code), `^FO 10,10` when `spacingPolicy=forbid`, or `^FX Comment with ^ character^FS`.
- **Fix**: Provide a valid command code after the leader, match signature spacing policy, and avoid raw `^`/`~` inside free-form text (encode/remove them).
- **Context keys**: `command` (the leader character/opcode), optional `spacing` (`spacingPolicy=forbid|require`)

#### ZPL.PARSER.1002 — Unknown Command Code
- **Severity**: Warn
- **Category**: Parser
- **Description**: Unknown command code (not in the command spec tables).
- **Example**: `^XX` — Command code 'XX' is not recognized
- **Fix**: Use a valid ZPL command code or check for typos.
- **Context keys**: `command`

#### ZPL.PARSER.1102 — Missing Label Terminator
- **Severity**: Error
- **Category**: Parser
- **Description**: Missing label terminator (^XZ).
- **Example**: `^XA^FO10,10^FDHello^FS` — Label starts with ^XA but never ends with ^XZ
- **Fix**: Add ^XZ to properly terminate the label.
- **Context keys**: `expected` (`"^XZ"`)

#### ZPL.PARSER.1202 — Missing Field Separator
- **Severity**: Error
- **Category**: Parser
- **Description**: Missing field separator (^FS) before label end or end of input.
- **Example**: `^FO10,10^FDHello^XZ` — Field is not closed with ^FS before label end
- **Fix**: Add ^FS to close the field before the label ends.
- **Context keys**: `expected` (`"^FS"`), `command` (when interrupting command is known)

#### ZPL.PARSER.1203 — Field Data Interrupted
- **Severity**: Warn
- **Category**: Parser
- **Description**: Field data interrupted by another command before ^FS.
- **Example**: `^FDHello^FO20,20^FS` — Field data is interrupted by ^FO before ^FS
- **Fix**: Close the field with ^FS before starting a new field.
- **Context keys**: `command` (the interrupting command)

#### ZPL.PARSER.1301 — Stray Content
- **Severity**: Warn
- **Category**: Parser
- **Description**: Stray content (text or punctuation) found outside of a command context.
- **Example**: `^XA^FDHello^FSsome stray text^XZ` — "some stray text" appears between commands without a leader
- **Fix**: Remove the stray content or wrap it in a proper command (e.g., `^FD...^FS`).

#### ZPL.PARSER.1302 — Non-ASCII Argument
- **Severity**: Error
- **Category**: Parser
- **Description**: Prefix/delimiter change commands (^CC, ^CT, ^CD) require an ASCII character argument. Non-ASCII characters cannot be used as command prefixes or delimiters because the lexer operates on single bytes.
- **Example**: `^CCé` — non-ASCII character 'é' used as command prefix
- **Fix**: Use a single ASCII character (0x00–0x7F) as the prefix or delimiter.
- **Context keys**: `command` (the prefix/delimiter change command)

---

## Machine-Readable Spec

The canonical machine-readable version of this document lives at `crates/diagnostics/spec/diagnostics.jsonc` (v1.1.0). Each entry includes a `constName` (Rust constant name), a `contextKeys` array declaring structured context keys, and optional `messageTemplates` for variant-specific message formatting. This file is the **single source of truth** — `crates/diagnostics/build.rs` auto-generates Rust diagnostic constants and lookup helpers from it at build time. To add a new diagnostic, add an entry to `diagnostics.jsonc` and the Rust code regenerates automatically.
