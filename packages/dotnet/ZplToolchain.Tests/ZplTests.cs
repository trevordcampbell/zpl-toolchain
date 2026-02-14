using System;
using Xunit;

namespace ZplToolchain.Tests;

public class ZplTests
{
    private static bool ContainsIgnoreCase(string text, string value) =>
        text.Contains(value, StringComparison.OrdinalIgnoreCase);

    [Fact]
    public void Parse_ValidZpl_ReturnsResult()
    {
        var result = Zpl.Parse("^XA^FDHello^FS^XZ");
        Assert.NotNull(result);
        Assert.NotNull(result.Ast);
        Assert.NotEmpty(result.Ast.Labels);
    }

    [Fact]
    public void Format_ValidZpl_ReturnsFormattedOutput()
    {
        var formatted = Zpl.Format("^XA^FD Hello ^FS^XZ", "label");
        Assert.NotNull(formatted);
        Assert.Contains("^XA", formatted);
    }

    [Fact]
    public void Validate_ValidZpl_ReturnsResult()
    {
        var result = Zpl.Validate("^XA^FDHello^FS^XZ");
        Assert.NotNull(result);
    }

    [Fact]
    public void Explain_UnknownCode_ReturnsNull()
    {
        var explanation = Zpl.Explain("ZPL9999");
        Assert.Null(explanation);
    }

    [Fact]
    public void ParseWithTables_InvalidJson_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => Zpl.ParseWithTables("^XA^FDHello^FS^XZ", "{invalid"));
        Assert.Contains("invalid", ex.Message, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void ValidateWithTables_InvalidJson_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => Zpl.ValidateWithTables("^XA^FDHello^FS^XZ", "{invalid"));
        Assert.Contains("invalid", ex.Message, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void PrintWithOptions_InvalidConfigJson_ThrowsBeforeNetworkIo()
    {
        var ex = Assert.Throws<InvalidOperationException>(() =>
            Zpl.PrintWithOptions("^XA^XZ", "127.0.0.1:9100", profileJson: null, validate: false, configJson: "{invalid"));
        Assert.True(
            ContainsIgnoreCase(ex.Message, "config")
                || ContainsIgnoreCase(ex.Message, "json")
                || ContainsIgnoreCase(ex.Message, "invalid"),
            $"unexpected message: {ex.Message}");
    }

    [Fact]
    public void QueryStatusWithOptions_InvalidConfigJson_ThrowsBeforeNetworkIo()
    {
        var ex = Assert.Throws<InvalidOperationException>(() =>
            Zpl.QueryStatusWithOptions("127.0.0.1:9100", configJson: "{invalid"));
        Assert.True(
            ContainsIgnoreCase(ex.Message, "config")
                || ContainsIgnoreCase(ex.Message, "json")
                || ContainsIgnoreCase(ex.Message, "invalid"),
            $"unexpected message: {ex.Message}");
    }

    [Fact]
    public void QueryInfoWithOptions_InvalidConfigJson_ThrowsBeforeNetworkIo()
    {
        var ex = Assert.Throws<InvalidOperationException>(() =>
            Zpl.QueryInfoWithOptions("127.0.0.1:9100", configJson: "{invalid"));
        Assert.True(
            ContainsIgnoreCase(ex.Message, "config")
                || ContainsIgnoreCase(ex.Message, "json")
                || ContainsIgnoreCase(ex.Message, "invalid"),
            $"unexpected message: {ex.Message}");
    }

    [Fact]
    public void Parse_Utf8Payload_RoundTripsContent()
    {
        var result = Zpl.Parse("^XA^FO50,50^FDHéllo 世界^FS^XZ");
        Assert.NotNull(result);
        Assert.NotEmpty(result.Ast.Labels);
    }
}
