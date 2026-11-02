// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use super::inspect::TargetResolution;
use crate::{Compatibility, PatchRef, PatchStatus, Trust};

/// Run every patch of the bundles that targets the APK's package, with
/// default options and version checks off, to see how each target resolves.
/// Nothing is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub apk_path: String,
    #[serde(default)]
    pub split_paths: Vec<String>,
    pub bundle_paths: Vec<String>,
    #[serde(default)]
    pub trust: Trust,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResolveReport {
    pub patches: Vec<PatchCheck>,
    /// Targets the run created that no patch tried to resolve: ones behind an
    /// option that is off, a branch that did not run, or a target that failed first.
    pub unreached: Vec<String>,
}

/// One patch's result and how each target it used resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PatchCheck {
    pub patch: PatchRef,
    pub name: String,
    /// The packages and versions the patch declares.
    pub compatibility: Compatibility,
    pub status: PatchStatus,
    pub resolutions: Vec<TargetResolution>,
}
