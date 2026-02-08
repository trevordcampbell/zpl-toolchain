# Examples

## Load a profile
```rust
use zpl_toolchain_profile::load_profile_from_str;

let s = std::fs::read_to_string("profiles/zebra-generic-203.json")?;
let profile = load_profile_from_str(&s)?;
assert_eq!(profile.dpi, 203);
if let Some(page) = &profile.page { println!("width_dots={:?} height_dots={:?}", page.width_dots, page.height_dots); }
```

