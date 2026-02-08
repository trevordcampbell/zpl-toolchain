# Examples

## Load parser tables
```rust
let s = std::fs::read_to_string("generated/parser_tables.json")?;
let tables: zpl_toolchain_spec_tables::ParserTables = serde_json::from_str(&s)?;
assert!(tables.commands.len() > 0);
assert_eq!(tables.format_version, zpl_toolchain_spec_tables::TABLE_FORMAT_VERSION);
```

## Inspect a command entry
```rust
use zpl_toolchain_spec_tables::ArgUnion;

let bc = tables.commands.iter().find(|c| c.codes.contains(&"^BC".to_string())).unwrap();
if let Some(args) = &bc.args {
    for a in args {
        match a {
            ArgUnion::Single(arg) => println!("{} {:?}", arg.key.clone().unwrap_or_default(), arg.range),
            ArgUnion::OneOf { one_of } => println!("oneOf with {} variants", one_of.len()),
        }
    }
}
```

## Opcode trie
```rust
if let Some(trie) = &tables.opcode_trie { assert!(trie.children.contains_key(&'^')); }
```

