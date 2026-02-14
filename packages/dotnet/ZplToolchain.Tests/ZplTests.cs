using System;
using Xunit;

namespace ZplToolchain.Tests;

public class ZplTests
{
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
}
