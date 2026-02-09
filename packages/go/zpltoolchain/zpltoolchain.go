package zpltoolchain

/*
#cgo LDFLAGS: -lzpl_toolchain_ffi
#include <stdlib.h>
#include <stdbool.h>

// ZPL toolchain C FFI functions.
extern char* zpl_parse(const char* input);
extern char* zpl_parse_with_tables(const char* input, const char* tables_json);
extern char* zpl_validate(const char* input, const char* profile_json);
extern char* zpl_format(const char* input, const char* indent);
extern char* zpl_explain(const char* id);
extern char* zpl_print(const char* zpl, const char* printer_addr, const char* profile_json, _Bool validate);
extern char* zpl_query_status(const char* printer_addr);
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

// Print sends ZPL to a network printer via TCP (port 9100).
//
// If validate is true, the ZPL is validated first using the optional profileJSON.
// printerAddr can be an IP address, hostname, or IP:port (default port 9100).
// profileJSON is an optional printer profile JSON string (pass "" for none).
func Print(zpl string, printerAddr string, profileJSON string, validate bool) (*PrintResult, error) {
	cZpl := C.CString(zpl)
	defer C.free(unsafe.Pointer(cZpl))

	cAddr := C.CString(printerAddr)
	defer C.free(unsafe.Pointer(cAddr))

	var cProfile *C.char
	if profileJSON != "" {
		cProfile = C.CString(profileJSON)
		defer C.free(unsafe.Pointer(cProfile))
	}

	cValidate := C._Bool(validate)

	cResult := C.zpl_print(cZpl, cAddr, cProfile, cValidate)
	if cResult == nil {
		return nil, fmt.Errorf("zpl_print returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	// Don't use checkFFIError here — print_zpl returns {"success": false, "error": "validation_failed", "issues": [...]}
	// for validation failures, which is a valid PrintResult, not an FFI error.
	// Only treat as FFI error when there's no "success" field (pure error response).
	var probe struct {
		Success *bool  `json:"success"`
		Error   string `json:"error"`
	}
	if err := json.Unmarshal([]byte(jsonStr), &probe); err == nil && probe.Success == nil && probe.Error != "" {
		return nil, fmt.Errorf("zpl_print: %s", probe.Error)
	}
	var result PrintResult
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal print result: %w", err)
	}
	return &result, nil
}

// QueryStatus queries a printer's host status via ~HS and returns the raw JSON response.
//
// printerAddr can be an IP address, hostname, or IP:port (default port 9100).
func QueryStatus(printerAddr string) (string, error) {
	cAddr := C.CString(printerAddr)
	defer C.free(unsafe.Pointer(cAddr))

	cResult := C.zpl_query_status(cAddr)
	if cResult == nil {
		return "", fmt.Errorf("zpl_query_status returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	if err := checkFFIError(jsonStr); err != nil {
		return "", fmt.Errorf("zpl_query_status: %w", err)
	}
	return jsonStr, nil
}
