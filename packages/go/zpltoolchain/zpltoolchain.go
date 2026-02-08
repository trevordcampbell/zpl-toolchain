package zpltoolchain

/*
#cgo LDFLAGS: -lzpl_toolchain_ffi
#include <stdlib.h>

// ZPL toolchain C FFI functions.
extern char* zpl_parse(const char* input);
extern char* zpl_parse_with_tables(const char* input, const char* tables_json);
extern char* zpl_validate(const char* input, const char* profile_json);
extern char* zpl_format(const char* input, const char* indent);
extern char* zpl_explain(const char* id);
extern void  zpl_free(char* ptr);
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"unsafe" // #nosec G103 — required for cgo interop
)

// checkFFIError inspects JSON returned by the FFI for an {"error": "..."} response.
// Returns the error message if present, nil otherwise.
func checkFFIError(jsonStr string) error {
	var resp struct {
		Error string `json:"error"`
	}
	if err := json.Unmarshal([]byte(jsonStr), &resp); err == nil && resp.Error != "" {
		return fmt.Errorf("%s", resp.Error)
	}
	return nil
}

// Parse parses a ZPL string and returns the AST with diagnostics.
// Uses embedded parser tables when available.
func Parse(input string) (*ParseResult, error) {
	cInput := C.CString(input)
	defer C.free(unsafe.Pointer(cInput))

	cResult := C.zpl_parse(cInput)
	if cResult == nil {
		return nil, fmt.Errorf("zpl_parse returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	var result ParseResult
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal parse result: %w", err)
	}
	return &result, nil
}

// ParseWithTables parses a ZPL string with explicitly provided parser tables (JSON string).
func ParseWithTables(input string, tablesJSON string) (*ParseResult, error) {
	cInput := C.CString(input)
	defer C.free(unsafe.Pointer(cInput))

	cTables := C.CString(tablesJSON)
	defer C.free(unsafe.Pointer(cTables))

	cResult := C.zpl_parse_with_tables(cInput, cTables)
	if cResult == nil {
		return nil, fmt.Errorf("zpl_parse_with_tables returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	if err := checkFFIError(jsonStr); err != nil {
		return nil, fmt.Errorf("zpl_parse_with_tables: %w", err)
	}
	var result ParseResult
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal parse result: %w", err)
	}
	return &result, nil
}

// Validate parses and validates a ZPL string.
// profileJSON is an optional printer profile JSON string (pass "" for none).
func Validate(input string, profileJSON string) (*ValidationResult, error) {
	cInput := C.CString(input)
	defer C.free(unsafe.Pointer(cInput))

	var cProfile *C.char
	if profileJSON != "" {
		cProfile = C.CString(profileJSON)
		defer C.free(unsafe.Pointer(cProfile))
	}

	cResult := C.zpl_validate(cInput, cProfile)
	if cResult == nil {
		return nil, fmt.Errorf("zpl_validate returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	if err := checkFFIError(jsonStr); err != nil {
		return nil, fmt.Errorf("zpl_validate: %w", err)
	}
	var result ValidationResult
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal validation result: %w", err)
	}
	return &result, nil
}

// Format formats a ZPL string with the specified indentation style.
// indent can be "none", "label", or "field" (pass "" for default "none").
func Format(input string, indent string) (string, error) {
	cInput := C.CString(input)
	defer C.free(unsafe.Pointer(cInput))

	var cIndent *C.char
	if indent != "" {
		cIndent = C.CString(indent)
		defer C.free(unsafe.Pointer(cIndent))
	}

	cResult := C.zpl_format(cInput, cIndent)
	if cResult == nil {
		return "", fmt.Errorf("zpl_format returned NULL")
	}
	defer C.zpl_free(cResult)

	return C.GoString(cResult), nil
}

// Explain returns a human-readable explanation for a diagnostic code.
// Returns an empty string if the code is unknown.
func Explain(id string) string {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	cResult := C.zpl_explain(cID)
	if cResult == nil {
		return ""
	}
	defer C.zpl_free(cResult)

	return C.GoString(cResult)
}
