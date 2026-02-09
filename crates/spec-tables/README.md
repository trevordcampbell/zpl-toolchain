# zpl_toolchain_spec_tables

Defines shared data structures for generated tables consumed by parser/validator.

Part of the [zpl-toolchain](https://github.com/trevordcampbell/zpl-toolchain) project.

## Key types
- `ParserTables { schema_version, format_version, commands, opcode_trie }`
- `CommandEntry { codes, arity, field_data, raw_payload, opens_field, closes_field, hex_escape_modifier, field_number, serialization, requires_field, signature, args, constraints, effects, plane, scope, ... }`
- `Signature { params, joiner, allow_empty_trailing }`
- `Arg { name, key, type, unit, range, optional, default, default_from, profile_constraint, range_when, rounding_policy, rounding_policy_when, enum, min_length, max_length }`
- `Constraint { kind: ConstraintKind, expr, message, severity: Option<ConstraintSeverity> }`
- `Effects { sets: Vec<String> }`
- `ProfileConstraint { field, op: ComparisonOp }`

## Enums
- `ConstraintKind`: Order, Requires, Incompatible, EmptyData, Range, Note, Custom
  - `ConstraintKind::ALL` is the **single source of truth** for the set of valid kinds; the JSONC schema mirrors this list and a spec-compiler test validates they stay in sync.
- `ConstraintSeverity`: Error, Warn, Info
- `ComparisonOp`: Lte, Gte, Lt, Gt, Eq
- `RoundingMode`: ToMultiple
- `Plane`: Format, Device, Host, Config
- `CommandScope`: Label, Session, Global
- `CommandCategory`: Format, Control, Status, Config
- `Stability`: Stable, Deprecated, Undocumented

## Additional Types
- `FieldDataRules { charset, length, parity }` — barcode field data validation rules
- `SplitRule { font_len, orientation_len }` — spec-driven `^A` font+orientation splitting
- `Composite { prefix, args }` — composite command expansion (e.g., `^XG`)

## Notes
- `TABLE_FORMAT_VERSION` is currently `0.3.0`.
- Legacy `args_spec` was removed in format `0.3.0`; use `args` (and `ArgUnion`).
- Structural role flags (`opens_field`, `closes_field`, etc.) drive the validator's field-tracking state machine.
- Conditional rules are evaluated by the validator using simple predicates.

