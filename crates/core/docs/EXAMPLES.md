# Examples

## Parse and validate with tables
```rust
use zpl_toolchain_core::{parse_with_tables, validate_with_profile, ParserTables};
use zpl_toolchain_profile::load_profile_from_str;

let tables: ParserTables = serde_json::from_str(&std::fs::read_to_string("generated/parser_tables.json")?)?;
let input = std::fs::read_to_string("samples/usps_surepost_sample.zpl")?;
let res = parse_with_tables(&input, Some(&tables));
let vr = validate_with_profile(&res.ast, &tables, None);
assert!(vr.ok);
// With profile (enforces ^PW width and ^LL height):
let profile = load_profile_from_str(&std::fs::read_to_string("profiles/zebra-generic-203.json")?)?;
let vrp = validate_with_profile(&res.ast, &tables, Some(&profile));
println!("issues with profile: {}", vrp.issues.len());
```

## Merge parser diagnostics into lint surface
```rust
use zpl_toolchain_core::{parse_with_tables, validate_with_profile};

let res = parse_with_tables(&input, Some(&tables));
let mut vr = validate_with_profile(&res.ast, &tables, None);
vr.issues.extend(res.diagnostics);
println!("{} issues", vr.issues.len());
```

