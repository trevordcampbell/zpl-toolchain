cli
===

`zpl` command-line interface for parsing, syntax-checking, linting, and formatting ZPL.

Commands
--------
```bash
zpl parse <file.zpl>
zpl syntax-check <file.zpl>
zpl lint <file.zpl> [--profile profiles/zebra-generic-203.json]
zpl format <file.zpl> [--write] [--check] [--indent none|label|field]
zpl coverage --coverage generated/coverage.json [--show-issues | --json]
zpl explain <DIAGNOSTIC_ID>
```

Options
-------
- `--tables`: path to `parser_tables.json` (optional — tables are embedded at compile time by default).
- `--profile`: optional printer profile JSON for contextual checks (e.g., `^PW`, `^LL`).
- `--output pretty|json`: force output mode (default: auto-detect TTY).

Behavior notes
--------------
- `syntax-check`: Returns `ok: true` in JSON output unless there are **Error**-severity diagnostics. Warnings and info do not affect the `ok` flag.
- `--tables`: When an explicit path is provided and the file cannot be read or parsed, the CLI reports the error and exits with code 1 (it does not silently fall back to embedded tables).

Format command
--------------
- Default: prints formatted ZPL to stdout.
- `--write` (`-w`): overwrites the file in-place (only if content changed).
- `--check`: exits with code 1 if the file is not already formatted (for CI).
- `--indent`: indentation style — `none` (flat, default), `label` (2-space inside `^XA`/`^XZ`), or `field` (label + 2-space inside `^FO`…`^FS`).
