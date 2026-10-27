// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Paths crossing the application boundary are UTF-8 strings; the engine validates
//! paths, signer keys, selections, and option values before executing patches.

mod compatibility;
mod container;
mod error;
mod icon;
mod inspect;
mod log;
mod metrics;
mod options;
mod outcome;
mod patch;
mod request;

pub use compatibility::{ApkVersion, Compatibility, CompatiblePackage, Incompatibility};
pub use container::ContainerFormat;
pub use error::{Problem, SdkError};
pub use icon::{ApplicationIcon, IconLayer};
pub use inspect::{ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata};
pub use log::{LogEntry, LogLevel};
pub use metrics::{
    ApplyDiagnostics, JvmHeapStats, MaterializationStats, MemoryBreakdown, PatchMetrics,
    PatchPhase, PatchPhaseMetrics,
};
pub use options::{OptionDeclaration, OptionType, OptionValue};
pub use outcome::{PatchArtifact, PatchOutcome, PatchResult, PatchStatus, RunEvent};
pub use patch::{PatchPreset, PatchRef, PatchSpec};
pub use request::{
    InstallMethod, PatchOutput, PatchRequest, PatchSelection, SigningKeyFiles, Trust,
};
