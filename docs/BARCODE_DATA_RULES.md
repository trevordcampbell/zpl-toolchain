# Barcode ^FD Data Format Validation Rules

> Reference document for per-barcode `^FD` data format validation.
> Each barcode's `fieldDataRules` in its spec file defines the character set,
> length, and parity constraints that the validator enforces at `^FS` time.
>
> Source: Zebra ZPL II Programming Guide (zpl-zbi2-programming-guide-en.pdf)
>
> Status: **Implemented** — all 29 barcode spec files audited against PDF

---

## How It Works

The `fieldDataRules` property in each barcode's JSONC spec file drives automated validation:

```jsonc
"fieldDataRules": {
  "characterSet": "0-9",       // Compact charset notation (ASCII ranges/literals)
  "exactLength": 12,           // Exact required length (shorthand for min==max)
  "minLength": 1,              // Minimum data length
  "maxLength": 14,             // Maximum data length
  "lengthParity": "even",      // Required parity ("even" or "odd")
  "notes": "Human-readable"    // Informational — not used for validation
}
```

At `^FS`, the validator checks any `^FD`/`^FV` content against the active barcode's rules:
- **ZPL2401** (error): Invalid character in field data
- **ZPL2402** (warn): Data length violation

Validation is **skipped** when `^FH` (hex escape) is active, since raw hex-escaped content would cause false positives.

---

## Barcode Rules Summary

### Fixed-Format Numeric Barcodes

| Barcode | Command | `characterSet` | Length | Parity | Check Digit |
|---------|---------|----------------|--------|--------|-------------|
| EAN-13 | `^BE` | `0-9` | `exactLength: 12` | — | Mod 10 (auto) |
| EAN-8 | `^B8` | `0-9` | `exactLength: 7` | — | Mod 10 (auto) |
| UPC-A | `^BU` | `0-9` | `exactLength: 11` | — | Mod 10 (auto) |
| UPC-E | `^B9` | `0-9` | `exactLength: 10` | — | Mod 10 (auto) |
| I 2of5 | `^B2` | `0-9` | — | `even` | Optional Mod 10 |
| UPC/EAN Ext | `^BS` | `0-9` | `allowedLengths: [2, 5]` | — | — |

### Variable-Length Numeric Barcodes

| Barcode | Command | `characterSet` | Length | Notes |
|---------|---------|----------------|--------|-------|
| Code 11 | `^B1` | `0-9\-` | — | Digits and dash |
| Ind. 2of5 | `^BI` | `0-9` | — | Numeric only |
| Std. 2of5 | `^BJ` | `0-9` | — | Numeric only |
| MSI | `^BM` | `0-9` | `min: 1`, `max: 14` | Max 13 when e=A |
| Planet | `^B5` | `0-9` | — | Length depends on format |
| Plessey | `^BP` | `0-9A-F` | — | Hex digits per Plessey standard |
| POSTAL | `^BZ` | `0-9` | `max: 31` | Type-dependent lengths |
| GS1 DataBar | `^BR` | *(none)* | — | Type-dependent; type 6 is alphanumeric |

### Variable-Length Alphanumeric Barcodes

| Barcode | Command | `characterSet` | Length | Notes |
|---------|---------|----------------|--------|-------|
| Code 39 | `^B3` | `A-Z0-9 \-.$/+%` | — | Extended mode: full ASCII via pairs |
| LOGMARS | `^BL` | `A-Z0-9 \-.$/+%` | — | Code 39 variant; Mod 43 mandatory |
| Codabar | `^BK` | `0-9\-$:/.+` | — | Start/stop A-D via params, not data |

### Full ASCII / Binary Barcodes (notes only)

| Barcode | Command | `maxLength` | Key Notes |
|---------|---------|-------------|-----------|
| Code 93 | `^BA` | — | Full 128-char ASCII; paired substitutes |
| Code 128 | `^BC` | — | Subsets A/B/C; Mode U = 19 digits |
| Code 49 | `^B4` | — | Full ASCII; multi-row stacked |
| PDF417 | `^B7` | 3072 | Full ASCII; `\&` for CR/LF |
| MicroPDF417 | `^BF` | 250 | 250 ASCII / 150 8-bit / 366 numeric |
| CODABLOCK | `^BB` | — | Mode A=Code 39; Mode E/F=full ASCII |
| TLC39 | `^BT` | 150 | Structured: 6-digit ECI + serial fields |

### 2D Matrix Symbologies (notes only)

| Barcode | Command | `maxLength` | Key Notes |
|---------|---------|-------------|-----------|
| QR Code | `^BQ` | — | Structured ^FD with switch fields |
| Data Matrix | `^BX` | 3072 | Format-dependent; quality 200 = ECC 200 |
| Aztec | `^B0`/`^BO` | — | Full 8-bit binary; ECI support |
| MaxiCode | `^BD` | 138 | Mode 2/3: structured postal hpm/lpm |

---

## `char_in_set()` Notation

The `characterSet` strings use a compact notation parsed by `char_in_set()`:

- **Ranges**: `A-Z`, `0-9`, `a-z` (reversed ranges like `Z-A` are auto-normalized)
- **Literal characters**: `$`, `/`, `+`, space
- **Escaped characters**: `\\-` for literal dash, `\\.` for literal dot

**Limitation**: ASCII-only. Multi-byte UTF-8 characters in charset strings are not supported.

---

## Design Decisions

1. **Barcodes with mode-dependent rules** (`^BC`, `^BR`, `^BZ`, `^BX`, `^BQ`) use notes-only `fieldDataRules` because no single `characterSet` covers all modes. Automated validation is limited to `maxLength` where applicable.

2. **Full-ASCII symbologies** (`^BA`, `^B4`, `^BD`, `^BC`, `^BB` modes E/F) omit `characterSet` to avoid false positives. The validator cannot distinguish between standard and extended ASCII modes at parse time.

3. **`maxLength`** is set conservatively from the ZPL guide's explicit limits (e.g., "limited to 3K" → `3072`). Symbologies with only physical limits ("limited to label width") omit `maxLength`.

4. **Check digits** are not validated — they are auto-calculated by the printer and not part of `^FD` data.
