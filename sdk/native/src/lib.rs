// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The BoltFFI surface of the application SDK. It lives one crate away from
//! the service because BoltFFI folds the exports of every direct dependency
//! into a binding root, and the patcher's exports belong to bundles, not apps.

use std::sync::{Mutex, MutexGuard, PoisonError};

use boltffi::export;
use reseam_model::{
    ApkMetadata, ApplicationIcon, InspectRequest, InspectResponse, PatchMetadata, PatchOutcome,
    PatchRequest, PatchSelection, RunEvent, SdkError,
};
use serde::{de::DeserializeOwned, Serialize};

/// Inspects APK and bundle metadata without running patches.
#[export]
pub fn inspect(request: InspectRequest) -> Result<InspectResponse, SdkError> {
    reseam_sdk::inspect(&request).map_err(failure)
}

/// Runs a patch request end to end. Progress is delivered on the calling
/// thread; the callback must return promptly and must not re-enter the engine.
#[export]
pub fn patch(
    request: PatchRequest,
    on_event: impl Fn(RunEvent) + Send + Sync + 'static,
) -> Result<PatchOutcome, SdkError> {
    reseam_sdk::patch(&request, on_event).map_err(failure)
}

/// An opened APK or container. Extracted component files stay valid until
/// the inspection is closed.
pub struct ApkInspection {
    opened: Mutex<reseam_sdk::ApkInspection>,
}

#[export]
impl ApkInspection {
    /// Opens an APK or container, reporting unreadable input as a typed problem.
    pub fn new(apk_path: String, split_paths: Vec<String>) -> Result<Self, SdkError> {
        let splits = split_paths
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>();
        reseam_sdk::ApkInspection::open(std::path::Path::new(&apk_path), &splits)
            .map(|opened| Self {
                opened: Mutex::new(opened),
            })
            .map_err(failure)
    }

    /// Reads application and bytecode metadata.
    pub fn metadata(&self) -> Result<ApkMetadata, SdkError> {
        self.opened().metadata().map_err(failure)
    }

    /// The base component path.
    pub fn base_path(&self) -> String {
        self.opened().base_path().display().to_string()
    }

    /// Additional component paths in their original order.
    pub fn split_paths(&self) -> Vec<String> {
        self.opened()
            .split_paths()
            .map(|path| path.display().to_string())
            .collect()
    }

    /// A bitmap or adaptive icon, or none when the APK declares no usable icon.
    pub fn application_icon(&self) -> Result<Option<ApplicationIcon>, SdkError> {
        self.opened().application_icon().map_err(failure)
    }
}

impl ApkInspection {
    fn opened(&self) -> MutexGuard<'_, reseam_sdk::ApkInspection> {
        self.opened.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Encodes a selection for persistence in the SDK's serde schema.
#[export]
pub fn encode_selection(selection: PatchSelection) -> Result<String, SdkError> {
    to_json(&selection)
}

/// Restores a persisted selection, rejecting incompatible input.
#[export]
pub fn decode_selection(json: String) -> Result<PatchSelection, SdkError> {
    from_json(&json)
}

/// Encodes patch metadata for persistence in the SDK's serde schema.
#[export]
pub fn encode_patch_metadata(patches: Vec<PatchMetadata>) -> Result<String, SdkError> {
    to_json(&patches)
}

/// Restores persisted patch metadata, rejecting incompatible input.
#[export]
pub fn decode_patch_metadata(json: String) -> Result<Vec<PatchMetadata>, SdkError> {
    from_json(&json)
}

fn failure(error: anyhow::Error) -> SdkError {
    reseam_sdk::sdk_error(&error)
}

fn to_json<T: Serialize>(value: &T) -> Result<String, SdkError> {
    serde_json::to_string(value).map_err(|error| failure(error.into()))
}

fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, SdkError> {
    serde_json::from_str(json).map_err(|error| failure(error.into()))
}

/// Android apps install their class loader before loading bundles; it becomes
/// the parent of every bundle loader and supplies the shared patch runtime.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_app_reseam_sdk_ReseamAndroidHost_setClassLoader(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    loader: jni::objects::JObject<'_>,
) {
    if let Err(error) = reseam_sdk::install_class_loader(&mut env, loader) {
        let _ = env.throw_new("java/lang/IllegalStateException", error);
    }
}

#[cfg(test)]
mod tests {
    use super::ApkInspection;
    use reseam_model::Problem;

    #[test]
    fn a_failed_open_reports_the_typed_problem() {
        let path = std::env::temp_dir().join("reseam-not-an-apk.jpg");
        std::fs::write(&path, b"not a zip").unwrap();
        let error = ApkInspection::new(path.display().to_string(), Vec::new())
            .err()
            .unwrap();
        assert_eq!(
            error.problem,
            Problem::UnreadableApk {
                path: path.display().to_string()
            }
        );
    }
}
