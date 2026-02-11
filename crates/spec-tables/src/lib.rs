//! ZPL command specification tables.
//!
//! Defines the data structures for ZPL command metadata, including command
//! entries, argument schemas, constraints, and an opcode trie for fast command
//! recognition.  These tables are deserialized from the generated JSON spec
//! and consumed by the parser and validator.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

// ─── Custom serde for HashMap<char, V> ──────────────────────────────────────
// JSON object keys are always strings. The opcode trie uses single-character
// keys, so we convert between `String` (JSON) and `char` (Rust) during
// serialization/deserialization to avoid per-lookup heap allocations.

fn deserialize_char_map<'de, D, V>(deserializer: D) -> Result<HashMap<char, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    let string_map = HashMap::<String, V>::deserialize(deserializer)?;
    string_map
        .into_iter()
        .map(|(k, v)| {
            let ch = k
                .chars()
                .next()
                .ok_or_else(|| serde::de::Error::custom("empty key in trie"))?;
            if k.len() != ch.len_utf8() {
                return Err(serde::de::Error::custom(format!(
                    "multi-char key in trie: {:?}",
                    k
                )));
            }
            Ok((ch, v))
        })
        .collect()
}

fn serialize_char_map<S, V: Serialize>(
    map: &HashMap<char, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeMap;
    let mut ser_map = serializer.serialize_map(Some(map.len()))?;
    for (k, v) in map {
        ser_map.serialize_entry(&k.to_string(), v)?;
    }
    ser_map.end()
}

/// Current format version for the spec table JSON schema.
pub const TABLE_FORMAT_VERSION: &str = "0.3.0";

/// Command scope — determines the lifecycle boundary of the command's effect.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommandScope {
    /// Applies to the entire document.
    Document,
    /// Applies within a single field block (`^FO`…`^FS`).
    Field,
    /// Applies to a print job.
    Job,
    /// Persists across labels within a session.
    Session,
    /// Applies within a single label (`^XA`…`^XZ`).
    Label,
}

impl std::fmt::Display for CommandScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandScope::Document => write!(f, "document"),
            CommandScope::Field => write!(f, "field"),
            CommandScope::Job => write!(f, "job"),
            CommandScope::Session => write!(f, "session"),
            CommandScope::Label => write!(f, "label"),
        }
    }
}

/// Functional category for a command.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommandCategory {
    /// Text rendering commands.
    Text,
    /// Barcode generation commands.
    Barcode,
    /// Graphic element commands.
    Graphics,
    /// Media handling and configuration.
    Media,
    /// Label format and layout commands.
    Format,
    /// Device status and control commands.
    Device,
    /// Host communication commands.
    Host,
    /// Printer configuration commands.
    Config,
    /// Network configuration commands.
    Network,
    /// RFID encoding and reading commands.
    Rfid,
    /// Wireless network commands.
    Wireless,
    /// File storage and retrieval commands.
    Storage,
    /// Keyboard Display Unit commands.
    Kdu,
    /// Miscellaneous commands.
    Misc,
}

impl std::fmt::Display for CommandCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandCategory::Text => write!(f, "text"),
            CommandCategory::Barcode => write!(f, "barcode"),
            CommandCategory::Graphics => write!(f, "graphics"),
            CommandCategory::Media => write!(f, "media"),
            CommandCategory::Format => write!(f, "format"),
            CommandCategory::Device => write!(f, "device"),
            CommandCategory::Host => write!(f, "host"),
            CommandCategory::Config => write!(f, "config"),
            CommandCategory::Network => write!(f, "network"),
            CommandCategory::Rfid => write!(f, "rfid"),
            CommandCategory::Wireless => write!(f, "wireless"),
            CommandCategory::Storage => write!(f, "storage"),
            CommandCategory::Kdu => write!(f, "kdu"),
            CommandCategory::Misc => write!(f, "misc"),
        }
    }
}

/// Stability level for a command.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Stability {
    /// Fully supported and stable.
    Stable,
    /// Experimental — may change without notice.
    Experimental,
    /// Deprecated — may be removed in a future firmware version.
    Deprecated,
}

impl std::fmt::Display for Stability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stability::Stable => write!(f, "stable"),
            Stability::Experimental => write!(f, "experimental"),
            Stability::Deprecated => write!(f, "deprecated"),
        }
    }
}

