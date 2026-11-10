// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Inspecting apps and bundles, and patching.

use std::sync::{Mutex, MutexGuard};

use boltffi::export;
use reseam_model::{
    ApkMetadata, ApplicationIcon, InspectRequest, InspectResponse, PatchOutcome, PatchRequest,
    PatchSpec, PatchText, RunEvent, SdkError,
};

/// Inspects APK and bundle metadata without running patches.
#[export]
pub fn inspect(request: InspectRequest) -> Result<InspectResponse, SdkError> {
    reseam_sdk::inspect(&request).map_err(SdkError::from)
}

/// What users who read `locales`, BCP 47 tags most preferred first, see of
/// `spec`. Each field falls back as [`PatchSpec::text`] describes.
#[export]
pub fn patch_text(spec: PatchSpec, locales: Vec<String>) -> PatchText {
    spec.text(&locales)
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
        SdkError::other("Prepared inspection is consumed; open another inspection")
    }

    fn prepared(&self) -> Result<MutexGuard<'_, Option<reseam_sdk::PreparedInspection>>, SdkError> {
        self.prepared.lock().map_err(|_| {
            SdkError::other("Prepared inspection was interrupted by a panic; close and reopen it")
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
            .splits()
            .map(|(_, path)| path.display().to_string())
            .collect())
    }

    /// A bitmap or adaptive icon, or none when the APK declares no usable icon.
    pub fn application_icon(&self) -> Result<Option<ApplicationIcon>, SdkError> {
        self.opened()?.application_icon().map_err(SdkError::from)
    }
}

impl ApkInspection {
    fn opened(&self) -> Result<MutexGuard<'_, reseam_sdk::ApkInspection>, SdkError> {
        self.opened.lock().map_err(|_| {
            SdkError::other("APK inspection was interrupted by a panic; close and reopen it")
        })
    }
}
