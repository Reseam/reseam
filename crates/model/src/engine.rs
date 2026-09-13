// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::LogEntry;
use serde::{Deserialize, Serialize};
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
    Skipped { reason: String },
    Failed { reason: String },
}

#[derive(Debug, Clone)]
#[boltffi::data]
pub enum ProgressEvent {
    PatchStarted { patch: String },
    PatchLog(LogEntry),
    PatchFinished { patch: String, status: PatchStatus },
}