/// Top-level container for all ZPL command spec tables.
///
/// Deserialized from the generated JSON spec and used by the parser and
/// validator for command recognition, argument parsing, and constraint checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserTables {
    /// Spec schema version (e.g., `"1.1.1"`).
    pub schema_version: String,
    /// Table format version for compatibility checks.
    #[serde(default = "default_format_version")]
    pub format_version: String,
    /// All known command entries.
    pub commands: Vec<CommandEntry>,
    /// Optional opcode trie for fast longest-match command recognition.
    #[serde(default)]
    pub opcode_trie: Option<OpcodeTrieNode>,

    /// Cached set of all known command codes (lazily initialized).
    #[serde(skip)]
    code_set_cache: OnceLock<HashSet<String>>,
    /// Cached map from command code → index into `commands` (lazily initialized).
    #[serde(skip)]
    cmd_map: OnceLock<HashMap<String, usize>>,
}

fn default_format_version() -> String {
    TABLE_FORMAT_VERSION.to_string()
}

impl ParserTables {
    /// Create a new `ParserTables` with the given fields.
    /// Cache fields are initialized lazily on first access.
    pub fn new(
        schema_version: String,
        format_version: String,
        commands: Vec<CommandEntry>,
        opcode_trie: Option<OpcodeTrieNode>,
    ) -> Self {
        Self {
            schema_version,
            format_version,
            commands,
            opcode_trie,
            code_set_cache: OnceLock::new(),
            cmd_map: OnceLock::new(),
        }
    }

    /// Returns a cached set of all known command codes.
    /// The set is built lazily on first access and reused thereafter.
    pub fn code_set(&self) -> &HashSet<String> {
        self.code_set_cache.get_or_init(|| {
            self.commands
                .iter()
                .flat_map(|c| c.codes.iter().cloned())
                .collect()
        })
    }

    /// Returns the cached code → index map, building it lazily on first access.
    fn cmd_map(&self) -> &HashMap<String, usize> {
        self.cmd_map.get_or_init(|| {
            let mut m = HashMap::new();
            for (i, c) in self.commands.iter().enumerate() {
                for code in &c.codes {
                    m.insert(code.clone(), i);
                }
            }
            m
        })
    }

    /// Look up a `CommandEntry` by its opcode string (e.g., "^FO", "~DG").
    /// Uses a cached HashMap for O(1) lookup.
    pub fn cmd_by_code(&self, code: &str) -> Option<&CommandEntry> {
        self.cmd_map().get(code).map(|&i| &self.commands[i])
    }
}

/// Metadata for a single ZPL command (or group of aliased commands).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntry {
    /// All opcode strings for this command (e.g., `["^FO"]` or `["^CC", "~CC"]`).
    pub codes: Vec<String>,
    /// Maximum number of arguments this command accepts.
    pub arity: u32,
    /// Whether this command consumes a raw binary/hex payload (e.g., `^GF`, `~DG`).
    #[serde(default)]
    pub raw_payload: bool,
    /// Whether this command collects field data (e.g., `^FD`, `^FV`).
    #[serde(default)]
    pub field_data: bool,
    /// Whether this command opens a field block (e.g., ^FO, ^FT).
    #[serde(default)]
    pub opens_field: bool,
    /// Whether this command closes a field block (e.g., ^FS).
    #[serde(default)]
    pub closes_field: bool,
    /// Whether this command enables hex escape mode in the current field (e.g., ^FH).
    #[serde(default)]
    pub hex_escape_modifier: bool,
    /// Whether this command assigns a field number (e.g., ^FN).
    #[serde(default)]
    pub field_number: bool,
    /// Whether this command is a serialization command within a field (e.g., ^SN, ^SF).
    #[serde(default)]
    pub serialization: bool,
    /// Whether this command requires an open field to be valid (e.g., ^FD, ^FV).
    /// Note: field_data already implies this, but this flag is explicit for non-field-data commands.
    #[serde(default)]
    pub requires_field: bool,
    /// Signature describing parameter names, joiner, and split rules.
    #[serde(default)]
    pub signature: Option<Signature>,
    /// Rich argument definitions with type, range, enum, and constraint metadata.
    #[serde(default)]
    pub args: Option<Vec<ArgUnion>>,
    /// Command-level validation constraints (order, requires, incompatible, etc.).
    #[serde(default)]
    pub constraints: Option<Vec<Constraint>>,
    /// Cross-command state effects: which state keys this command sets.
    #[serde(default)]
    pub effects: Option<Effects>,
    /// Plane: format, config, host, device.
    #[serde(default)]
    pub plane: Option<Plane>,
    /// Scope: field, label, session, document, job.
    #[serde(default)]
    pub scope: Option<CommandScope>,
    /// Placement rules that refine where commands are allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,

    // ── Metadata & versioning ───────────────────────────────────────────
    /// Human-readable command name (e.g., "Print Width").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Functional category (e.g., media, barcode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CommandCategory>,
    /// Firmware version the command was introduced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Whether this command is deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    /// Firmware version the command was deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_since: Option<String>,
    /// Stability level (e.g., stable, experimental).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,

    // ── Composite / free-form data ──────────────────────────────────────
    /// Composite argument groups (typed since v0.3.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composites: Option<Vec<Composite>>,
    /// Default value overrides (freeform bag).
    /// Stays as `serde_json::Value` because the schema defines no specific
    /// properties (`additionalProperties: true`) and no code inspects its contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<serde_json::Value>,
    /// Unit string for all arguments (e.g., "dots").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    /// Printer gate requirements (e.g., ["ezpl", "zbi"]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer_gates: Option<Vec<String>>,
    /// Per-opcode signature overrides, keyed by opcode (e.g., "^CC").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_overrides: Option<HashMap<String, Signature>>,
    /// Validation rules for field data when this barcode command is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_data_rules: Option<FieldDataRules>,
    /// Executable/documentation examples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<Example>>,
}

