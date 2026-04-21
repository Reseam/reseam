// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use crate::log::LogEntry;
use crate::options::PatchOptions;
use crate::patch::PatchId;

mod execute;
mod selection;

pub use execute::{
    apply_patches, apply_patches_with_options, apply_patches_with_plan,
    apply_patches_with_plan_and_observer, apply_patches_with_selection,
    apply_patches_with_selection_and_observer, validate_patches_with_plan,
    validate_patches_with_selection,
};
pub use selection::resolve_patch_selection;

#[derive(Debug, Clone)]
pub struct PatchResult {
    pub name: String,
    pub status: PatchStatus,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default)]
pub struct PatchSelection {
    selected: HashSet<PatchId>,
    disabled: HashSet<PatchId>,
    options: HashMap<PatchId, PatchOptions>,
}

pub type ExecutionPlan = PatchSelection;

impl PatchSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_patch(&mut self, patch: impl Into<PatchId>) {
        self.selected.insert(patch.into());
    }

    pub fn disable_patch(&mut self, patch: impl Into<PatchId>) {
        self.disabled.insert(patch.into());
    }

    pub fn set_patch_options(&mut self, patch: impl Into<PatchId>, options: PatchOptions) {
        self.options.insert(patch.into(), options);
    }

    pub fn selected(&self) -> &HashSet<PatchId> {
        &self.selected
    }

    pub fn disabled(&self) -> &HashSet<PatchId> {
        &self.disabled
    }

    pub fn options(&self) -> &HashMap<PatchId, PatchOptions> {
        &self.options
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPatchPlan {
    order: Vec<usize>,
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    desired: Vec<bool>,
    disabled: Vec<bool>,
    options: Vec<Option<PatchOptions>>,
}

impl ResolvedPatchPlan {
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn dependencies_for(&self, idx: usize) -> &[usize] {
        &self.dependencies[idx]
    }

    pub fn dependents_for(&self, idx: usize) -> &[usize] {
        &self.dependents[idx]
    }

    pub fn is_desired(&self, idx: usize) -> bool {
        self.desired[idx]
    }

    pub fn is_disabled(&self, idx: usize) -> bool {
        self.disabled[idx]
    }

    pub fn options_for(&self, idx: usize) -> Option<&PatchOptions> {
        self.options[idx].as_ref()
    }
}
