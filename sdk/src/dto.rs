// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use reseam_apk::ContainerFormat;
use reseam_patcher::engine::{PatchResult, PatchSelection, PatchStatus, ProgressEvent};
use reseam_patcher::log::LogEntry;
use reseam_patcher::PatchSpec;
use serde::{Deserialize, Serialize};

use crate::error::Problem;

use crate::metrics::PatchMetrics;
use crate::trust::TrustStore;

#[derive(Debug, Clone, Serialize)]
pub struct ApkMetadata {
    pub application_label: Option<String>,
    pub package_name: Option<String>,
    pub version_name: Option<String>,
    pub version_code: Option<u32>,
    /// The container format the input came from, when it was an APKM/XAPK file.
    pub bundle_kind: Option<ContainerFormat>,
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
    /// Set when the bundle cannot be used; its patches are then absent from the response.
    pub problem: Option<Problem>,
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
    /// Use `path` as a directory for splits, or append `.apk` for one component.
    Auto {
        path: PathBuf,
    },
    SingleFile {
        path: PathBuf,
    },
    SplitDir {
        path: PathBuf,
    },
}

/// The concrete output selected after opening the input, including for dry runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatchArtifact {
    SingleFile { path: PathBuf },
    SplitDir { path: PathBuf },
}

impl PatchArtifact {
    pub fn path(&self) -> &Path {
        match self {
            Self::SingleFile { path } | Self::SplitDir { path } => path,
        }
    }
}

impl PatchOutput {
    /// Requested destination. For automatic output, use the outcome for the final path.
    pub fn path(&self) -> &Path {
        match self {
            Self::Auto { path } | Self::SingleFile { path } | Self::SplitDir { path } => path,
        }
    }

    pub(crate) fn resolve(&self, components: usize) -> anyhow::Result<PatchArtifact> {
        Ok(match self {
            Self::Auto { path } if components == 1 => {
                let mut name = path.as_os_str().to_os_string();
                name.push(".apk");
                PatchArtifact::SingleFile { path: name.into() }
            }
            Self::Auto { path } | Self::SplitDir { path } => {
                PatchArtifact::SplitDir { path: path.clone() }
            }
            Self::SingleFile { path } => {
                anyhow::ensure!(components == 1, "input has splits; use a split directory (--output-dir) instead of single-file output");
                PatchArtifact::SingleFile { path: path.clone() }
            }
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchOutcome {
    pub output: PatchArtifact,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_output_depends_on_components_and_preserves_dotted_names() {
        let output: PatchOutput =
            serde_json::from_str(r#"{"kind":"auto","path":"out/com.example.app.reseamed"}"#)
                .unwrap();
        assert_eq!(
            output.resolve(1).unwrap(),
            PatchArtifact::SingleFile {
                path: "out/com.example.app.reseamed.apk".into()
            }
        );
        assert_eq!(
            output.resolve(2).unwrap(),
            PatchArtifact::SplitDir {
                path: "out/com.example.app.reseamed".into()
            }
        );
    }

    #[test]
    fn explicit_outputs_are_honored_or_rejected_never_redirected() {
        let path = PathBuf::from("chosen");
        let directory = PatchOutput::SplitDir { path: path.clone() };
        for count in [1, 2] {
            assert_eq!(
                directory.resolve(count).unwrap(),
                PatchArtifact::SplitDir { path: path.clone() }
            );
        }
        let file = PatchOutput::SingleFile { path: path.clone() };
        assert_eq!(file.resolve(1).unwrap(), PatchArtifact::SingleFile { path });
        assert!(file
            .resolve(2)
            .unwrap_err()
            .to_string()
            .contains("--output-dir"));
    }
}
