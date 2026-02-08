using System.Runtime.InteropServices;
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

    // ── P/Invoke declarations ───────────────────────────────────────────

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_parse(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string input);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_parse_with_tables(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string input,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string tablesJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_validate(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string input,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? profileJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_format(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string input,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? indent);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr zpl_explain(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void zpl_free(IntPtr ptr);

    // ── Helpers ─────────────────────────────────────────────────────────

    private static string? ConsumePtr(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        try
        {
            return Marshal.PtrToStringUTF8(ptr);
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
        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.TryGetProperty("error", out var errorElement))
        {
            var message = errorElement.GetString() ?? "Unknown error from native library";
            throw new InvalidOperationException(message);
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
        var json = ConsumePtr(zpl_validate(input, profileJson))
            ?? throw new InvalidOperationException("zpl_validate returned NULL");
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
        return ConsumePtr(zpl_format(input, indent))
            ?? throw new InvalidOperationException("zpl_format returned NULL");
    }

    /// <summary>
    /// Explain a diagnostic code (e.g., "ZPL1201").
    /// </summary>
    /// <returns>The explanation, or null if the code is unknown.</returns>
    public static string? Explain(string id)
    {
        return ConsumePtr(zpl_explain(id));
    }
}
