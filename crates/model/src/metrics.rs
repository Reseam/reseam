// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[boltffi::data]
pub struct MaterializationStats {
    pub total_classes: u64,
    pub resolved_classes: u64,
    pub methods: u64,
    pub instructions: u64,
}

/// Full native heap attribution for a container, so RSS can be split into its
/// contributors rather than guessed at. All figures are lower bounds (they
/// exclude `Vec` capacity slack and allocator overhead).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[boltffi::data]
pub struct MemoryBreakdown {
    pub raw_buffer_bytes: u64,
    pub string_pool_bytes: u64,
    pub string_count: u64,
    pub id_table_bytes: u64,
    pub class_def_bytes: u64,
    pub materialized: MaterializationStats,
}

/// Java-heap usage of the in-process patch JVM. Part of this process's RSS, so
/// it must be subtracted to attribute memory to the native side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[boltffi::data]
pub struct JvmHeapStats {
    pub used_bytes: u64,
    pub committed_bytes: u64,
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchPhase {
    OpenApk,
    LoadBundles,
    ValidatePatches,
    ApplyPatches,
    WriteUnsignedArtifacts,
    LoadSigningKey,
    SignArtifacts,
}

impl PatchPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenApk => "open_apk",
            Self::LoadBundles => "load_bundles",
            Self::ValidatePatches => "validate_patches",
            Self::ApplyPatches => "apply_patches",
            Self::WriteUnsignedArtifacts => "write_unsigned_artifacts",
            Self::LoadSigningKey => "load_signing_key",
            Self::SignArtifacts => "sign_artifacts",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct PatchPhaseMetrics {
    pub phase: PatchPhase,
    pub duration_ms: u64,
    pub rss_bytes: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub heap_live_bytes: Option<u64>,
    /// Highest live heap during the phase, independent of what the allocator
    /// keeps cached afterwards.
    pub heap_peak_bytes: Option<u64>,
}

/// Sampled right after `apply_patches`, at the apply-phase memory peak, to
/// attribute RSS to materialized DEX IR vs the in-process JVM vs everything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct ApplyDiagnostics {
    pub rss_bytes: Option<u64>,
    pub dex: MemoryBreakdown,
    pub jvm: Option<JvmHeapStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[boltffi::data]
pub struct PatchMetrics {
    pub total_duration_ms: u64,
    pub final_rss_bytes: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub final_heap_live_bytes: Option<u64>,
    pub final_rss_anon_bytes: Option<u64>,
    pub final_rss_file_bytes: Option<u64>,
    pub phases: Vec<PatchPhaseMetrics>,
    pub apply_diagnostics: Option<ApplyDiagnostics>,
}
