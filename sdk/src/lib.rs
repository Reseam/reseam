// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Host-facing API for inspecting APKs and bundles and running typed patch requests.

mod error;
mod inspect;
mod metrics;
mod output;
mod run;
mod trust;

pub use error::{HostError, Problem, SdkError, sdk_error};
pub use inspect::{ApkInspection, PreparedInspection, inspect, inspect_apk, load_bundles};
pub use reseam_apk::{ApplicationIcon, IconLayer};
pub use reseam_model::{
    ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchArtifact, PatchMetadata,
    PatchOutcome, PatchOutput, PatchRequest, RunEvent, SigningKeyFiles,
};
pub use reseam_model::{InstallMethod, OptionValue, PatchPreset, PatchSelection, Trust};

pub use metrics::{ApplyDiagnostics, PatchMetrics, PatchPhase, PatchPhaseMetrics};
#[cfg(target_os = "android")]
pub use reseam_patcher::kotlin::android_host::install_class_loader;
pub use run::{patch, patch_with_selection};
pub use trust::TrustStore;

#[cfg(test)]
mod tests;
