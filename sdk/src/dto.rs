// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use reseam_patcher::engine::{PatchResult, PatchSelection, PatchStatus, ProgressEvent};
use reseam_patcher::log::LogEntry;
use reseam_patcher::PatchSpec;
use serde::{Deserialize, Serialize};

use crate::metrics::PatchMetrics;
use crate::trust::TrustStore;

#[derive(Debug, Clone, Serialize)]
pub struct ApkMetadata {
    pub package_name: Option<String>,
    pub version_name: Option<String>,
    pub version_code: Option<u32>,
    pub dex_files: usize,
    pub component_count: usize,
    pub split_names: Vec<String>,
    pub class_count: usize,
    pub method_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BundleMetadata {
    pub file_name: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub files: Vec<String>,
    pub public_key: String,
    pub engine: String,
    pub trusted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchMetadata {
    pub bundle: String,
    #[serde(flatten)]
    pub spec: PatchSpec,
    pub incompatibility: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InspectRequest {
    #[serde(default)]
    pub apk_path: Option<PathBuf>,
    #[serde(default)]
    pub split_paths: Vec<PathBuf>,
    #[serde(default)]
    pub bundle_paths: Vec<PathBuf>,
    #[serde(default)]
    pub trust: TrustStore,
}

/// `patches` is empty while any bundle is untrusted: untrusted code is never
/// loaded, and loading is what reveals the patches.
#[derive(Debug, Clone, Serialize)]
pub struct InspectResponse {
    pub apk: Option<ApkMetadata>,
    pub bundles: Vec<BundleMetadata>,
    pub patches: Vec<PatchMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchRequest {
    pub apk_path: PathBuf,
    #[serde(default)]
    pub split_paths: Vec<PathBuf>,
    pub bundle_paths: Vec<PathBuf>,
    #[serde(default)]
    pub trust: TrustStore,
    #[serde(default)]
    pub selection: PatchSelection,
    pub output: PatchOutput,
    /// Generated next to the output when absent.
    #[serde(default)]
    pub signing: Option<SigningKeyFiles>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SigningKeyFiles {
    pub key: PathBuf,
    pub cert: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatchOutput {
    SingleFile { path: PathBuf },
    SplitDir { path: PathBuf },
}

impl PatchOutput {
    pub fn path(&self) -> &Path {
        match self {
            Self::SingleFile { path } | Self::SplitDir { path } => path,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchOutcome {
    pub results: Vec<PatchResult>,
    pub metrics: PatchMetrics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    Info { message: String },
    PatchStarted { patch: String },
    PatchLog(LogEntry),
    PatchFinished { patch: String, status: PatchStatus },
}

impl From<ProgressEvent> for RunEvent {
    fn from(event: ProgressEvent) -> Self {
        match event {
            ProgressEvent::PatchStarted { patch } => Self::PatchStarted { patch },
            ProgressEvent::PatchLog(entry) => Self::PatchLog(entry),
            ProgressEvent::PatchFinished { patch, status } => Self::PatchFinished { patch, status },
        }
    }
}
