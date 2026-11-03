// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::{AppIdentity, InstallMethod, PatchMetadata, PatchRef, PatchSelection};

/// What a build was made from, which tells a host later whether patching the
/// same app again would change it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct BuildRecord {
    /// The app the build patched.
    pub app: AppIdentity,
    /// The selection as the request gave it.
    pub selection: PatchSelection,
    pub install_method: InstallMethod,
    /// Every patch the run took up, applied or not, dependencies included,
    /// so a fix to one that failed counts as a change.
    pub patches: Vec<RecordedPatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct RecordedPatch {
    pub patch: PatchRef,
    /// The patch's fingerprint when the build ran it.
    pub fingerprint: String,
}

/// Compares a build with the patches available now.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct ChangesRequest {
    pub record: BuildRecord,
    /// The patches an inspection lists now.
    pub patches: Vec<PatchMetadata>,
}

/// How patching the app of a build again, with its selection, would differ
/// from the build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchChange {
    /// A patch the build took up whose code or files changed since.
    Changed { patch: PatchRef },
    /// A patch the build lacks that patching again would take up.
    Added { patch: PatchRef },
    /// A patch the build took up that patching again would not.
    Removed { patch: PatchRef },
}
