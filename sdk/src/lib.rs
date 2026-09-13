// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Host-facing API over the engine: inspect an APK and bundles, run a patch
//! request, and the JSON exports the Kotlin SDK binds to.

mod error;
mod inspect;
mod metrics;
mod output;
mod run;
mod trust;

pub use error::{sdk_error, Problem, SdkError};
pub use inspect::{inspect, inspect_apk, load_bundles, ApkInspection};
pub use reseam_apk::reseam_dex::estimated_ir_bytes;
pub use reseam_apk::{ApplicationIcon, IconLayer};
pub use reseam_model::{
    ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchArtifact, PatchMetadata,
    PatchOutcome, PatchOutput, PatchRequest, RunEvent, SigningKeyFiles,
};
pub use reseam_model::{OptionValue, PatchSelection, Trust};

pub use metrics::{
    trace_heap_growth, ApplyDiagnostics, CountingAllocator, PatchMetrics, PatchPhase,
    PatchPhaseMetrics,
};
#[cfg(target_os = "android")]
pub use reseam_patcher::kotlin::android_host::install_class_loader;
pub use run::patch;
pub use trust::TrustStore;

#[cfg(test)]
mod tests;
