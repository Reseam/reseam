// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Host-facing API over the engine: inspect an APK and bundles, run a patch
//! request, and the JSON exports the Kotlin SDK binds to.

mod dto;
mod ffi;
mod inspect;
mod metrics;
mod output;
mod run;
mod trust;

pub use dto::{
    ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata, PatchOutcome,
    PatchOutput, PatchRequest, RunEvent, SigningKeyFiles,
};
pub use inspect::{inspect, inspect_apk, load_bundles};
pub use metrics::{
    trace_heap_growth, ApplyDiagnostics, CountingAllocator, PatchMetrics, PatchPhase,
    PatchPhaseMetrics,
};
pub use run::patch;
pub use trust::TrustStore;
