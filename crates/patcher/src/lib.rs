// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod bundle;
pub mod context;
pub mod engine;
pub mod error;
#[cfg(feature = "kotlin")]
pub mod kotlin;
pub mod log;
pub mod options;
pub mod patch;

use serde::{Deserialize, Serialize};

pub use crate::patch::{Compatibility, Patch, PatchSpec};
pub use reseam_apk;
pub use reseam_apk::reseam_dex;

/// Java-heap usage of the in-process patch JVM. Part of this process's RSS, so
/// it must be subtracted to attribute memory to the native side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JvmHeapStats {
    pub used_bytes: u64,
    pub committed_bytes: u64,
    pub max_bytes: u64,
}

/// Java-heap stats of the running patch JVM, or `None` if no JVM is live.
pub fn jvm_heap_stats() -> Option<JvmHeapStats> {
    #[cfg(feature = "kotlin")]
    {
        kotlin::jvm::heap_stats()
    }
    #[cfg(not(feature = "kotlin"))]
    {
        None
    }
}

/// Releases what the patch runtime accumulated over a run: a full collection
/// unloads the run's class loader and lets the heap shrink back.
pub fn release_runtime_memory() {
    #[cfg(feature = "kotlin")]
    kotlin::jvm::collect_garbage();
}
