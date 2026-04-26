// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;

use reseam_patcher::engine::{self};
use reseam_patcher::options::{OptionType, OptionValue};
use serde::{Deserialize, Serialize};

use crate::metrics::PatchMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityMetadata {
    pub package_name: String,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionMetadata {
    pub key: String,
    pub title: String,
    pub description: String,
    pub option_type: OptionKind,
    pub default_value: Option<InputOptionValue>,
    pub valid_values: Option<Vec<String>>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchMetadata {
    pub source_bundle: String,
    pub name: String,
    pub description: String,
    pub enabled_by_default: bool,
    pub dependencies: Vec<String>,
    pub compatible_with: Vec<CompatibilityMetadata>,
    pub options: Vec<OptionMetadata>,
    pub is_compatible: bool,
    pub incompatibility_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetadata {
    pub file_name: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub extension_dex: Vec<String>,
    pub signer_public_key_hex: String,
    pub signer_fingerprint: String,
    pub trust_status: TrustStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResponse {
    pub apk: Option<ApkMetadata>,
    pub bundles: Vec<BundleMetadata>,
    pub patches: Vec<PatchMetadata>,
    pub requires_trust: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustStatus {
    Trusted,
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    keys: Vec<[u8; 32]>,
}

impl TrustStore {
    pub fn new<I>(keys: I) -> Self
    where
        I: IntoIterator<Item = [u8; 32]>,
    {
        let mut keys = keys.into_iter().collect::<Vec<_>>();
        keys.sort_unstable();
        keys.dedup();
        Self { keys }
    }

    pub fn keys(&self) -> &[[u8; 32]] {
        &self.keys
    }

    pub fn contains(&self, key: &[u8; 32]) -> bool {
        self.keys.binary_search(key).is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    String,
    Bool,
    Int,
    Float,
    StringList,
    Path,
}

impl From<&OptionType> for OptionKind {
    fn from(value: &OptionType) -> Self {
        match value {
            OptionType::String => Self::String,
            OptionType::Bool => Self::Bool,
            OptionType::Int => Self::Int,
            OptionType::Float => Self::Float,
            OptionType::StringList => Self::StringList,
            OptionType::Path => Self::Path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum InputOptionValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    StringList(Vec<String>),
    Path(String),
}

impl From<&OptionValue> for InputOptionValue {
    fn from(value: &OptionValue) -> Self {
        match value {
            OptionValue::String(value) => Self::String(value.clone()),
            OptionValue::Bool(value) => Self::Bool(*value),
            OptionValue::Int(value) => Self::Int(*value),
            OptionValue::Float(value) => Self::Float(*value),
            OptionValue::StringList(value) => Self::StringList(value.clone()),
            OptionValue::Path(value) => Self::Path(value.display().to_string()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchSelection {
    #[serde(default)]
    pub enable: Vec<String>,
    #[serde(default)]
    pub disable: Vec<String>,
    #[serde(default)]
    pub options: HashMap<String, HashMap<String, InputOptionValue>>,
}

#[derive(Debug, Clone)]
pub struct PatchRequest {
    pub apk_path: PathBuf,
    pub split_paths: Vec<PathBuf>,
    pub bundle_paths: Vec<PathBuf>,
    pub trust_store: TrustStore,
    pub selection: PatchSelection,
    pub output: PatchOutput,
    pub key_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub enum PatchOutput {
    SingleFile(PathBuf),
    SplitDir(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Apk,
    SplitDirectory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchArtifact {
    pub kind: ArtifactKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PatchOutcome {
    pub results: Vec<engine::PatchResult>,
    pub artifact: Option<PatchArtifact>,
    pub metrics: PatchMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    Info {
        message: String,
    },
    PatchStarted {
        patch: String,
    },
    PatchFinished {
        patch: String,
        status: PatchRunStatus,
        reason: Option<String>,
    },
    PatchLog {
        patch: String,
        level: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchRunStatus {
    Applied,
    Skipped,
    Failed,
}
