// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;

use reseam_storage::ScratchDir;
use serde::{Deserialize, Serialize};

use super::handles::ContextGuard;
use crate::bundle::declarations::declarations;
use crate::bundle::index::Declaration;
use crate::error::{PatcherError, Result};
use crate::{Patch, PatchPhase, PatchSpec};

#[derive(Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Request<'a> {
    Load {
        jars: &'a [PathBuf],
        bundle: &'a str,
        declarations: Vec<Declaration>,
    },
    Invoke {
        handle: u32,
        patch: &'a str,
        phase: Phase,
    },
    Close {
        handle: u32,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Execute,
    Finalize,
}

#[derive(Deserialize)]
struct Loaded {
    handle: u32,
    patches: Vec<LoadedPatch>,
}

#[derive(Deserialize)]
struct LoadedPatch {
    spec: PatchSpec,
    finalizes: bool,
}

struct Loader {
    handle: u32,
    directory: Arc<ScratchDir>,
}

impl Drop for Loader {
    fn drop(&mut self) {
        if let Err(error) = call(&Request::Close {
            handle: self.handle,
        }) {
            tracing::warn!(%error, "failed to close browser patch loader");
        }
    }
}

pub fn load_patches(
    jars: &[PathBuf],
    directory: Arc<ScratchDir>,
    bundle: &str,
) -> Result<Vec<Patch>> {
    let declarations = declarations(jars)?.into_values().flatten().collect();
    let loaded: Loaded = serde_json::from_slice(&call(&Request::Load {
        jars,
        bundle,
        declarations,
    })?)
    .map_err(json_error)?;
    let loader = Arc::new(Loader {
        handle: loaded.handle,
        directory,
    });
    Ok(loaded
        .patches
        .into_iter()
        .map(|LoadedPatch { spec, finalizes }| {
            let loader = Arc::clone(&loader);
            let reference = spec.reference();
            Patch::new(spec, finalizes, move |phase, context| {
                let guard = ContextGuard::enter(context, loader.directory.path().to_path_buf())?;
                let result = call(&Request::Invoke {
                    handle: loader.handle,
                    patch: &reference,
                    phase: match phase {
                        PatchPhase::Execute => Phase::Execute,
                        PatchPhase::Finalize => Phase::Finalize,
                    },
                });
                let native = guard.finish();
                result.and(native)
            })
        })
        .collect())
}

fn call(request: &impl Serialize) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(request).map_err(json_error)?;
    host_call(&bytes)
}

#[cfg(target_os = "wasi")]
fn host_call(bytes: &[u8]) -> Result<Vec<u8>> {
    #[derive(Deserialize)]
    struct Response {
        error: Option<String>,
        value: serde_json::Value,
    }

    #[repr(C)]
    struct Buffer {
        length: u32,
        width: u32,
    }
    #[link(wasm_import_module = "reseam_host")]
    unsafe extern "C" {
        fn call(ptr: *const u8, length: usize) -> *mut Buffer;
    }
    unsafe extern "C" {
        fn reseam_buffer_free(buffer: *mut Buffer);
    }
    // SAFETY: the browser host reads this slice synchronously and returns an
    // owned transport buffer, allocated by reseam_buffer_alloc in this module.
    let buffer = unsafe { call(bytes.as_ptr(), bytes.len()) };
    if buffer.is_null() {
        return Err(PatcherError::Bridge(
            "browser host returned no response".into(),
        ));
    }
    // SAFETY: the host's buffer has the transport header followed by length bytes.
    let response = unsafe {
        let length = (*buffer).length as usize;
        let value = std::slice::from_raw_parts(buffer.add(1).cast::<u8>(), length).to_vec();
        reseam_buffer_free(buffer);
        value
    };
    let response: Response = serde_json::from_slice(&response).map_err(json_error)?;
    if let Some(error) = response.error {
        return Err(PatcherError::Bridge(error));
    }
    serde_json::to_vec(&response.value).map_err(json_error)
}

#[cfg(not(target_os = "wasi"))]
fn host_call(_bytes: &[u8]) -> Result<Vec<u8>> {
    Err(PatcherError::Bridge(
        "browser patches require a WASI browser host".into(),
    ))
}

fn json_error(error: serde_json::Error) -> PatcherError {
    PatcherError::Bridge(format!("browser host protocol: {error}"))
}
