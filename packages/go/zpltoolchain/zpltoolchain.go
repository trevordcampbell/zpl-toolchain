package zpltoolchain

/*
#cgo LDFLAGS: -lzpl_toolchain_ffi
#include <stdlib.h>
#include <stdbool.h>

// ZPL toolchain C FFI functions.
extern char* zpl_parse(const char* input);
extern char* zpl_parse_with_tables(const char* input, const char* tables_json);
extern char* zpl_validate(const char* input, const char* profile_json);
extern char* zpl_validate_with_tables(const char* input, const char* tables_json, const char* profile_json);
extern char* zpl_format(const char* input, const char* indent);
extern char* zpl_explain(const char* id);
extern char* zpl_print_with_options(const char* zpl, const char* printer_addr, const char* profile_json, _Bool validate, unsigned long long timeout_ms, const char* config_json);
extern char* zpl_query_status_with_options(const char* printer_addr, unsigned long long timeout_ms, const char* config_json);
extern char* zpl_query_info_with_options(const char* printer_addr, unsigned long long timeout_ms, const char* config_json);
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
		Error   string `json:"error"`
		Message string `json:"message"`
	}
	if err := json.Unmarshal([]byte(jsonStr), &resp); err == nil && resp.Error != "" {
		if resp.Message != "" {
			return fmt.Errorf("%s: %s", resp.Error, resp.Message)
		}
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
	if err := checkFFIError(jsonStr); err != nil {
		return nil, fmt.Errorf("zpl_parse: %w", err)
	}
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

// ValidateWithTables parses and validates a ZPL string with explicitly provided parser tables.
// profileJSON is an optional printer profile JSON string (pass "" for none).
func ValidateWithTables(input string, tablesJSON string, profileJSON string) (*ValidationResult, error) {
	cInput := C.CString(input)
	defer C.free(unsafe.Pointer(cInput))

	cTables := C.CString(tablesJSON)
	defer C.free(unsafe.Pointer(cTables))

	var cProfile *C.char
	if profileJSON != "" {
		cProfile = C.CString(profileJSON)
		defer C.free(unsafe.Pointer(cProfile))
	}

	cResult := C.zpl_validate_with_tables(cInput, cTables, cProfile)
	if cResult == nil {
		return nil, fmt.Errorf("zpl_validate_with_tables returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	if err := checkFFIError(jsonStr); err != nil {
		return nil, fmt.Errorf("zpl_validate_with_tables: %w", err)
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

	formatted := C.GoString(cResult)
	if err := checkFFIError(formatted); err != nil {
		return "", fmt.Errorf("zpl_format: %w", err)
	}
	return formatted, nil
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

	text := C.GoString(cResult)
	// Preserve the stable Explain signature (string-only) while avoiding
	// leaking raw FFI error envelopes (e.g. {"error":"ffi_panic"}) to callers.
	if checkFFIError(text) != nil {
		return ""
	}
	return text
}

// Print sends ZPL to a network printer via TCP (port 9100).
//
// If validate is true, the ZPL is validated first using the optional profileJSON.
// printerAddr can be an IP address, hostname, or IP:port (default port 9100).
// profileJSON is an optional printer profile JSON string (pass "" for none).
func Print(zpl string, printerAddr string, profileJSON string, validate bool) (*PrintResult, error) {
	return PrintWithOptions(zpl, printerAddr, profileJSON, validate, nil)
}

// PrintOptions configures transport behavior for PrintWithOptions.
type PrintOptions struct {
	TimeoutMs  uint64
	ConfigJSON string
}

// PrintWithOptions sends ZPL to a network printer with optional timeout/config overrides.
//
// TimeoutMs:
//   - 0 uses defaults (or config_json only)
//   - >0 applies coarse timeout profile (connect/write/read scaling)
//
// ConfigJSON is optional JSON for granular timeout/retry/trace settings.
func PrintWithOptions(zpl string, printerAddr string, profileJSON string, validate bool, opts *PrintOptions) (*PrintResult, error) {
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
	var cTimeoutMs C.ulonglong
	var cConfigJSON *C.char
	if opts != nil {
		cTimeoutMs = C.ulonglong(opts.TimeoutMs)
		if opts.ConfigJSON != "" {
			cConfigJSON = C.CString(opts.ConfigJSON)
			defer C.free(unsafe.Pointer(cConfigJSON))
		}
	}

	cResult := C.zpl_print_with_options(cZpl, cAddr, cProfile, cValidate, cTimeoutMs, cConfigJSON)
	if cResult == nil {
		return nil, fmt.Errorf("zpl_print_with_options returned NULL")
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
		return nil, fmt.Errorf("zpl_print_with_options: %s", probe.Error)
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
	return QueryStatusWithOptions(printerAddr, 0, "")
}

// QueryStatusWithOptions queries printer host status via ~HS with optional timeout/config overrides.
func QueryStatusWithOptions(printerAddr string, timeoutMs uint64, configJSON string) (string, error) {
	cAddr := C.CString(printerAddr)
	defer C.free(unsafe.Pointer(cAddr))

	var cConfigJSON *C.char
	if configJSON != "" {
		cConfigJSON = C.CString(configJSON)
		defer C.free(unsafe.Pointer(cConfigJSON))
	}

	cResult := C.zpl_query_status_with_options(cAddr, C.ulonglong(timeoutMs), cConfigJSON)
	if cResult == nil {
		return "", fmt.Errorf("zpl_query_status_with_options returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	if err := checkFFIError(jsonStr); err != nil {
		return "", fmt.Errorf("zpl_query_status_with_options: %w", err)
	}
	return jsonStr, nil
}

// QueryStatusTyped queries ~HS and unmarshals into a typed HostStatus object.
func QueryStatusTyped(printerAddr string) (*HostStatus, error) {
	return QueryStatusTypedWithOptions(printerAddr, 0, "")
}

// QueryStatusTypedWithOptions queries ~HS with timeout/config overrides and unmarshals typed output.
func QueryStatusTypedWithOptions(printerAddr string, timeoutMs uint64, configJSON string) (*HostStatus, error) {
	jsonStr, err := QueryStatusWithOptions(printerAddr, timeoutMs, configJSON)
	if err != nil {
		return nil, err
	}
	var status HostStatus
	if err := json.Unmarshal([]byte(jsonStr), &status); err != nil {
		return nil, fmt.Errorf("failed to unmarshal host status: %w", err)
	}
	return &status, nil
}

// QueryInfo queries printer identification via ~HI and returns raw JSON.
func QueryInfo(printerAddr string) (string, error) {
	return QueryInfoWithOptions(printerAddr, 0, "")
}

// QueryInfoWithOptions queries ~HI with optional timeout/config overrides.
func QueryInfoWithOptions(printerAddr string, timeoutMs uint64, configJSON string) (string, error) {
	cAddr := C.CString(printerAddr)
	defer C.free(unsafe.Pointer(cAddr))

	var cConfigJSON *C.char
	if configJSON != "" {
		cConfigJSON = C.CString(configJSON)
		defer C.free(unsafe.Pointer(cConfigJSON))
	}

	cResult := C.zpl_query_info_with_options(cAddr, C.ulonglong(timeoutMs), cConfigJSON)
	if cResult == nil {
		return "", fmt.Errorf("zpl_query_info_with_options returned NULL")
	}
	defer C.zpl_free(cResult)

	jsonStr := C.GoString(cResult)
	if err := checkFFIError(jsonStr); err != nil {
		return "", fmt.Errorf("zpl_query_info_with_options: %w", err)
	}
	return jsonStr, nil
}

// QueryInfoTyped queries ~HI and unmarshals into a typed PrinterInfo object.
func QueryInfoTyped(printerAddr string) (*PrinterInfo, error) {
	return QueryInfoTypedWithOptions(printerAddr, 0, "")
}

// QueryInfoTypedWithOptions queries ~HI with timeout/config overrides and unmarshals typed output.
func QueryInfoTypedWithOptions(printerAddr string, timeoutMs uint64, configJSON string) (*PrinterInfo, error) {
	jsonStr, err := QueryInfoWithOptions(printerAddr, timeoutMs, configJSON)
	if err != nil {
		return nil, err
	}
	var info PrinterInfo
	if err := json.Unmarshal([]byte(jsonStr), &info); err != nil {
		return nil, fmt.Errorf("failed to unmarshal printer info: %w", err)
	}
	return &info, nil
}
