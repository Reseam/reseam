// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `BoltFFI` surface of the application SDK. It lives one crate away from
//! the service because `BoltFFI` folds the exports of every direct dependency
//! into a binding root, and the patcher's exports belong to bundles, not apps.

#![expect(
    clippy::needless_pass_by_value,
    reason = "BoltFFI exports require owned values across the host boundary"
)]

use std::sync::{Mutex, MutexGuard};

use boltffi::export;
use reseam_model::{
    ApkMetadata, ApplicationIcon, InspectRequest, InspectResponse, PatchMetadata, PatchOutcome,
    PatchRequest, PatchSelection, RunEvent, SdkError,
};
use serde::{Serialize, de::DeserializeOwned};

/// Inspects APK and bundle metadata without running patches.
#[export]
pub fn inspect(request: InspectRequest) -> Result<InspectResponse, SdkError> {
    reseam_sdk::inspect(&request).map_err(SdkError::from)
}

/// Runs a patch request end to end. Progress is delivered on the calling
/// thread; the callback must return promptly and must not re-enter the engine.
#[export]
pub fn patch(
    request: PatchRequest,
    on_event: impl Fn(RunEvent) + Send + Sync + 'static,
) -> Result<PatchOutcome, SdkError> {
    reseam_sdk::patch(&request, on_event).map_err(SdkError::from)
}

/// Keeps inspected APK data and signed catalogs for one patch run. Input files
/// must remain unchanged until this object is closed or consumed by `patch`.
pub struct PreparedInspection {
    prepared: Mutex<Option<reseam_sdk::PreparedInspection>>,
}

#[export]
impl PreparedInspection {
    pub fn new(request: InspectRequest) -> Result<Self, SdkError> {
        Ok(Self {
            prepared: Mutex::new(Some(
                reseam_sdk::PreparedInspection::open(&request).map_err(SdkError::from)?,
            )),
        })
    }

    pub fn metadata(&self) -> Result<InspectResponse, SdkError> {
        Ok(self
            .prepared()?
            .as_ref()
            .ok_or_else(Self::consumed)?
            .metadata()
            .clone())
    }

    /// Consumes the preparation even if patching fails. Trust is rechecked using
    /// this request, and bundle payloads are verified before loading code.
    pub fn patch(
        &self,
        request: PatchRequest,
        on_event: impl Fn(RunEvent) + Send + Sync + 'static,
    ) -> Result<PatchOutcome, SdkError> {
        let prepared = self.prepared()?.take().ok_or_else(Self::consumed)?;
        prepared.patch(&request, on_event).map_err(SdkError::from)
    }
}

impl PreparedInspection {
    fn consumed() -> SdkError {
        SdkError {
            problem: reseam_model::Problem::Other,
            message: "Prepared inspection is consumed; open another inspection".into(),
        }
    }

    fn prepared(&self) -> Result<MutexGuard<'_, Option<reseam_sdk::PreparedInspection>>, SdkError> {
        self.prepared.lock().map_err(|_| SdkError {
            problem: reseam_model::Problem::Other,
            message: "Prepared inspection was interrupted by a panic; close and reopen it".into(),
        })
    }
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
            .map_err(SdkError::from)
    }

    pub fn metadata(&self) -> Result<ApkMetadata, SdkError> {
        self.opened()?.metadata().map_err(SdkError::from)
    }

    pub fn base_path(&self) -> Result<String, SdkError> {
        Ok(self.opened()?.base_path().display().to_string())
    }

    pub fn split_paths(&self) -> Result<Vec<String>, SdkError> {
        Ok(self
            .opened()?
            .split_paths()
            .map(|path| path.display().to_string())
            .collect())
    }

    /// A bitmap or adaptive icon, or none when the APK declares no usable icon.
    pub fn application_icon(&self) -> Result<Option<ApplicationIcon>, SdkError> {
        self.opened()?.application_icon().map_err(SdkError::from)
    }
}

impl ApkInspection {
    fn opened(&self) -> Result<MutexGuard<'_, reseam_sdk::ApkInspection>, SdkError> {
        self.opened.lock().map_err(|_| SdkError {
            problem: reseam_model::Problem::Other,
            message: "APK inspection was interrupted by a panic; close and reopen it".into(),
        })
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

fn to_json<T: Serialize>(value: &T) -> Result<String, SdkError> {
    serde_json::to_string(value).map_err(|error| reseam_sdk::sdk_error(&error))
}

fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, SdkError> {
    serde_json::from_str(json).map_err(|error| reseam_sdk::sdk_error(&error))
}

/// Android apps install their class loader before loading bundles; it becomes
/// the parent of every bundle loader and supplies the shared patch runtime.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_reseam_sdk_ReseamAndroidHost_setClassLoader(
    mut env: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    loader: jni::objects::JObject<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        if let Err(error) = reseam_sdk::install_class_loader(env, loader) {
            env.throw_new(
                jni::jni_str!("java/lang/IllegalStateException"),
                jni::strings::JNIString::new(error.to_string()),
            )?;
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[cfg(test)]
mod tests {
    use super::ApkInspection;
    use reseam_model::Problem;

    #[test]
    fn unreadable_inputs_report_typed_failures() {
        let tmp = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            ("absent.apk", None),
            ("plain.apk", Some(b"not a zip".as_slice())),
            ("truncated.apk", Some(b"PK\x03\x04".as_slice())),
        ] {
            let path = tmp.path().join(name);
            if let Some(bytes) = bytes {
                std::fs::write(&path, bytes).unwrap();
            }
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
}
