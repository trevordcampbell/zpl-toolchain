//! C FFI bindings for the ZPL toolchain.
//!
//! All functions accept null-terminated C strings and return heap-allocated
//! JSON strings. The caller MUST free returned strings with `zpl_free()`.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use zpl_toolchain_bindings_common as common;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Convert a C string pointer to a Rust `&str`. Returns `None` if null or invalid UTF-8.
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Allocate a C string from a Rust string. Returns null on failure.
fn to_c_string(s: &str) -> *mut c_char {
    CString::new(s)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// Serialize a value to a JSON C string.
fn to_json_c<T: serde::Serialize>(value: &T) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(json) => to_c_string(&json),
        Err(_) => ptr::null_mut(),
    }
}

// ── Public API ──────────────────────────────────────────────────────────

/// Parse a ZPL string. Returns a JSON string with `{ "ast": ..., "diagnostics": [...] }`.
///
/// The caller MUST free the returned pointer with `zpl_free()`.
/// Returns NULL on invalid input.
///
/// # Safety
///
/// `input` must be a valid, null-terminated C string pointer (or NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zpl_parse(input: *const c_char) -> *mut c_char {
    let Some(input) = (unsafe { cstr_to_str(input) }) else {
        return ptr::null_mut();
    };

    let result = common::parse_zpl(input);
    to_json_c(&result)
}

/// Parse a ZPL string with explicitly provided parser tables (JSON string).
///
/// Returns a JSON string with `{ "ast": ..., "diagnostics": [...] }`.
/// The caller MUST free the returned pointer with `zpl_free()`.
/// Returns NULL on invalid input.
///
/// # Safety
///
/// `input` and `tables_json` must be valid, null-terminated C string pointers (or NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zpl_parse_with_tables(
    input: *const c_char,
    tables_json: *const c_char,
) -> *mut c_char {
    let Some(input) = (unsafe { cstr_to_str(input) }) else {
        return ptr::null_mut();
    };
    let Some(tables_str) = (unsafe { cstr_to_str(tables_json) }) else {
        return ptr::null_mut();
    };

    match common::parse_zpl_with_tables_json(input, tables_str) {
        Ok(result) => to_json_c(&result),
        Err(e) => {
            let out = serde_json::json!({"error": e});
            to_json_c(&out)
        }
    }
}

/// Parse and validate a ZPL string. Returns a JSON string with `{ "ok": ..., "issues": [...] }`.
///
/// `profile_json` is an optional null-terminated JSON string for a printer profile.
/// Pass NULL to validate without a profile.
///
/// The caller MUST free the returned pointer with `zpl_free()`.
///
/// # Safety
///
/// `input` and `profile_json` must be valid, null-terminated C string pointers (or NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zpl_validate(
    input: *const c_char,
    profile_json: *const c_char,
) -> *mut c_char {
    let Some(input) = (unsafe { cstr_to_str(input) }) else {
        return ptr::null_mut();
    };

    let profile_str = unsafe { cstr_to_str(profile_json) };

    match common::validate_zpl(input, profile_str) {
        Ok(vr) => to_json_c(&vr),
        Err(e) => {
            let out = serde_json::json!({"error": e});
            to_json_c(&out)
        }
    }
}

/// Format a ZPL string. Returns the formatted ZPL as a C string.
///
/// `indent` is a null-terminated string: "none", "label", or "field". Pass NULL for "none".
///
/// The caller MUST free the returned pointer with `zpl_free()`.
///
/// # Safety
///
/// `input` and `indent` must be valid, null-terminated C string pointers (or NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zpl_format(input: *const c_char, indent: *const c_char) -> *mut c_char {
    let Some(input) = (unsafe { cstr_to_str(input) }) else {
        return ptr::null_mut();
    };

    let indent_str = unsafe { cstr_to_str(indent) };
    let formatted = common::format_zpl(input, indent_str);
    to_c_string(&formatted)
}

/// Explain a diagnostic code. Returns the explanation as a C string, or NULL if unknown.
///
/// The caller MUST free the returned pointer with `zpl_free()`.
///
/// # Safety
///
/// `id` must be a valid, null-terminated C string pointer (or NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zpl_explain(id: *const c_char) -> *mut c_char {
    let Some(id) = (unsafe { cstr_to_str(id) }) else {
        return ptr::null_mut();
    };

    match common::explain_diagnostic(id) {
        Some(text) => to_c_string(text),
        None => ptr::null_mut(),
    }
}

/// Free a string previously returned by any `zpl_*` function.
///
/// Passing NULL is safe (no-op).
///
/// # Safety
///
/// `ptr` must be a pointer previously returned by a `zpl_*` function, or NULL.
/// Each pointer must be freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zpl_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}
