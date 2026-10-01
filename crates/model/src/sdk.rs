// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    ContainerFormat, LogEntry, PatchMetrics, PatchPreset, PatchResult, PatchSelection, PatchSpec,
    PatchStatus, Problem, ProgressEvent, Trust,
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
    /// The presets that select this patch for the inspected APK.
    #[serde(default)]
    pub presets: Vec<PatchPreset>,
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

/// Patches are listed only for bundles that were trusted and loaded. Problems
/// with other bundles are reported in `bundles`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct InspectResponse {
    #[boltffi::default(None)]
    pub apk: Option<ApkMetadata>,
    pub bundles: Vec<BundleMetadata>,
    pub patches: Vec<PatchMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// When absent, reuse or generate a pair next to the output.
    /// Supplied paths also generate on first use when both files are missing.
    /// Exactly one existing file is an error; existing credentials are never replaced.
    #[serde(default)]
    #[boltffi::default(None)]
    pub signing: Option<SigningKeyFiles>,
    #[serde(default)]
    #[boltffi::default(false)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Resolves automatic output using the input component count.
    /// Explicit file output requires exactly one component; directories are preserved.
    pub fn resolve(&self, components: usize) -> crate::Result<PatchArtifact> {
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
                if components != 1 {
                    return Err(Problem::SingleFileComponents { components });
                }
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
            ProgressEvent::Started { patch } => Self::PatchStarted { patch },
            ProgressEvent::Log(entry) => Self::PatchLog(entry),
            ProgressEvent::Finished { patch, status } => Self::PatchFinished { patch, status },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_selection_honors_component_counts_and_explicit_destinations() {
        let path = "out/com.example.app.reseamed";
        let file = |path: &str| Some(PatchArtifact::SingleFile { path: path.into() });
        let directory = || Some(PatchArtifact::SplitDir { path: path.into() });
        for (kind, expected) in [
            ("auto", [file(&format!("{path}.apk")), directory()]),
            ("split_dir", [directory(), directory()]),
            ("single_file", [file(path), None]),
        ] {
            let output: PatchOutput =
                serde_json::from_value(serde_json::json!({"kind": kind, "path": path})).unwrap();
            for (count, expected) in [1, 2].into_iter().zip(expected) {
                let actual = output.resolve(count);
                if let Some(expected) = expected {
                    assert_eq!(actual.unwrap(), expected);
                } else {
                    assert!(matches!(actual, Err(Problem::SingleFileComponents { .. })));
                }
            }
        }
    }
}
