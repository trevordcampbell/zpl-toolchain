using System;
using System.Collections.Generic;
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

/// <summary>Typed defaults from ^BY.</summary>
public record BarcodeDefaults(
    [property: JsonPropertyName("module_width")] uint? ModuleWidth = null,
    [property: JsonPropertyName("ratio")] double? Ratio = null,
    [property: JsonPropertyName("height")] uint? Height = null
);

/// <summary>Typed defaults from ^CF.</summary>
public record FontDefaults(
    [property: JsonPropertyName("font")] string? Font = null,
    [property: JsonPropertyName("height")] uint? Height = null,
    [property: JsonPropertyName("width")] uint? Width = null
);

/// <summary>Typed defaults from ^FW.</summary>
public record FieldOrientationDefaults(
    [property: JsonPropertyName("orientation")] string? Orientation = null,
    [property: JsonPropertyName("justification")] byte? Justification = null
);

/// <summary>Typed label-home defaults from ^LH.</summary>
public record LabelHome(
    [property: JsonPropertyName("x")] double X,
    [property: JsonPropertyName("y")] double Y
);

/// <summary>Typed layout defaults.</summary>
public record LayoutDefaults(
    [property: JsonPropertyName("print_width")] double? PrintWidth = null,
    [property: JsonPropertyName("label_length")] double? LabelLength = null,
    [property: JsonPropertyName("print_orientation")] string? PrintOrientation = null,
    [property: JsonPropertyName("mirror_image")] string? MirrorImage = null,
    [property: JsonPropertyName("reverse_print")] string? ReversePrint = null,
    [property: JsonPropertyName("label_top")] double? LabelTop = null,
    [property: JsonPropertyName("label_shift")] double? LabelShift = null
);

/// <summary>Typed per-label state snapshot.</summary>
public record LabelValueState(
    [property: JsonPropertyName("barcode")] BarcodeDefaults Barcode,
    [property: JsonPropertyName("font")] FontDefaults Font,
    [property: JsonPropertyName("field")] FieldOrientationDefaults Field,
    [property: JsonPropertyName("label_home")] LabelHome LabelHome,
    [property: JsonPropertyName("layout")] LayoutDefaults Layout
);

/// <summary>Renderer-ready per-label resolved state.</summary>
public record ResolvedLabelState(
    [property: JsonPropertyName("values")] LabelValueState Values,
    [property: JsonPropertyName("effective_width")] double? EffectiveWidth = null,
    [property: JsonPropertyName("effective_height")] double? EffectiveHeight = null
);

/// <summary>Result of validating a ZPL string.</summary>
public record ValidationResult(
    [property: JsonPropertyName("ok")] bool Ok,
    [property: JsonPropertyName("issues")] List<Diagnostic> Issues,
    [property: JsonPropertyName("resolved_labels")] List<ResolvedLabelState>? ResolvedLabels = null
);

/// <summary>Result of sending ZPL to a printer.</summary>
public record PrintResult(
    [property: JsonPropertyName("success")] bool Success,
    [property: JsonPropertyName("bytes_sent")] int BytesSent,
    [property: JsonPropertyName("error")] string? Error = null,
    [property: JsonPropertyName("issues")] List<Diagnostic>? Issues = null
);

/// <summary>Typed parsed response from printer host status (~HS).</summary>
public record HostStatus(
    [property: JsonPropertyName("communication_flag")] uint CommunicationFlag,
    [property: JsonPropertyName("paper_out")] bool PaperOut,
    [property: JsonPropertyName("paused")] bool Paused,
    [property: JsonPropertyName("label_length_dots")] uint LabelLengthDots,
    [property: JsonPropertyName("formats_in_buffer")] uint FormatsInBuffer,
    [property: JsonPropertyName("buffer_full")] bool BufferFull,
    [property: JsonPropertyName("comm_diag_mode")] bool CommDiagMode,
    [property: JsonPropertyName("partial_format")] bool PartialFormat,
    [property: JsonPropertyName("reserved_1")] uint Reserved1,
    [property: JsonPropertyName("corrupt_ram")] bool CorruptRam,
    [property: JsonPropertyName("under_temperature")] bool UnderTemperature,
    [property: JsonPropertyName("over_temperature")] bool OverTemperature,
    [property: JsonPropertyName("function_settings")] uint FunctionSettings,
    [property: JsonPropertyName("head_up")] bool HeadUp,
    [property: JsonPropertyName("ribbon_out")] bool RibbonOut,
    [property: JsonPropertyName("thermal_transfer_mode")] bool ThermalTransferMode,
    [property: JsonPropertyName("print_mode")] string PrintMode,
    [property: JsonPropertyName("print_width_mode")] uint PrintWidthMode,
    [property: JsonPropertyName("label_waiting")] bool LabelWaiting,
    [property: JsonPropertyName("labels_remaining")] uint LabelsRemaining,
    [property: JsonPropertyName("format_while_printing")] uint FormatWhilePrinting,
    [property: JsonPropertyName("graphics_stored_in_memory")] uint GraphicsStoredInMemory,
    [property: JsonPropertyName("password")] uint Password,
    [property: JsonPropertyName("static_ram_installed")] bool StaticRamInstalled
);

/// <summary>Typed parsed response from printer identification (~HI).</summary>
public record PrinterInfo(
    [property: JsonPropertyName("model")] string Model,
    [property: JsonPropertyName("firmware")] string Firmware,
    [property: JsonPropertyName("dpi")] uint Dpi,
    [property: JsonPropertyName("memory_kb")] uint MemoryKb
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
