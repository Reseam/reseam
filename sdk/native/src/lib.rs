// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed application bindings. All request, response, event, and error data
//! comes from reseam-model; this crate owns only the foreign object lifetime.

use boltffi::export;
use reseam_model::{ApkMetadata, ApplicationIcon};
use reseam_model::{
    InspectRequest, InspectResponse, PatchMetadata, PatchOutcome, PatchRequest, PatchSelection,
    RunEvent, SdkError,
};

/// Inspects APK and bundle metadata without running patches.
#[export]
pub fn inspect(request: InspectRequest) -> Result<InspectResponse, SdkError> {
    reseam_sdk::inspect(&request).map_err(failure)
}

/// Runs a patch synchronously. Progress is delivered on the calling thread.
///
/// The caller chooses its executor. The callback must return promptly and must
/// not re-enter the patch engine while its current context is active.
#[export]
pub fn patch(
    request: PatchRequest,
    on_event: impl Fn(RunEvent) + Send + Sync + 'static,
) -> Result<PatchOutcome, SdkError> {
    reseam_sdk::patch(&request, on_event).map_err(failure)
}

/// Converts an error chain into the typed problem and its full diagnostic text.
fn failure(error: anyhow::Error) -> SdkError {
    reseam_sdk::sdk_error(&error)
}

/// An opened input. Extracted paths stay valid until this object is closed.
pub struct ApkInspection {
    opened: std::sync::Mutex<reseam_sdk::ApkInspection>,
}

#[export]
impl ApkInspection {
    /// Opens an APK or split container and retains extracted files until closed.
    pub fn new(apk_path: String, split_paths: Vec<String>) -> Result<Self, SdkError> {
        let splits = split_paths
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>();
        let opened = reseam_sdk::ApkInspection::open(std::path::Path::new(&apk_path), &splits)
            .map_err(failure)?;
        Ok(Self {
            opened: std::sync::Mutex::new(opened),
        })
    }

    /// Returns application and bytecode metadata.
    pub fn metadata(&self) -> Result<ApkMetadata, SdkError> {
        self.opened
            .lock()
            .map_err(|error| failure(anyhow::anyhow!("{error}")))?
            .metadata()
            .map_err(failure)
    }

    /// Returns the base APK path, valid while this inspection remains open.
    pub fn base_path(&self) -> Result<String, SdkError> {
        Ok(self
            .opened
            .lock()
            .map_err(|error| failure(anyhow::anyhow!("{error}")))?
            .base_path()
            .to_string_lossy()
            .into_owned())
    }

    /// Returns extracted split paths, valid while this inspection remains open.
    pub fn split_paths(&self) -> Result<Vec<String>, SdkError> {
        Ok(self
            .opened
            .lock()
            .map_err(|error| failure(anyhow::anyhow!("{error}")))?
            .split_paths()
            .map(|path| path.to_string_lossy().into_owned())
            .collect())
    }

    /// Returns a bitmap or adaptive icon, or none when the APK has no icon.
    pub fn application_icon(&self) -> Result<Option<ApplicationIcon>, SdkError> {
        Ok(self
            .opened
            .lock()
            .map_err(|error| failure(anyhow::anyhow!("{error}")))?
            .application_icon()
            .map_err(failure)?)
    }
}

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

    #[test]
    fn a_failed_open_reports_the_typed_problem() {
        let path = std::env::temp_dir().join("reseam-not-an-apk.jpg");
        std::fs::write(&path, b"not a zip").unwrap();
        let error = ApkInspection::new(path.display().to_string(), Vec::new())
            .err()
            .expect("a jpeg is not an APK");
        assert!(
            matches!(error.problem, reseam_model::Problem::UnreadableApk { path: failed } if failed == path.display().to_string())
        );
    }
}

/// Encodes navigation state using the SDK's serde schema, not its transient FFI ABI.
#[export]
pub fn encode_selection(selection: PatchSelection) -> Result<String, SdkError> {
    serde_json::to_string(&selection).map_err(|error| failure(error.into()))
}

/// Restores persisted selection data, rejecting invalid or incompatible input.
#[export]
pub fn decode_selection(json: String) -> Result<PatchSelection, SdkError> {
    serde_json::from_str(&json).map_err(|error| failure(error.into()))
}

/// Encodes patch metadata for persisted navigation state.
#[export]
pub fn encode_patch_metadata(patches: Vec<PatchMetadata>) -> Result<String, SdkError> {
    serde_json::to_string(&patches).map_err(|error| failure(error.into()))
}

/// Restores patch metadata from persisted navigation state.
#[export]
pub fn decode_patch_metadata(json: String) -> Result<Vec<PatchMetadata>, SdkError> {
    serde_json::from_str(&json).map_err(|error| failure(error.into()))
}
