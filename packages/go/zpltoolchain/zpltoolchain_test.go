package zpltoolchain

import (
	"strings"
	"testing"
)

func TestParseReturnsAst(t *testing.T) {
	result, err := Parse("^XA^FDHello^FS^XZ")
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}
	if result == nil {
		t.Fatal("Parse returned nil result")
	}
	if len(result.Ast.Labels) == 0 {
		t.Fatal("expected at least one parsed label")
	}
}

func TestFormatReturnsOutput(t *testing.T) {
	formatted, err := Format("^XA^FD Hello ^FS^XZ", "label")
	if err != nil {
		t.Fatalf("Format failed: %v", err)
	}
	if !strings.Contains(formatted, "^XA") {
		t.Fatalf("formatted output missing ^XA: %q", formatted)
	}
}

func TestValidateReturnsResult(t *testing.T) {
	vr, err := Validate("^XA^FDHello^FS^XZ", "")
	if err != nil {
		t.Fatalf("Validate failed: %v", err)
	}
	if vr == nil {
		t.Fatal("Validate returned nil result")
	}
}

func TestExplainUnknownCodeReturnsEmptyString(t *testing.T) {
	text := Explain("ZPL9999")
	if text != "" {
		t.Fatalf("expected empty explanation for unknown code, got: %q", text)
	}
}

func TestParseWithTablesRejectsInvalidJson(t *testing.T) {
	result, err := ParseWithTables("^XA^FDHello^FS^XZ", "{invalid")
	if err == nil {
		t.Fatalf("expected parse-with-tables error, got result: %#v", result)
	}
	if !strings.Contains(strings.ToLower(err.Error()), "error") && !strings.Contains(strings.ToLower(err.Error()), "invalid") {
		t.Fatalf("expected invalid tables json error, got: %v", err)
	}
}

func TestValidateWithTablesRejectsInvalidJson(t *testing.T) {
	result, err := ValidateWithTables("^XA^FDHello^FS^XZ", "{invalid", "")
	if err == nil {
		t.Fatalf("expected validate-with-tables error, got result: %#v", result)
	}
	if !strings.Contains(strings.ToLower(err.Error()), "error") && !strings.Contains(strings.ToLower(err.Error()), "invalid") {
		t.Fatalf("expected invalid tables json error, got: %v", err)
	}
}