/// Composite parameter group — combines multiple args into a single path-like
/// parameter in the signature (e.g., `d:o.x` for `^XG`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Composite {
    /// Name used inside `signature.params` (e.g., `"d:o.x"`).
    pub name: String,
    /// Template referencing underlying arg keys (e.g., `"{d}:{o}.{x}"`).
    pub template: String,
    /// Which arg keys this composite groups together.
    pub exposes_args: Vec<String>,
    /// Optional documentation for this composite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

/// Describes which cross-command state a command sets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
    /// State keys this command sets (e.g., ["barcode.moduleWidth", "barcode.ratio", "barcode.height"]).
    #[serde(default)]
    pub sets: Vec<String>,
}

/// Placement rules that refine command location semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// Whether this command is allowed inside ^XA/^XZ label bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_inside_label: Option<bool>,
    /// Whether this command is allowed outside ^XA/^XZ label bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_outside_label: Option<bool>,
}

/// Validation rules for field data content associated with a barcode command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDataRules {
    /// Regex character class for allowed characters (e.g., "0-9", "A-Z0-9").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_set: Option<String>,
    /// Minimum data length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    /// Maximum data length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    /// Shorthand for min_length == max_length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_length: Option<usize>,
    /// Discrete allowed lengths (e.g., [2, 5]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_lengths: Option<Vec<usize>>,
    /// Required parity of data length ("even" or "odd").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_parity: Option<String>,
    /// Human-readable notes about the data format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Rule for splitting a parameter into multiple parts by character count.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitRule {
    /// Which param to split (0-based index).
    pub param_index: usize,
    /// Character counts per split part.
    pub char_counts: Vec<usize>,
}

/// Describes the parameter signature of a ZPL command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    /// Ordered list of parameter names (keys).
    pub params: Vec<String>,
    /// Character(s) that separate arguments (default `","`).
    #[serde(default = "default_joiner")]
    pub joiner: String,
    /// Whether an opcode is immediately followed by parameters with no space.
    #[serde(default = "default_no_space_after_opcode")]
    pub no_space_after_opcode: bool,
    /// Whether to pad the argument list with empty trailing slots.
    #[serde(default = "default_allow_empty_trailing")]
    pub allow_empty_trailing: bool,
    /// Optional rule for splitting a single raw parameter into multiple args.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_rule: Option<SplitRule>,
}

fn default_joiner() -> String {
    ",".to_string()
}

fn default_no_space_after_opcode() -> bool {
    true
}

fn default_allow_empty_trailing() -> bool {
    true
}

/// A node in the opcode trie used for longest-match command recognition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpcodeTrieNode {
    /// Child nodes keyed by the next character in the opcode.
    #[serde(
        default,
        deserialize_with = "deserialize_char_map",
        serialize_with = "serialize_char_map"
    )]
    pub children: HashMap<char, OpcodeTrieNode>,
    /// Whether this node represents a complete, valid opcode.
    #[serde(default)]
    pub terminal: bool,
}

/// A command argument definition — either a single [`Arg`] or a `oneOf` union.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgUnion {
    /// A single argument definition.
    Single(Box<Arg>),
    /// A union of possible argument definitions (polymorphic parameter).
    OneOf {
        /// The set of alternative argument definitions.
        #[serde(rename = "oneOf")]
        one_of: Vec<Arg>,
    },
}

