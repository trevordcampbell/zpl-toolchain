// Package zpltoolchain provides Go bindings for the ZPL toolchain via the C FFI.
//
// All functions call into the shared library (libzpl_toolchain_ffi) and
// unmarshal JSON results into native Go types.
package zpltoolchain

import (
	"encoding/json"
	"fmt"
)

// Ast represents a parsed ZPL document.
type Ast struct {
	Labels []Label `json:"labels"`
}

// Label represents a single ZPL label (^XA ... ^XZ block).
type Label struct {
	Nodes []Node `json:"nodes"`
}

// NodeKind identifies the type of an AST node.
type NodeKind string

const (
	NodeCommand   NodeKind = "Command"
	NodeFieldData NodeKind = "FieldData"
	NodeRawData   NodeKind = "RawData"
	NodeTrivia    NodeKind = "Trivia"
)

// nodeHeader is used to peek at the "kind" discriminator before full deserialization.
type nodeHeader struct {
	Kind NodeKind `json:"kind"`
}

// CommandNode represents a parsed ZPL command.
type CommandNode struct {
	Kind NodeKind  `json:"kind"`
	Code string    `json:"code"`
	Args []ArgSlot `json:"args"`
	Span Span      `json:"span"`
}

// FieldDataNode represents field data content (text between ^FD/^FV and ^FS).
type FieldDataNode struct {
	Kind       NodeKind `json:"kind"`
	Content    string   `json:"content"`
	HexEscaped bool     `json:"hex_escaped"`
	Span       Span     `json:"span"`
}

// RawDataNode represents raw binary/hex payload (e.g., graphic data after ^GF).
type RawDataNode struct {
	Kind    NodeKind `json:"kind"`
	Command string   `json:"command"`
	Data    *string  `json:"data,omitempty"`
	Span    Span     `json:"span"`
}

// TriviaNode represents preserved trivia: comments, whitespace, content outside labels.
type TriviaNode struct {
	Kind NodeKind `json:"kind"`
	Text string   `json:"text"`
	Span Span     `json:"span"`
}

// Node is a union type that can hold any AST node variant.
// Use Kind to determine which variant is populated, then type-assert.
type Node struct {
	Kind     NodeKind
	Command  *CommandNode
	Field    *FieldDataNode
	Raw      *RawDataNode
	Trivia   *TriviaNode
}

// UnmarshalJSON implements custom JSON unmarshaling for the internally-tagged Node enum.
func (n *Node) UnmarshalJSON(data []byte) error {
	var header nodeHeader
	if err := json.Unmarshal(data, &header); err != nil {
		return err
	}
	n.Kind = header.Kind

	switch header.Kind {
	case NodeCommand:
		var cmd CommandNode
		if err := json.Unmarshal(data, &cmd); err != nil {
			return err
		}
		n.Command = &cmd
	case NodeFieldData:
		var fd FieldDataNode
		if err := json.Unmarshal(data, &fd); err != nil {
			return err
		}
		n.Field = &fd
	case NodeRawData:
		var rd RawDataNode
		if err := json.Unmarshal(data, &rd); err != nil {
			return err
		}
		n.Raw = &rd
	case NodeTrivia:
		var t TriviaNode
		if err := json.Unmarshal(data, &t); err != nil {
			return err
		}
		n.Trivia = &t
	default:
		return fmt.Errorf("unknown node kind: %q", header.Kind)
	}
	return nil
}

// ArgSlot represents a parsed argument of a command.
type ArgSlot struct {
	Key      *string `json:"key,omitempty"`
	Presence string  `json:"presence"`
	Value    *string `json:"value,omitempty"`
}

// Span represents a byte range in the source input.
type Span struct {
	Start int `json:"start"`
	End   int `json:"end"`
}

// Diagnostic represents a single diagnostic message.
type Diagnostic struct {
	ID       string            `json:"id"`
	Severity string            `json:"severity"`
	Message  string            `json:"message"`
	Span     *Span             `json:"span,omitempty"`
	Context  map[string]string `json:"context,omitempty"`
}

// ParseResult is the result of parsing ZPL input.
type ParseResult struct {
	Ast         Ast          `json:"ast"`
	Diagnostics []Diagnostic `json:"diagnostics"`
}

// ValidationResult is the result of validating ZPL input.
type ValidationResult struct {
	OK     bool         `json:"ok"`
	Issues []Diagnostic `json:"issues"`
}

// PrintResult is the result of sending ZPL to a printer.
type PrintResult struct {
	Success  bool         `json:"success"`
	BytesSent int         `json:"bytes_sent"`
	Error    string       `json:"error,omitempty"`
	Issues   []Diagnostic `json:"issues,omitempty"`
}
