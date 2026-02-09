using System.Text.Json;
using System.Text.Json.Serialization;

namespace ZplToolchain;

/// <summary>Byte range in source input.</summary>
public record Span(
    [property: JsonPropertyName("start")] int Start,
    [property: JsonPropertyName("end")] int End
);

/// <summary>A parsed argument slot.</summary>
public record ArgSlot(
    [property: JsonPropertyName("key")] string? Key = null,
    [property: JsonPropertyName("presence")] string Presence = "unset",
    [property: JsonPropertyName("value")] string? Value = null
);

/// <summary>A diagnostic message from the parser or validator.</summary>
public record Diagnostic(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("severity")] string Severity,
    [property: JsonPropertyName("message")] string Message,
    [property: JsonPropertyName("span")] Span? Span = null,
    [property: JsonPropertyName("context")] Dictionary<string, string>? Context = null
);

/// <summary>Result of parsing a ZPL string.</summary>
public record ParseResult(
    [property: JsonPropertyName("ast")] Ast Ast,
    [property: JsonPropertyName("diagnostics")] List<Diagnostic> Diagnostics
);

/// <summary>Result of validating a ZPL string.</summary>
public record ValidationResult(
    [property: JsonPropertyName("ok")] bool Ok,
    [property: JsonPropertyName("issues")] List<Diagnostic> Issues
);

/// <summary>Result of sending ZPL to a printer.</summary>
public record PrintResult(
    [property: JsonPropertyName("success")] bool Success,
    [property: JsonPropertyName("bytes_sent")] int BytesSent,
    [property: JsonPropertyName("error")] string? Error = null,
    [property: JsonPropertyName("issues")] List<Diagnostic>? Issues = null
);

/// <summary>Top-level AST for a ZPL document.</summary>
public record Ast(
    [property: JsonPropertyName("labels")] List<Label> Labels
);

/// <summary>A single ZPL label (^XA ... ^XZ block).</summary>
public record Label(
    [property: JsonPropertyName("nodes")] List<Node> Nodes
);

/// <summary>
/// AST node — discriminated by the <c>kind</c> field.
/// Rust serializes <c>Node</c> with <c>#[serde(tag = "kind")]</c> (internally tagged),
/// producing JSON like <c>{"kind": "Command", "code": "^XA", ...}</c>.
/// Since C# doesn't have native tagged unions, all variant fields are
/// nullable and the <c>Kind</c> property indicates which fields are populated.
/// </summary>
[JsonConverter(typeof(NodeJsonConverter))]
public record Node
{
    /// <summary>Discriminator: "Command", "FieldData", "RawData", or "Trivia".</summary>
    [JsonPropertyName("kind")]
    public string Kind { get; init; } = "";

    // ── Command fields ──
    [JsonPropertyName("code")]
    public string? Code { get; init; }

    [JsonPropertyName("args")]
    public List<ArgSlot>? Args { get; init; }

    // ── FieldData fields ──
    [JsonPropertyName("content")]
    public string? Content { get; init; }

    [JsonPropertyName("hex_escaped")]
    public bool? HexEscaped { get; init; }

    // ── RawData fields ──
    /// <summary>For RawData nodes, the command that initiated raw data collection.</summary>
    [JsonPropertyName("command")]
    public string? Command { get; init; }

    [JsonPropertyName("data")]
    public string? Data { get; init; }

    // ── Trivia fields ──
    [JsonPropertyName("text")]
    public string? Text { get; init; }

    // ── Shared fields ──
    /// <summary>Source span of this node. Always present in Rust AST output.</summary>
    [JsonPropertyName("span")]
    public Span Span { get; init; } = new(0, 0);
}

/// <summary>
/// Custom JSON converter for <see cref="Node"/> that handles the internally-tagged
/// enum serialization from Rust's serde.
/// </summary>
internal class NodeJsonConverter : JsonConverter<Node>
{
    public override Node? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        var raw = root.GetRawText();

        // Use a simple options instance without the converter to avoid recursion.
        var plainOptions = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true
        };

        // Deserialize all fields; only the relevant ones for the Kind will be non-null.
        var node = JsonSerializer.Deserialize<NodeDto>(raw, plainOptions);
        if (node == null) return null;

        return new Node
        {
            Kind = node.Kind ?? "",
            Code = node.Code,
            Args = node.Args,
            Content = node.Content,
            HexEscaped = node.HexEscaped,
            Command = node.Command,
            Data = node.Data,
            Text = node.Text,
            Span = node.Span,
        };
    }

    public override void Write(Utf8JsonWriter writer, Node value, JsonSerializerOptions options)
    {
        // Serialize via NodeDto to avoid infinite recursion — Node has [JsonConverter]
        // on the type itself, so serializing Node directly would re-enter this converter.
        var dto = new NodeDto
        {
            Kind = value.Kind,
            Code = value.Code,
            Args = value.Args,
            Content = value.Content,
            HexEscaped = value.HexEscaped,
            Command = value.Command,
            Data = value.Data,
            Text = value.Text,
            Span = value.Span,
        };
        JsonSerializer.Serialize(writer, dto);
    }

    /// <summary>Internal DTO to avoid converter recursion.</summary>
    private record NodeDto
    {
        [JsonPropertyName("kind")] public string? Kind { get; init; }
        [JsonPropertyName("code")] public string? Code { get; init; }
        [JsonPropertyName("args")] public List<ArgSlot>? Args { get; init; }
        [JsonPropertyName("content")] public string? Content { get; init; }
        [JsonPropertyName("hex_escaped")] public bool? HexEscaped { get; init; }
        [JsonPropertyName("command")] public string? Command { get; init; }
        [JsonPropertyName("data")] public string? Data { get; init; }
        [JsonPropertyName("text")] public string? Text { get; init; }
        [JsonPropertyName("span")] public Span Span { get; init; } = new(0, 0);
    }
}
