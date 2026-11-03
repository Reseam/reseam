// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{ApkVersion, ContainerFormat, Incompatibility, PatchPreset, PatchSpec, Problem, Trust};
use serde::{Deserialize, Serialize};

/// What an app declares itself as in its manifest. It is known before the
/// APK is, from a store listing, an installed app, or an earlier build, which
/// is enough to check patches against.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct AppIdentity {
    /// Absent when the manifest names no package; only universal patches
    /// apply to such an app.
    #[serde(default)]
    #[boltffi::default(None)]
    pub package_name: Option<String>,
    #[serde(default)]
    #[boltffi::default(None)]
    pub version_name: Option<String>,
    #[serde(default)]
    #[boltffi::default(None)]
    pub version_code: Option<i32>,
}

impl AppIdentity {
    pub fn version(&self) -> ApkVersion<'_> {
        ApkVersion {
            name: self.version_name.as_deref(),
            code: self.version_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct ApkMetadata {
    #[boltffi::default(None)]
    pub application_label: Option<String>,
    #[serde(flatten)]
    pub identity: AppIdentity,
    /// The container format the input came from, when it was an APKM, XAPK or
    /// APKS file.
    #[boltffi::default(None)]
    pub bundle_kind: Option<ContainerFormat>,
    pub dex_files: usize,
    pub component_count: usize,
    pub split_names: Vec<String>,
    pub class_count: usize,
    pub method_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct BundleMetadata {
    pub file_name: String,
    pub name: String,
    pub author: String,
    pub description: String,
    /// Empty when the manifest names none.
    pub license: String,
    pub files: Vec<String>,
    pub public_key: String,
    /// The release the bundle is; empty for a development build.
    pub version: String,
    pub engine: String,
    /// Whether the host approves this signer for execution; inspection does not require it.
    pub trusted: bool,
    /// Set when the signed catalog cannot be read; its patches are then absent.
    #[boltffi::default(None)]
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchMetadata {
    #[serde(flatten)]
    pub spec: PatchSpec,
    #[boltffi::default(None)]
    pub incompatibility: Option<Incompatibility>,
    /// The presets that select this patch for the inspected APK.
    #[serde(default)]
    pub presets: Vec<PatchPreset>,
    /// Hex hash of the code and bundle files the patch can reach. A patch keeps it
    /// across bundle releases that change nothing it runs.
    pub fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct InspectRequest {
    #[serde(default)]
    #[boltffi::default(None)]
    pub apk_path: Option<String>,
    #[serde(default)]
    pub split_paths: Vec<String>,
    #[serde(default)]
    pub bundle_paths: Vec<String>,
    /// Marks approved signers in the response without gating catalog inspection.
    #[serde(default)]
    pub trust: Trust,
}

/// Patches come from signed catalogs without loading code or requiring trust.
/// Problems with unreadable bundles are reported in `bundles`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct InspectResponse {
    #[boltffi::default(None)]
    pub apk: Option<ApkMetadata>,
    pub bundles: Vec<BundleMetadata>,
    pub patches: Vec<PatchMetadata>,
}
