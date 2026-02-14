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

// BarcodeDefaults contains typed defaults from ^BY.
type BarcodeDefaults struct {
	ModuleWidth *uint32  `json:"module_width,omitempty"`
	Ratio       *float64 `json:"ratio,omitempty"`
	Height      *uint32  `json:"height,omitempty"`
}

// FontDefaults contains typed defaults from ^CF.
type FontDefaults struct {
	Font   *string `json:"font,omitempty"`
	Height *uint32 `json:"height,omitempty"`
	Width  *uint32 `json:"width,omitempty"`
}

// FieldOrientationDefaults contains typed defaults from ^FW.
type FieldOrientationDefaults struct {
	Orientation   *string `json:"orientation,omitempty"`
	Justification *uint8  `json:"justification,omitempty"`
}

// LabelHome contains typed label-home defaults from ^LH.
type LabelHome struct {
	X float64 `json:"x"`
	Y float64 `json:"y"`
}

// LayoutDefaults contains typed layout defaults.
type LayoutDefaults struct {
	PrintWidth       *float64 `json:"print_width,omitempty"`
	LabelLength      *float64 `json:"label_length,omitempty"`
	PrintOrientation *string  `json:"print_orientation,omitempty"`
	MirrorImage      *string  `json:"mirror_image,omitempty"`
	ReversePrint     *string  `json:"reverse_print,omitempty"`
	LabelTop         *float64 `json:"label_top,omitempty"`
	LabelShift       *float64 `json:"label_shift,omitempty"`
}

// LabelValueState is the typed per-label state snapshot.
type LabelValueState struct {
	Barcode   BarcodeDefaults          `json:"barcode"`
	Font      FontDefaults             `json:"font"`
	Field     FieldOrientationDefaults `json:"field"`
	LabelHome LabelHome                `json:"label_home"`
	Layout    LayoutDefaults           `json:"layout"`
}

// ResolvedLabelState is renderer-ready per-label state from validation output.
type ResolvedLabelState struct {
	Values          LabelValueState `json:"values"`
	EffectiveWidth  *float64        `json:"effective_width,omitempty"`
	EffectiveHeight *float64        `json:"effective_height,omitempty"`
}

// ValidationResult is the result of validating ZPL input.
type ValidationResult struct {
	OK             bool                 `json:"ok"`
	Issues         []Diagnostic         `json:"issues"`
	ResolvedLabels []ResolvedLabelState `json:"resolved_labels,omitempty"`
}

// PrintResult is the result of sending ZPL to a printer.
type PrintResult struct {
	Success  bool         `json:"success"`
	BytesSent int         `json:"bytes_sent"`
	Error    string       `json:"error,omitempty"`
	Issues   []Diagnostic `json:"issues,omitempty"`
}

// HostStatus is the typed parsed response from ~HS.
type HostStatus struct {
	CommunicationFlag      uint32 `json:"communication_flag"`
	PaperOut               bool   `json:"paper_out"`
	Paused                 bool   `json:"paused"`
	LabelLengthDots        uint32 `json:"label_length_dots"`
	FormatsInBuffer        uint32 `json:"formats_in_buffer"`
	BufferFull             bool   `json:"buffer_full"`
	CommDiagMode           bool   `json:"comm_diag_mode"`
	PartialFormat          bool   `json:"partial_format"`
	Reserved1              uint32 `json:"reserved_1"`
	CorruptRAM             bool   `json:"corrupt_ram"`
	UnderTemperature       bool   `json:"under_temperature"`
	OverTemperature        bool   `json:"over_temperature"`
	FunctionSettings       uint32 `json:"function_settings"`
	HeadUp                 bool   `json:"head_up"`
	RibbonOut              bool   `json:"ribbon_out"`
	ThermalTransferMode    bool   `json:"thermal_transfer_mode"`
	PrintMode              string `json:"print_mode"`
	PrintWidthMode         uint32 `json:"print_width_mode"`
	LabelWaiting           bool   `json:"label_waiting"`
	LabelsRemaining        uint32 `json:"labels_remaining"`
	FormatWhilePrinting    uint32 `json:"format_while_printing"`
	GraphicsStoredInMemory uint32 `json:"graphics_stored_in_memory"`
	Password               uint32 `json:"password"`
	StaticRAMInstalled     bool   `json:"static_ram_installed"`
}

// PrinterInfo is the typed parsed response from ~HI.
type PrinterInfo struct {
	Model    string `json:"model"`
	Firmware string `json:"firmware"`
	DPI      uint32 `json:"dpi"`
	MemoryKB uint32 `json:"memory_kb"`
}
