// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{LogEntry, PatchMetrics, PatchRef};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchOutcome {
    pub output: PatchArtifact,
    pub results: Vec<PatchResult>,
    pub metrics: PatchMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
#[boltffi::data]
pub enum RunEvent {
    Info {
        message: String,
    },
    PatchStarted {
        patch: PatchRef,
    },
    PatchLog(LogEntry),
    PatchFinished {
        patch: PatchRef,
        status: PatchStatus,
    },
    /// Finished patches whose results no longer hold: a patch that ran before
    /// them could not apply, so the run went back to before it. They run again.
    PatchesReverted {
        patches: Vec<PatchRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchResult {
    pub patch: PatchRef,
    /// Internal patches run as dependencies and are never listed to users.
    pub hidden: bool,
    /// The running patches that pulled this one in, empty when it was asked
    /// for directly.
    pub required_by: Vec<PatchRef>,
    pub status: PatchStatus,
    pub logs: Vec<LogEntry>,
}

impl PatchResult {
    /// Work the user asked for, rather than a dependency that came with it.
    pub fn chosen(&self) -> bool {
        !self.hidden && self.required_by.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[boltffi::data]
pub enum PatchStatus {
    Applied,
    Skipped {
        reason: String,
    },
    /// Left out of a mount build because the patch changes the manifest, which the run
    /// undid, or rewrites the written APK.
    Unmountable {
        reason: String,
    },
    Failed {
        reason: String,
    },
}
