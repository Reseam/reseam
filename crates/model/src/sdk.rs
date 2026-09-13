// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    ContainerFormat, LogEntry, PatchMetrics, PatchResult, PatchSelection, PatchSpec, PatchStatus,
    Problem, ProgressEvent, Trust,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct ApkMetadata {
    #[boltffi::default(None)]
    pub application_label: Option<String>,
    #[boltffi::default(None)]
    pub package_name: Option<String>,
    #[boltffi::default(None)]
    pub version_name: Option<String>,
    #[boltffi::default(None)]
    pub version_code: Option<u32>,
    /// The container format the input came from, when it was an APKM/XAPK file.
    #[boltffi::default(None)]
    pub bundle_kind: Option<ContainerFormat>,
    pub dex_files: usize,
    pub component_count: usize,
    pub split_names: Vec<String>,
    pub class_count: usize,
    pub method_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
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
    #[boltffi::default(None)]
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct PatchMetadata {
    #[serde(flatten)]
    pub spec: PatchSpec,
    #[boltffi::default(None)]
    pub incompatibility: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[boltffi::data]
pub struct InspectRequest {
    #[serde(default)]
    #[boltffi::default(None)]
    pub apk_path: Option<String>,
    #[serde(default)]
    pub split_paths: Vec<String>,
    #[serde(default)]
    pub bundle_paths: Vec<String>,
    #[serde(default)]
    pub trust: Trust,
}

/// `patches` is empty while any bundle is untrusted: untrusted code is never
/// loaded, and loading is what reveals the patches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct InspectResponse {
    #[boltffi::default(None)]
    pub apk: Option<ApkMetadata>,
    pub bundles: Vec<BundleMetadata>,
    pub patches: Vec<PatchMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
#[boltffi::data]
pub struct PatchRequest {
    pub apk_path: String,
    #[serde(default)]
    pub split_paths: Vec<String>,
    pub bundle_paths: Vec<String>,
    #[serde(default)]
    pub trust: Trust,
    #[serde(default)]
    pub selection: PatchSelection,
    pub output: PatchOutput,
    /// Generated next to the output when absent.
    #[serde(default)]
    #[boltffi::default(None)]
    pub signing: Option<SigningKeyFiles>,
    #[serde(default)]
    #[boltffi::default(false)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[boltffi::data]
pub struct SigningKeyFiles {
    pub key: String,
    pub cert: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchOutput {
    /// Use `path` as a directory for splits, or append `.apk` for one component.
    Auto {
        path: String,
    },
    SingleFile {
        path: String,
    },
    SplitDir {
        path: String,
    },
}

/// The concrete output selected after opening the input, including for dry runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchArtifact {
    SingleFile { path: String },
    SplitDir { path: String },
}

impl PatchArtifact {
    pub fn path(&self) -> &Path {
        match self {
            Self::SingleFile { path } | Self::SplitDir { path } => Path::new(path),
        }
    }
}

impl PatchOutput {
    /// Requested destination. For automatic output, use the outcome for the final path.
    pub fn path(&self) -> &Path {
        match self {
            Self::Auto { path } | Self::SingleFile { path } | Self::SplitDir { path } => {
                Path::new(path)
            }
        }
    }

    pub fn resolve(&self, components: usize) -> anyhow::Result<PatchArtifact> {
        Ok(match self {
            Self::Auto { path } if components == 1 => {
                let mut name = path.clone();
                name.push_str(".apk");
                PatchArtifact::SingleFile { path: name }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct PatchOutcome {
    pub output: PatchArtifact,
    pub results: Vec<PatchResult>,
    pub metrics: PatchMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[boltffi::data]
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
        let path = String::from("chosen");
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
