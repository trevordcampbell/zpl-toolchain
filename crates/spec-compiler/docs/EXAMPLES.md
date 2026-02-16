# Examples

## Build tables from spec
```bash
zpl-spec-compiler build --spec-dir spec --out-dir generated
```

## Validate spec only
```bash
zpl-spec-compiler check --spec-dir spec
```

## Programmatic use (Rust)
```rust
// Read merged parser tables
let json = std::fs::read_to_string("generated/parser_tables.json")?;
let tables: zpl_toolchain_spec_tables::ParserTables = serde_json::from_str(&json)?;
println!("{} commands", tables.commands.len());
```

## Artifacts

- `parser_tables.json` (format_version 0.4.0; includes opcode trie inline)
- `docs_bundle.json` (by_code with anchors, formatTemplate, enumValues, composites.exposesArgs, missingFields — not consumed at runtime; for external tooling)
- `constraints_bundle.json` (per-code constraints — not consumed at runtime; for external tooling)
- `coverage.json` (present/missing counts, per_code missing fields/union positions, validation_errors)

