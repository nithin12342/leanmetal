//! packages/rust/src/ffi.rs
//! C-ABI Foreign Function Interface (FFI) bindings for Zero-Overhead Abstraction.
//! Allows Python, Go, Node.js, C#, and Java to invoke EdgeFlag evaluation with sub-microsecond overhead.

use crate::hasher::is_in_rollout;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uchar};

/// FFI: Computes xxHash64 percentage rollout bucket for a given flag and user ID.
/// Returns bucket in range [0..99], or 255 on invalid UTF-8 pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn edgeflag_compute_bucket(
    flag_key: *const c_char,
    user_id: *const c_char,
) -> c_uchar {
    if flag_key.is_null() || user_id.is_null() {
        return 255;
    }

    let flag_str = match unsafe { CStr::from_ptr(flag_key) }.to_str() {
        Ok(s) => s,
        Err(_) => return 255,
    };
    let user_str = match unsafe { CStr::from_ptr(user_id) }.to_str() {
        Ok(s) => s,
        Err(_) => return 255,
    };

    crate::hasher::compute_bucket(flag_str, user_str)
}

/// FFI: Evaluates rollout inclusion with zero heap allocations.
/// Returns 1 for true, 0 for false, or 255 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn edgeflag_is_in_rollout(
    flag_key: *const c_char,
    user_id: *const c_char,
    percentage: c_uchar,
) -> c_uchar {
    if flag_key.is_null() || user_id.is_null() {
        return 0;
    }

    let flag_str = match unsafe { CStr::from_ptr(flag_key) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let user_str = match unsafe { CStr::from_ptr(user_id) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    if is_in_rollout(flag_str, user_str, percentage) {
        1
    } else {
        0
    }
}

/// FFI: Evaluates a JSON-encoded FlagDefinition against a JSON-encoded EvaluationContext.
/// Returns a freshly allocated JSON string pointer representing EvaluationResult.
/// Caller must free memory via `edgeflag_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn edgeflag_evaluate_json(
    flag_json: *const c_char,
    context_json: *const c_char,
) -> *mut c_char {
    if flag_json.is_null() || context_json.is_null() {
        return std::ptr::null_mut();
    }

    let flag_str = match unsafe { CStr::from_ptr(flag_json) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let ctx_str = match unsafe { CStr::from_ptr(context_json) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let flag: crate::models::FlagDefinition = match serde_json::from_str(flag_str) {
        Ok(f) => f,
        Err(_) => return std::ptr::null_mut(),
    };

    let ctx: crate::models::EvaluationContext = match serde_json::from_str(ctx_str) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = crate::evaluator::evaluate_flag_zero_overhead(&flag, &ctx);
    let result_json = match serde_json::to_string(&result) {
        Ok(j) => j,
        Err(_) => return std::ptr::null_mut(),
    };

    match CString::new(result_json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// FFI: Frees a string previously allocated by `edgeflag_evaluate_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn edgeflag_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}
