// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#![cfg(target_os = "wasi")]

use std::fs::File;
use std::os::wasi::io::FromRawFd;

#[link(wasm_import_module = "reseam_compression")]
unsafe extern "C" {
    fn error(pointer: *const u8, length: usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn compression_alloc(length: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; length].into_boxed_slice()).cast::<u8>()
}

/// # Safety
/// The pointer and length must identify one live `compression_alloc` allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_free(pointer: *mut u8, length: usize) {
    // SAFETY: the host returns the complete unique allocation exactly once.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, length)) });
}

/// Streams input into the output ZIP and closes both transferred descriptors.
///
/// # Safety
/// Descriptors must be live and uniquely owned. `name` must contain `length`
/// readable bytes, remaining valid until this synchronous call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_run(
    input: i32,
    output: i32,
    name: *const u8,
    length: usize,
    level: i64,
) -> u32 {
    // SAFETY: the host transfers unique descriptors for the duration of this job.
    let (input, output) = unsafe { (File::from_raw_fd(input), File::from_raw_fd(output)) };
    // SAFETY: the host guarantees the allocated name range is readable.
    let name = unsafe { std::slice::from_raw_parts(name, length) };
    let result = std::str::from_utf8(name)
        .map_err(|error| error.to_string())
        .and_then(|name| {
            reseam_apk::compression::compress_dex_entry(input, output, name, level)
                .map(drop)
                .map_err(|error| error.to_string())
        });
    if let Err(message) = result {
        // SAFETY: the host copies the error text before returning.
        unsafe { error(message.as_ptr(), message.len()) };
        return 1;
    }
    0
}