/// Declares how firmware interprets argument presence/emptiness.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArgPresence {
    /// Argument is absent.
    #[serde(rename = "unset")]
    Unset,
    /// Argument is present but explicitly empty.
    #[serde(rename = "empty")]
    Empty,
    /// Argument is present with a concrete value.
    #[serde(rename = "value")]
    Value,
    /// Argument may be explicit value or firmware default.
    #[serde(rename = "valueOrDefault")]
    ValueOrDefault,
    /// Empty argument means "use default".
    #[serde(rename = "emptyMeansUseDefault")]
    EmptyMeansUseDefault,
}

/// Resource family for `resourceRef` arguments.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    /// Graphic resource.
    Graphic,
    /// Font resource.
    Font,
    /// Any resource family.
    Any,
}

/// Rich metadata for a single command argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Arg {
    /// Human-readable argument name.
    #[serde(default)]
    pub name: Option<String>,
    /// Short key used in signatures and lookups.
    #[serde(default)]
    pub key: Option<String>,
    /// Value type: `"int"`, `"float"`, `"enum"`, `"string"`, `"char"`, etc.
    #[serde(rename = "type")]
    pub r#type: String,

    /// Unit of measurement (e.g., `"dots"`, `"in"`).
    #[serde(default)]
    pub unit: Option<String>,
    /// Allowed numeric range `[min, max]`.
    #[serde(default)]
    pub range: Option<[f64; 2]>,
    /// Minimum string length.
    #[serde(default)]
    pub min_length: Option<u32>,
    /// Maximum string length.
    #[serde(default)]
    pub max_length: Option<u32>,

    /// Whether this argument may be omitted.
    #[serde(default)]
    pub optional: bool,
    /// Clarifies how empty vs missing args are interpreted by firmware.
    #[serde(default)]
    pub presence: Option<ArgPresence>,
    /// Static default value.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// DPI-dependent default values. Keys are DPI strings (e.g., `"203"`, `"300"`),
    /// values are the default at that DPI. Falls back to `default` when no match.
    #[serde(default)]
    pub default_by_dpi: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Command that provides this arg's default (e.g., `"^CF"`, `"^BY"`).
    #[serde(default)]
    pub default_from: Option<String>,

    /// Profile-driven constraint on this argument's value.
    #[serde(default)]
    pub profile_constraint: Option<ProfileConstraint>,

    /// Conditional range overrides based on other argument values.
    #[serde(default)]
    pub range_when: Option<Vec<ConditionalRange>>,
    /// Rounding policy for numeric values.
    #[serde(default)]
    pub rounding_policy: Option<RoundingPolicy>,
    /// Conditional rounding policy overrides.
    #[serde(default)]
    pub rounding_policy_when: Option<Vec<ConditionalRounding>>,
    /// Resource family when `type == "resourceRef"`.
    #[serde(default)]
    pub resource: Option<ResourceKind>,

    /// Allowed enum values (simple strings or rich objects with gates).
    #[serde(default)]
    pub r#enum: Option<Vec<EnumValue>>,
}

/// Comparison operators for profile constraints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonOp {
    /// Less than or equal.
    Lte,
    /// Greater than or equal.
    Gte,
    /// Strictly less than.
    Lt,
    /// Strictly greater than.
    Gt,
    /// Equal (with tolerance for integer-cast floats).
    Eq,
}

/// Data-driven profile constraint on an arg value.
/// Replaces hardcoded checks like "^PW <= profile.page.width_dots".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConstraint {
    /// Dotted path into the Profile struct (e.g., "page.width_dots")
    pub field: String,
    /// Comparison operator
    pub op: ComparisonOp,
}

/// An allowed enum value — either a plain string or a rich object with gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnumValue {
    /// A plain string value.
    Simple(String),
    /// A value with optional printer gate requirements and extra metadata.
    Object {
        /// The enum value string.
        value: String,
        /// Printer capabilities required for this value.
        #[serde(default, rename = "printerGates")]
        printer_gates: Option<Vec<String>>,
        /// Additional freeform metadata.
        #[serde(default)]
        extras: Option<serde_json::Value>,
    },
}

/// A numeric range that applies only when a predicate is satisfied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalRange {
    /// Predicate expression (e.g., `"arg:modeIsValue:T"`).
    pub when: String,
    /// The `[min, max]` range to enforce when the predicate matches.
    pub range: [f64; 2],
}

/// Rounding modes for numeric arguments.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RoundingMode {
    /// Value must be a multiple of a given base.
    ToMultiple,
}

/// Policy for rounding or quantizing a numeric argument value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundingPolicy {
    /// Unit of measurement for rounding context.
    #[serde(default)]
    pub unit: Option<String>,
    /// Rounding mode to apply.
    pub mode: RoundingMode,
    /// The multiple to which the value should be rounded.
    #[serde(default)]
    pub multiple: Option<f64>,
}

