// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{LogEntry, PatchMetrics};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    Info {
        message: String,
    },
    PatchStarted {
        patch: String,
    },
    PatchLog(LogEntry),
    PatchFinished {
        patch: String,
        status: PatchStatus,
    },
    /// Finished patches whose results no longer hold: a patch that ran before
    /// them could not apply, so the run went back to before it. They run again.
    PatchesReverted {
        patches: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[boltffi::data]
pub struct PatchResult {
    /// `<bundle>/<id>`.
    pub patch: String,
    /// Internal patches run as dependencies and are never listed to users.
    pub hidden: bool,
    /// References of the running patches that pulled this one in, empty when
    /// it was asked for directly.
    pub required_by: Vec<String>,
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
