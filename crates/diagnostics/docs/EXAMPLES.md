# Examples

## Emit a diagnostic (recommended: use `codes::` constants)
```rust
use zpl_toolchain_diagnostics::{Diagnostic, codes};

// Preferred: use compile-time constants for typo detection and IDE autocomplete
let d = Diagnostic::error(codes::ARITY, "too many arguments", None);
println!("{}: {}", d.id, d.message);

// With a span:
use zpl_toolchain_diagnostics::Span;
let d = Diagnostic::warn(codes::REDUNDANT_STATE, "redundant state override", Some(Span::new(10, 25)));
```

## Emit a diagnostic (string literals — discouraged)
```rust
use zpl_toolchain_diagnostics::Diagnostic;

// Works, but prefer codes:: constants to catch typos at compile time:
let d = Diagnostic::error("ZPL1101", "too many arguments", None);
```

## Display formatting
```rust
use zpl_toolchain_diagnostics::{Diagnostic, codes};

let d = Diagnostic::error(codes::ARITY, "too many arguments", None);
println!("{}", d);  // "error[ZPL1101]: too many arguments"
```

## Explain a diagnostic
```rust
use zpl_toolchain_diagnostics::{Diagnostic, codes};

// From a Diagnostic instance (convenience method):
let d = Diagnostic::error(codes::ARITY, "too many arguments", None);
if let Some(desc) = d.explain() {
    println!("  help: {}", desc);
}

// Or via the free function:
let desc = zpl_toolchain_diagnostics::explain(codes::PROFILE_CONSTRAINT).unwrap_or("(no explanation)");
println!("{} => {}", codes::PROFILE_CONSTRAINT, desc);
```

