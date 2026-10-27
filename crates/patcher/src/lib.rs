// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod app_runtime;
#[cfg(feature = "bridge")]
pub mod bridge;
pub mod bundle;
pub mod catalog;
pub mod context;
pub mod engine;
pub mod error;
pub mod log;
pub mod options;
pub mod output;
pub mod patch;

pub use crate::patch::{
    ApkVersion, Compatibility, CompatiblePackage, Patch, PatchPhase, PatchPreset, PatchRef,
    PatchSpec,
};
pub use reseam_apk;

pub use reseam_model::JvmHeapStats;

/// Java-heap stats of the running patch JVM, or `None` if no JVM is live.
pub fn jvm_heap_stats() -> Option<JvmHeapStats> {
    #[cfg(feature = "kotlin")]
    {
        bridge::runtime::jvm::heap_stats()
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
    bridge::runtime::jvm::collect_garbage();
}

#[cfg(test)]
mod test_support;
