using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace ZplToolchain;

/// <summary>
/// Static class providing ZPL toolchain operations via the native C FFI library.
/// 
/// All methods call into the native <c>zpl_toolchain_ffi</c> shared library
/// and deserialize JSON results into .NET types.
/// </summary>
public static class Zpl
{
    private const string LibName = "zpl_toolchain_ffi";
    // netstandard2.0 can compile against frameworks where the enum member isn't available,
    // but the runtime value for UTF-8 strings is stable.
    private const UnmanagedType Utf8String = (UnmanagedType)48;

    // ── P/Invoke declarations ───────────────────────────────────────────

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_parse(
        [MarshalAs(Utf8String)] string input);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_parse_with_tables(
        [MarshalAs(Utf8String)] string input,
        [MarshalAs(Utf8String)] string tablesJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_validate(
        [MarshalAs(Utf8String)] string input,
        [MarshalAs(Utf8String)] string? profileJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_validate_with_tables(
        [MarshalAs(Utf8String)] string input,
        [MarshalAs(Utf8String)] string tablesJson,
        [MarshalAs(Utf8String)] string? profileJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_format(
        [MarshalAs(Utf8String)] string input,
        [MarshalAs(Utf8String)] string? indent);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_format_with_options_v2(
        [MarshalAs(Utf8String)] string input,
        [MarshalAs(Utf8String)] string? indent,
        [MarshalAs(Utf8String)] string? compaction);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_explain(
        [MarshalAs(Utf8String)] string id);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_print_with_options(
        [MarshalAs(Utf8String)] string zpl,
        [MarshalAs(Utf8String)] string printerAddr,
        [MarshalAs(Utf8String)] string? profileJson,
        [MarshalAs(UnmanagedType.I1)] bool validate,
        ulong timeoutMs,
        [MarshalAs(Utf8String)] string? configJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_query_status_with_options(
        [MarshalAs(Utf8String)] string printerAddr,
        ulong timeoutMs,
        [MarshalAs(Utf8String)] string? configJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_query_info_with_options(
        [MarshalAs(Utf8String)] string printerAddr,
        ulong timeoutMs,
        [MarshalAs(Utf8String)] string? configJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void zpl_free(IntPtr ptr);

    // ── Helpers ─────────────────────────────────────────────────────────

    private static string? ConsumePtr(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        try
        {
            var len = 0;
            while (Marshal.ReadByte(ptr, len) != 0)
            {
                len++;
            }
            var bytes = new byte[len];
            Marshal.Copy(ptr, bytes, 0, len);
            return Encoding.UTF8.GetString(bytes);
        }
        finally
        {
            zpl_free(ptr);
        }
    }

    private static T Deserialize<T>(string json)
    {
        return JsonSerializer.Deserialize<T>(json)
            ?? throw new InvalidOperationException("Deserialization returned null");
    }

    /// <summary>
    /// Check if the FFI returned an error response ({"error": "..."}) and throw if so.
    /// </summary>
    private static void CheckForError(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.ValueKind != JsonValueKind.Object) return;
            if (!doc.RootElement.TryGetProperty("error", out var errorElement)) return;
            var message = errorElement.GetString() ?? "Unknown error from native library";
            if (doc.RootElement.TryGetProperty("message", out var detailElement))
            {
                var detail = detailElement.GetString();
                if (!string.IsNullOrWhiteSpace(detail))
                {
                    message = $"{message}: {detail}";
                }
            }
            throw new InvalidOperationException(message);
        }
        catch (JsonException)
        {
            // Non-JSON payloads (e.g. formatted ZPL or explanation text) are valid.
        }
    }

    // ── Public API ──────────────────────────────────────────────────────

    /// <summary>
    /// Parse a ZPL string and return the AST with diagnostics.
    /// </summary>
    public static ParseResult Parse(string input)
    {
        var json = ConsumePtr(zpl_parse(input))
            ?? throw new InvalidOperationException("zpl_parse returned NULL");
        CheckForError(json);
        return Deserialize<ParseResult>(json);
    }

    /// <summary>
    /// Parse a ZPL string with explicitly provided parser tables (JSON string).
    /// </summary>
    public static ParseResult ParseWithTables(string input, string tablesJson)
    {
        var json = ConsumePtr(zpl_parse_with_tables(input, tablesJson))
            ?? throw new InvalidOperationException("zpl_parse_with_tables returned NULL");
        CheckForError(json);
        return Deserialize<ParseResult>(json);
    }

    /// <summary>
    /// Parse and validate a ZPL string.
    /// </summary>
    /// <param name="input">ZPL source code.</param>
    /// <param name="profileJson">Optional printer profile JSON string.</param>
    public static ValidationResult Validate(string input, string? profileJson = null)
    {
        if (string.IsNullOrEmpty(profileJson)) profileJson = null;
        var json = ConsumePtr(zpl_validate(input, profileJson))
            ?? throw new InvalidOperationException("zpl_validate returned NULL");
        CheckForError(json);
        return Deserialize<ValidationResult>(json);
    }

    /// <summary>
    /// Parse and validate a ZPL string with explicitly provided parser tables (JSON string).
    /// </summary>
    /// <param name="input">ZPL source code.</param>
    /// <param name="tablesJson">Parser tables JSON.</param>
    /// <param name="profileJson">Optional printer profile JSON string.</param>
    public static ValidationResult ValidateWithTables(string input, string tablesJson, string? profileJson = null)
    {
        if (string.IsNullOrEmpty(profileJson)) profileJson = null;
        var json = ConsumePtr(zpl_validate_with_tables(input, tablesJson, profileJson))
            ?? throw new InvalidOperationException("zpl_validate_with_tables returned NULL");
        CheckForError(json);
        return Deserialize<ValidationResult>(json);
    }

    /// <summary>
    /// Format a ZPL string with the specified indentation style.
    /// </summary>
    /// <param name="input">ZPL source code.</param>
    /// <param name="indent">"none", "label", or "field" (null for default).</param>
    public static string Format(string input, string? indent = null)
    {
        return FormatWithOptions(input, indent, null);
    }

    /// <summary>
    /// Format a ZPL string with indentation and compaction options.
    /// </summary>
    /// <param name="input">ZPL source code.</param>
    /// <param name="indent">"none", "label", or "field" (null for default).</param>
    /// <param name="compaction">"none" or "field" (null for default).</param>
    public static string FormatWithOptions(
        string input,
        string? indent = null,
        string? compaction = null)
    {
        if (string.IsNullOrEmpty(indent)) indent = null;
        if (string.IsNullOrEmpty(compaction)) compaction = null;
        var formatted = ConsumePtr(zpl_format_with_options_v2(input, indent, compaction))
            ?? throw new InvalidOperationException("zpl_format_with_options_v2 returned NULL");
        CheckForError(formatted);
        return formatted;
    }

    /// <summary>
    /// Explain a diagnostic code (e.g., "ZPL1201").
    /// </summary>
    /// <returns>The explanation, or null if the code is unknown.</returns>
    public static string? Explain(string id)
    {
        var explanation = ConsumePtr(zpl_explain(id));
        if (explanation is null) return null;
        try
        {
            CheckForError(explanation);
        }
        catch (InvalidOperationException)
        {
            // Keep Explain stable and nullable-only for unknown/invalid cases,
            // matching behavior in other wrappers.
            return null;
        }
        return explanation;
    }

    /// <summary>
    /// Send ZPL to a network printer via TCP (port 9100).
    /// </summary>
    /// <param name="zpl">ZPL source code to print.</param>
    /// <param name="printerAddr">Printer IP address, hostname, or IP:port.</param>
    /// <param name="profileJson">Optional printer profile JSON string for pre-print validation.</param>
    /// <param name="validate">Whether to validate ZPL before sending (default: true).</param>
    public static PrintResult Print(string zpl, string printerAddr, string? profileJson = null, bool validate = true)
    {
        return PrintWithOptions(zpl, printerAddr, profileJson, validate, null, null);
    }

    /// <summary>
    /// Send ZPL to a network printer via TCP with optional timeout/config overrides.
    /// </summary>
    /// <param name="zpl">ZPL source code to print.</param>
    /// <param name="printerAddr">Printer IP address, hostname, or IP:port.</param>
    /// <param name="profileJson">Optional printer profile JSON string for pre-print validation.</param>
    /// <param name="validate">Whether to validate ZPL before sending (default: true).</param>
    /// <param name="timeoutMs">Optional coarse timeout override in milliseconds (0/default to ignore).</param>
    /// <param name="configJson">Optional granular transport config JSON.</param>
    public static PrintResult PrintWithOptions(
        string zpl,
        string printerAddr,
        string? profileJson = null,
        bool validate = true,
        ulong? timeoutMs = null,
        string? configJson = null)
    {
        if (string.IsNullOrEmpty(profileJson)) profileJson = null;
        if (string.IsNullOrEmpty(configJson)) configJson = null;
        var json = ConsumePtr(
            zpl_print_with_options(
                zpl,
                printerAddr,
                profileJson,
                validate,
                timeoutMs ?? 0UL,
                configJson))
            ?? throw new InvalidOperationException("zpl_print_with_options returned NULL");
        // Don't use CheckForError here — print_zpl returns
        // {"success": false, "error": "validation_failed", "issues": [...]}
        // for validation failures, which is a valid PrintResult (not an FFI error).
        // Only treat as FFI error when there's no "success" field.
        using var doc = JsonDocument.Parse(json);
        if (!doc.RootElement.TryGetProperty("success", out _)
            && doc.RootElement.TryGetProperty("error", out var errElem))
        {
            throw new InvalidOperationException(
                errElem.GetString() ?? "Unknown error from native library");
        }
        return Deserialize<PrintResult>(json);
    }

    /// <summary>
    /// Query a printer's host status via ~HS.
    /// </summary>
    /// <param name="printerAddr">Printer IP address, hostname, or IP:port.</param>
    /// <returns>Raw JSON string with the printer's host status fields.</returns>
    public static string QueryStatus(string printerAddr)
    {
        return QueryStatusWithOptions(printerAddr, null, null);
    }

    /// <summary>
    /// Query a printer's host status via ~HS with optional timeout/config overrides.
    /// </summary>
    public static string QueryStatusWithOptions(string printerAddr, ulong? timeoutMs = null, string? configJson = null)
    {
        if (string.IsNullOrEmpty(configJson)) configJson = null;
        var json = ConsumePtr(zpl_query_status_with_options(printerAddr, timeoutMs ?? 0UL, configJson))
            ?? throw new InvalidOperationException("zpl_query_status_with_options returned NULL");
        CheckForError(json);
        return json;
    }

    /// <summary>
    /// Query a printer's host status via ~HS and deserialize into a typed object.
    /// </summary>
    public static HostStatus QueryStatusTyped(string printerAddr, ulong? timeoutMs = null, string? configJson = null)
    {
        var json = QueryStatusWithOptions(printerAddr, timeoutMs, configJson);
        return Deserialize<HostStatus>(json);
    }

    /// <summary>
    /// Query printer identification via ~HI.
    /// </summary>
    public static string QueryInfo(string printerAddr)
    {
        return QueryInfoWithOptions(printerAddr, null, null);
    }

    /// <summary>
    /// Query printer identification via ~HI with optional timeout/config overrides.
    /// </summary>
    public static string QueryInfoWithOptions(string printerAddr, ulong? timeoutMs = null, string? configJson = null)
    {
        if (string.IsNullOrEmpty(configJson)) configJson = null;
        var json = ConsumePtr(zpl_query_info_with_options(printerAddr, timeoutMs ?? 0UL, configJson))
            ?? throw new InvalidOperationException("zpl_query_info_with_options returned NULL");
        CheckForError(json);
        return json;
    }

    /// <summary>
    /// Query printer identification via ~HI and deserialize into a typed object.
    /// </summary>
    public static PrinterInfo QueryInfoTyped(string printerAddr, ulong? timeoutMs = null, string? configJson = null)
    {
        var json = QueryInfoWithOptions(printerAddr, timeoutMs, configJson);
        return Deserialize<PrinterInfo>(json);
    }
}
