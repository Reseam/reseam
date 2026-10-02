// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(target_os = "wasi")]

use reseam_sdk::{InspectRequest, PatchRequest};
use reseam_sdk::{Problem, SdkError};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "operation", content = "request", rename_all = "snake_case")]
enum Request {
    Inspect(InspectRequest),
    Patch(PatchRequest),
}

#[repr(C)]
pub struct Buffer {
    length: u32,
    width: u32,
}

unsafe extern "C" {
    fn reseam_buffer_alloc(length: u32, width: u32) -> *mut Buffer;
}

#[cfg(target_os = "wasi")]
#[link(wasm_import_module = "reseam_host")]
unsafe extern "C" {
    fn event(ptr: *const u8, length: usize);
}

#[derive(Serialize)]
#[serde(untagged)]
enum Response {
    Success { value: serde_json::Value },
    Failure { error: SdkError },
}

fn invalid(message: impl Into<String>) -> SdkError {
    SdkError {
        problem: Problem::Other,
        message: message.into(),
    }
}

pub struct BrowserState {
    prepared: Option<reseam_sdk::PreparedInspection>,
}

/// Allocates state owned by one browser engine worker.
#[unsafe(no_mangle)]
pub extern "C" fn reseam_session_new() -> *mut BrowserState {
    Box::into_raw(Box::new(BrowserState { prepared: None }))
}

/// Releases state after all requests have completed.
///
/// # Safety
/// `state` must be a live pointer returned by `reseam_session_new`, used once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reseam_session_free(state: *mut BrowserState) {
    if !state.is_null() {
        // SAFETY: the host transfers the unique allocation back to Rust.
        drop(unsafe { Box::from_raw(state) });
    }
}

fn dispatch(state: &mut BrowserState, bytes: &[u8]) -> Result<serde_json::Value, SdkError> {
    let request =
        serde_json::from_slice::<Request>(bytes).map_err(|error| invalid(error.to_string()))?;
    match request {
        Request::Inspect(request) => {
            state.prepared = None;
            let prepared = reseam_sdk::PreparedInspection::open(&request)
                .map_err(|error| reseam_sdk::sdk_error(&error))?;
            let value = serde_json::to_value(prepared.metadata())
                .map_err(|error| invalid(error.to_string()))?;
            state.prepared = Some(prepared);
            Ok(value)
        }
        Request::Patch(request) => match state.prepared.take() {
            Some(prepared) => prepared.patch(&request, |event| emit(&event)),
            None => reseam_sdk::patch(&request, |event| emit(&event)),
        }
        .map_err(|error| reseam_sdk::sdk_error(&error))
        .and_then(|value| serde_json::to_value(value).map_err(|error| invalid(error.to_string()))),
    }
}

fn emit(event: &reseam_sdk::RunEvent) {
    let bytes = serde_json::to_vec(event)
        .expect("run events contain only JSON-serializable text and status");
    // SAFETY: the host copies this borrowed event before returning.
    unsafe {
        self::event(bytes.as_ptr(), bytes.len());
    }
}

/// Dispatches a host request. The host must supply an allocated, readable input
/// range and free the returned transport buffer with `reseam_buffer_free`.
///
/// # Safety
/// `ptr` must point to `length` initialized bytes in this instance's memory;
/// `state` must be live, uniquely borrowed state from `reseam_session_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reseam_request(
    state: *mut BrowserState,
    ptr: *const u8,
    length: usize,
) -> *mut Buffer {
    let result = if state.is_null() || ptr.is_null() || length > 16 * 1024 * 1024 {
        Err(invalid("invalid browser request size"))
    } else {
        // SAFETY: the host guarantees that the input range is readable.
        unsafe { dispatch(&mut *state, std::slice::from_raw_parts(ptr, length)) }
    };
    let response = match result {
        Ok(value) => Response::Success { value },
        Err(error) => Response::Failure { error },
    };
    let bytes =
        serde_json::to_vec(&response).expect("host responses contain only serializable data");
    let Ok(length) = u32::try_from(bytes.len()) else {
        return std::ptr::null_mut();
    };
    // SAFETY: the transport allocator returns length writable bytes after its header.
    unsafe {
        let buffer = reseam_buffer_alloc(length, 1);
        if !buffer.is_null() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.add(1).cast::<u8>(), bytes.len());
        }
        buffer
    }
}