/// A rounding policy that applies only when a predicate is satisfied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalRounding {
    /// Predicate expression for when this rounding applies.
    pub when: String,
    /// Rounding mode to apply.
    pub mode: RoundingMode,
    /// The multiple to which the value should be rounded.
    #[serde(default)]
    pub multiple: Option<f64>,
}

/// Constraint kinds for command-level constraints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintKind {
    /// Command ordering constraint (before/after another command).
    Order,
    /// Requires another command to be present in the label.
    Requires,
    /// Incompatible with another command in the same label.
    Incompatible,
    /// Requires non-empty field data content.
    EmptyData,
    /// Numeric range constraint (future extension).
    Range,
    /// Informational note emitted when the command is used.
    Note,
    /// Custom constraint with freeform logic.
    Custom,
}

impl ConstraintKind {
    /// All constraint kind variants.
    ///
    /// This is the **single source of truth** for the set of valid constraint kinds.
    /// The JSONC schema at `spec/schema/zpl-spec.schema.jsonc` must mirror this list;
    /// a spec-compiler test validates they stay in sync.
    pub const ALL: &[Self] = &[
        Self::Order,
        Self::Requires,
        Self::Incompatible,
        Self::EmptyData,
        Self::Range,
        Self::Note,
        Self::Custom,
    ];
}

impl std::fmt::Display for ConstraintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintKind::Order => write!(f, "order"),
            ConstraintKind::Requires => write!(f, "requires"),
            ConstraintKind::Incompatible => write!(f, "incompatible"),
            ConstraintKind::EmptyData => write!(f, "emptyData"),
            ConstraintKind::Range => write!(f, "range"),
            ConstraintKind::Note => write!(f, "note"),
            ConstraintKind::Custom => write!(f, "custom"),
        }
    }
}

/// Severity level for constraint diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConstraintSeverity {
    /// Hard error — the label is invalid.
    Error,
    /// Warning — the label may produce unexpected results.
    Warn,
    /// Informational note.
    Info,
}

/// Evaluation scope for command-level constraints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConstraintScope {
    /// Evaluate the constraint across the full label.
    Label,
    /// Evaluate the constraint within the current field only.
    Field,
}

/// Command plane — determines where in the ZPL hierarchy the command operates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Plane {
    /// Format plane — commands inside label blocks (`^XA`…`^XZ`).
    Format,
    /// Device plane — device-level configuration commands.
    Device,
    /// Host plane — host communication commands.
    Host,
    /// Config plane — persistent configuration commands.
    Config,
}

impl std::fmt::Display for Plane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Plane::Format => write!(f, "format"),
            Plane::Device => write!(f, "device"),
            Plane::Host => write!(f, "host"),
            Plane::Config => write!(f, "config"),
        }
    }
}

/// An executable/documentation example for a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Example {
    /// ZPL code for this example (required).
    pub zpl: String,
    /// Title of the example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional BLAKE3 hash of the rendered PNG for golden-file testing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png_hash: Option<String>,
    /// Explanatory notes for this example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Firmware version this example applies from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Printer profiles this example is relevant to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<String>>,
}

/// A command-level validation constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Constraint {
    /// The type of constraint.
    pub kind: ConstraintKind,
    /// Expression for the constraint (e.g., `"before:^XZ"` or `"^PW"`).
    #[serde(default)]
    pub expr: Option<String>,
    /// Human-readable diagnostic message when the constraint is violated.
    pub message: String,
    /// Severity override for the diagnostic (defaults to warn).
    #[serde(default)]
    pub severity: Option<ConstraintSeverity>,
    /// Optional evaluation scope for this constraint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ConstraintScope>,
}

#[cfg(test)]
mod tests {
    use super::{Arg, ArgPresence, ResourceKind, Signature};

    #[test]
    fn signature_allow_empty_trailing_defaults_true() {
        let sig: Signature =
            serde_json::from_str(r#"{"params":["a"],"joiner":","}"#).expect("valid signature");
        assert!(
            sig.allow_empty_trailing,
            "allow_empty_trailing should default to true to match schema"
        );
    }

    #[test]
    fn arg_presence_and_resource_deserialize() {
        let arg: Arg = serde_json::from_str(
            r#"{
                "name":"obj",
                "type":"resourceRef",
                "presence":"valueOrDefault",
                "resource":"font"
            }"#,
        )
        .expect("valid arg");
        assert_eq!(arg.presence, Some(ArgPresence::ValueOrDefault));
        assert_eq!(arg.resource, Some(ResourceKind::Font));
    }
}
