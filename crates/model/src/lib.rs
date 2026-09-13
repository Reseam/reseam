// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared application data. Rust and generated client bindings use these declarations.
//! Paths crossing the application boundary are UTF-8 strings; the engine validates
//! paths, signer keys, selections, and option values before executing patches.

mod container;
mod engine;
mod error;
mod log;
mod metrics;
mod options;
mod patch;
mod request;
mod sdk;

pub use container::ContainerFormat;
pub use engine::{PatchResult, PatchStatus, ProgressEvent};
pub use error::{Problem, SdkError};
pub use log::{LogEntry, LogLevel};
pub use metrics::{
    ApplyDiagnostics, JvmHeapStats, MaterializationStats, MemoryBreakdown, PatchMetrics,
    PatchPhase, PatchPhaseMetrics,
};
pub use options::{OptionDeclaration, OptionType, OptionValue};
pub use patch::{is_slug, Compatibility, CompatiblePackage, PatchSpec};
pub use request::{PatchSelection, Trust};
pub use sdk::{
    ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchArtifact, PatchMetadata,
    PatchOutcome, PatchOutput, PatchRequest, RunEvent, SigningKeyFiles,
};

mod icon;
pub use icon::{ApplicationIcon, IconLayer};
