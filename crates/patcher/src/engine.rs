// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resolving which patches run and running them in dependency order.

mod plan;
mod run;

use serde::Serialize;

use crate::log::LogEntry;

pub use plan::PatchSelection;
pub(crate) use plan::ResolvedPlan;
pub use run::{apply_patches, validate_patches};

#[derive(Debug, Clone, Serialize)]
pub struct PatchResult {
    pub name: String,
    pub status: PatchStatus,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatchStatus {
    Applied,
    Skipped { reason: String },
    Failed { reason: String },
}

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    PatchStarted { patch: String },
    PatchLog(LogEntry),
    PatchFinished { patch: String, status: PatchStatus },
}
