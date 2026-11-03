// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The engine calls a web page needs while a user picks patches and keys,
//! which read no files and run no patch: small enough to load on the page's
//! main thread and answer at once.

#![cfg(target_os = "wasi")]

use reseam_model::{
    ChangesRequest, OptionDeclaration, OptionValue, ParsedOption, PlanRequest, SdkError,
    SigningKeyMaterial,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "operation", content = "request", rename_all = "snake_case")]
enum Request {
    Plan(PlanRequest),
    Changes(ChangesRequest),
    ParseOption {
        option: OptionDeclaration,
        text: String,
    },
    OptionText(OptionValue),
    ImportPkcs12 {
        keystore: Vec<u8>,
        password: String,
    },
    ExportPkcs12 {
        material: SigningKeyMaterial,
        password: String,
    },
    CertificateFingerprint(Vec<u8>),
}

#[derive(Serialize)]
#[serde(untagged)]
enum Response {
    Success { value: serde_json::Value },
    Failure { error: SdkError },
}

fn dispatch(bytes: &[u8]) -> Result<serde_json::Value, SdkError> {
    let request: Request =
        serde_json::from_slice(bytes).map_err(|error| SdkError::other(error.to_string()))?;
    let value = match request {
        Request::Plan(request) => json(&reseam_sdk::plan(&request)?),
        Request::Changes(request) => json(&reseam_sdk::changes(&request)?),
        Request::ParseOption { option, text } => json(&match option.parse(&text) {
            Ok(value) => ParsedOption::Value { value },
            Err(problem) => ParsedOption::Problem { problem },
        }),
        Request::OptionText(value) => json(&value.to_string()),
        Request::ImportPkcs12 { keystore, password } => {
            json(&reseam_sdk::import_pkcs12(&keystore, &password)?)
        }
        Request::ExportPkcs12 { material, password } => {
            json(&reseam_sdk::export_pkcs12(&material, &password)?)
        }
        Request::CertificateFingerprint(certificate) => {
            json(&reseam_sdk::certificate_fingerprint(&certificate))
        }
    };
    value.map_err(|error| SdkError::other(error.to_string()))
}

fn json(value: &impl Serialize) -> serde_json::Result<serde_json::Value> {
    serde_json::to_value(value)
}

/// Allocates `length` bytes for the host to write a request into.
#[unsafe(no_mangle)]
pub extern "C" fn tools_alloc(length: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; length].into_boxed_slice()).cast::<u8>()
}

/// Frees a request or response allocation.
///
/// # Safety
/// The pointer and length must identify one live `tools_alloc` allocation or
/// one response `tools_call` returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tools_free(pointer: *mut u8, length: usize) {
    // SAFETY: the host returns the complete unique allocation exactly once.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, length)) });
}

/// Answers the JSON request at `pointer` with a JSON response, returned as
/// its address in the high and its length in the low 32 bits. The host frees
/// both with `tools_free`.
///
/// # Safety
/// `pointer` must point to `length` initialized bytes in this instance's memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tools_call(pointer: *const u8, length: usize) -> u64 {
    // SAFETY: the host guarantees that the input range is readable.
    let input = unsafe { std::slice::from_raw_parts(pointer, length) };
    let response = match dispatch(input) {
        Ok(value) => Response::Success { value },
        Err(error) => Response::Failure { error },
    };
    let bytes = serde_json::to_vec(&response)
        .expect("responses contain only serializable data")
        .into_boxed_slice();
    let length = bytes.len() as u64;
    let pointer = Box::into_raw(bytes).cast::<u8>() as usize as u64;
    (pointer << 32) | length
}
