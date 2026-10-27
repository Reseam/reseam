// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use reseam_journal::Savepoint;
use reseam_storage::ScratchDir;
use serde::{Deserialize, Serialize};

use crate::bridge::phase::{ContextGuard, LoadedBundle};
use crate::bridge::savepoints::SavepointAction;
use crate::bundle::BundleInfo;
use crate::bundle::index::Declaration;
use crate::error::{PatcherError, Result};
use crate::{Patch, PatchPhase, PatchSpec};

#[derive(Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Request<'a> {
    Load {
        jars: &'a [PathBuf],
        bundle: &'a str,
        declarations: Vec<&'a Declaration>,
    },
    Invoke {
        handle: u32,
        patch: &'a str,
        phase: Phase,
    },
    Close {
        handle: u32,
    },
    Savepoint {
        handle: u32,
        action: SavepointAction,
        savepoint: u32,
    },
}

thread_local! {
    /// The loaders open in this host, each with its own patch runtime.
    static LOADERS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

/// Tells every loader's patch runtime about `savepoint`.
pub(in crate::bridge) fn savepoint(action: SavepointAction, savepoint: Savepoint) -> Result<()> {
    for handle in LOADERS.with(|loaders| loaders.borrow().clone()) {
        call(&Request::Savepoint {
            handle,
            action,
            savepoint: savepoint.id(),
        })?;
    }
    Ok(())
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
    bundle: LoadedBundle,
}

impl Drop for Loader {
    fn drop(&mut self) {
        LOADERS.with(|loaders| loaders.borrow_mut().retain(|&handle| handle != self.handle));
        if let Err(error) = call(&Request::Close {
            handle: self.handle,
        }) {
            tracing::warn!(%error, "failed to close browser patch loader");
        }
    }
}

pub(crate) fn load_patches(
    jars: &[PathBuf],
    declarations: &BTreeMap<String, Vec<Declaration>>,
    directory: Arc<ScratchDir>,
    bundle: &BundleInfo,
) -> Result<Vec<Patch>> {
    if declarations.is_empty() {
        return Ok(Vec::new());
    }
    let loaded: Loaded = serde_json::from_slice(&call(&Request::Load {
        jars,
        bundle: &bundle.name,
        declarations: declarations.values().flatten().collect(),
    })?)
    .map_err(json_error)?;
    LOADERS.with(|loaders| loaders.borrow_mut().push(loaded.handle));
    let loader = Arc::new(Loader {
        handle: loaded.handle,
        bundle: LoadedBundle::new(bundle, directory),
    });
    Ok(loaded
        .patches
        .into_iter()
        .map(|LoadedPatch { spec, finalizes }| {
            let loader = Arc::clone(&loader);
            let reference = spec.reference.to_string();
            Patch::new(spec, finalizes, move |phase| {
                let (phase, context) = match phase {
                    PatchPhase::Execute(context) => (Phase::Execute, context),
                    PatchPhase::Finalize(context) => (Phase::Finalize, context),
                    PatchPhase::RewriteOutput(_) => {
                        return Err(PatcherError::Bridge(format!(
                            "{reference} rewrites the written APK, which the browser patcher cannot run"
                        )));
                    }
                };
                let guard = ContextGuard::enter(context, loader.bundle.running())?;
                let result = call(&Request::Invoke {
                    handle: loader.handle,
                    patch: &reference,
                    phase,
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
