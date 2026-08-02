// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod bundle;
pub mod context;
pub mod dependency;
pub mod engine;
pub mod error;
#[cfg(feature = "kotlin")]
pub mod kotlin;
pub mod log;
pub mod options;
pub mod patch;

pub use crate::patch::{Compatibility, CompatibilitySpec, Patch, PatchId, PatchSpec};
pub use reseam_apk;
pub use reseam_apk::reseam_dex;

/// Java-heap usage of the in-process patch JVM. Part of this process's RSS, so
/// it must be subtracted to attribute memory to the native side.
#[derive(Debug, Clone, Copy)]
pub struct JvmHeapStats {
    pub used_bytes: u64,
    pub committed_bytes: u64,
    pub max_bytes: u64,
}

/// Java-heap stats of the running patch JVM, or `None` if no JVM is live.
#[cfg(feature = "kotlin")]
pub fn jvm_heap_stats() -> Option<JvmHeapStats> {
    kotlin::jvm_heap_stats()
}

#[cfg(not(feature = "kotlin"))]
pub fn jvm_heap_stats() -> Option<JvmHeapStats> {
    None
}

pub mod prelude {
    pub use crate::context::PatchContext;
    pub use crate::engine::{ExecutionPlan, PatchSelection, ProgressEvent, ResolvedPatchPlan};
    pub use crate::error::{PatcherError, Result};
    pub use crate::log::{LogEntry, LogLevel, PatchLog};
    pub use crate::options::{OptionDeclaration, OptionKey, OptionType, OptionValue, PatchOptions};
    pub use crate::patch::{Compatibility, CompatibilitySpec, Patch, PatchId, PatchSpec};
    pub use reseam_patcher_macros::reseam_patch;
}

pub use reseam_patcher_macros::reseam_patch;
