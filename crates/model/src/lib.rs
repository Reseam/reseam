// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Paths crossing the application boundary are UTF-8 strings; the engine validates
//! paths, signer keys, selections, and option values before executing patches.

pub mod analysis;
mod compatibility;
mod container;
mod error;
mod icon;
mod inspect;
mod log;
mod metrics;
mod mount;
mod options;
mod outcome;
mod patch;
mod plan;
mod record;
mod release;
mod request;
mod signing;

pub use compatibility::{ApkVersion, Compatibility, CompatiblePackage, Incompatibility};
pub use container::ContainerFormat;
pub use error::{Problem, SdkError};
pub use icon::{ApplicationIcon, IconLayer};
pub use inspect::{
    ApkMetadata, AppIdentity, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata,
};
pub use log::{LogEntry, LogLevel};
pub use metrics::{
    ApplyDiagnostics, JvmHeapStats, MaterializationStats, MemoryBreakdown, PatchMetrics,
    PatchPhase, PatchPhaseMetrics,
};
pub use mount::{
    InstalledApp, InstalledSplit, MountNamespace, MountRequest, PatchedSplit, RootScript,
};
pub use options::{OptionDeclaration, OptionProblem, OptionType, OptionValue, ParsedOption};
pub use outcome::{PatchArtifact, PatchOutcome, PatchResult, PatchStatus, RunEvent};
pub use patch::{PatchPreset, PatchRef, PatchSpec};
pub use plan::{OptionIssue, PlanRequest, PlannedPatch, PlannedRun, SelectionPlan};
pub use record::{BuildRecord, ChangesRequest, PatchChange, RecordedPatch};
pub use release::{PublishedRelease, Publisher, ReleaseChannel, ReleaseIndex};
pub use request::{
    InstallMethod, PatchOutput, PatchRequest, PatchSelection, SigningIdentity, Trust,
};
pub use signing::SigningKeyMaterial;
