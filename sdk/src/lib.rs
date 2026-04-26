// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod dto;
mod inspect;
mod metrics;
mod selection;
mod service;

pub use dto::{
    ApkMetadata, ArtifactKind, BundleMetadata, CompatibilityMetadata, InputOptionValue,
    InspectResponse, OptionKind, OptionMetadata, PatchArtifact, PatchMetadata, PatchOutcome,
    PatchOutput, PatchRequest, PatchRunStatus, PatchSelection, RunEvent, TrustStatus, TrustStore,
};
pub use inspect::{built_in_trust_store, inspect_apk, inspect_with_trust, load_bundle_with_trust};
pub use metrics::{PatchExecutionReport, PatchMetrics, PatchPhase, PatchPhaseMetrics};
pub use selection::{
    build_execution_plan, compile_patch_selection, parse_cli_option, selection_from_cli,
};
pub use service::{measure_patch, patch};

include!("ffi.rs");
