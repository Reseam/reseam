// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::plan::CHANGES_MANIFEST;
use super::super::{PatchResult, PatchStatus, ResolvedPlan, RunEvent};
use crate::log::LogEntry;
use crate::patch::{Patch, PatchRef, PatchSpec};

pub(super) struct Run<'a> {
    specs: &'a [&'a PatchSpec],
    code: &'a [Option<&'a Patch>],
    results: Vec<Option<PatchResult>>,
    /// Patches in the order they got their results, apart from left-out ones,
    /// whose results stand whatever the run takes back.
    settled: Vec<usize>,
    /// How many log entries each result held when execution ended.
    execution_logs: Vec<usize>,
    finalized: Vec<bool>,
    /// The status each patch was last reported finished with.
    reported: Vec<Option<PatchStatus>>,
}

impl<'a> Run<'a> {
    pub(super) fn new(specs: &'a [&'a PatchSpec], code: &'a [Option<&'a Patch>]) -> Self {
        Self {
            specs,
            code,
            results: (0..specs.len()).map(|_| None).collect(),
            settled: Vec::new(),
            execution_logs: vec![0; specs.len()],
            finalized: vec![false; specs.len()],
            reported: vec![None; specs.len()],
        }
    }

    pub(super) fn settled(&self, idx: usize) -> bool {
        self.results[idx].is_some()
    }

    /// Why the patch cannot run, as its final status, or `None` when it can.
    pub(super) fn blocked(&self, plan: &ResolvedPlan, idx: usize) -> Option<PatchStatus> {
        if let Some(status) = plan.precluded(idx) {
            return Some(status.clone());
        }
        let skipped = |reason: String| Some(PatchStatus::Skipped { reason });
        for &dependency in plan.dependencies(idx) {
            let name = &self.specs[dependency].reference;
            match self.results[dependency]
                .as_ref()
                .map(|result| &result.status)
            {
                Some(PatchStatus::Applied) => {}
                Some(PatchStatus::Skipped { reason }) => {
                    return skipped(format!("dependency '{name}' skipped: {reason}"));
                }
                Some(PatchStatus::Unmountable { .. }) => {
                    return skipped(format!("dependency '{name}' {CHANGES_MANIFEST}"));
                }
                Some(PatchStatus::Failed { reason }) => {
                    return skipped(format!("dependency '{name}' failed: {reason}"));
                }
                None => return skipped(format!("dependency '{name}' was not executed")),
            }
        }
        None
    }

    pub(super) fn applied(&self, idx: usize) -> bool {
        matches!(
            self.results[idx].as_ref().map(|r| &r.status),
            Some(PatchStatus::Applied)
        )
    }

    pub(super) fn finalized(&self, idx: usize) -> bool {
        self.finalized[idx]
    }

    fn finalizes(&self, idx: usize) -> bool {
        self.code[idx].is_some_and(Patch::finalizes)
    }

    pub(super) fn finish(
        &mut self,
        plan: &ResolvedPlan,
        idx: usize,
        status: PatchStatus,
        logs: Vec<LogEntry>,
        observer: &mut impl FnMut(RunEvent),
    ) {
        self.settled.push(idx);
        self.execution_logs[idx] = logs.len();
        self.set_result(plan, idx, status, logs);
        if !self.applied(idx) || !self.finalizes(idx) {
            self.report(idx, observer);
        }
    }

    /// Records the final result of a patch the plan left out.
    pub(super) fn finish_excluded(
        &mut self,
        plan: &ResolvedPlan,
        idx: usize,
        status: PatchStatus,
        logs: Vec<LogEntry>,
        observer: &mut impl FnMut(RunEvent),
    ) {
        self.set_result(plan, idx, status, logs);
        self.report(idx, observer);
    }

    fn set_result(
        &mut self,
        plan: &ResolvedPlan,
        idx: usize,
        status: PatchStatus,
        logs: Vec<LogEntry>,
    ) {
        let spec = self.specs[idx];
        self.results[idx] = Some(PatchResult {
            patch: spec.reference.clone(),
            hidden: spec.hidden,
            required_by: plan
                .required_by(idx)
                .map(|dependent| self.specs[dependent].reference.clone())
                .collect(),
            status,
            logs,
        });
    }

    pub(super) fn finalize(
        &mut self,
        idx: usize,
        logs: Vec<LogEntry>,
        observer: &mut impl FnMut(RunEvent),
    ) {
        self.result_mut(idx).logs.extend(logs);
        self.finalized[idx] = true;
        self.report(idx, observer);
    }

    pub(super) fn take_logs(&mut self, idx: usize) -> Vec<LogEntry> {
        std::mem::take(&mut self.result_mut(idx).logs)
    }

    /// Takes back the results of `first` and every patch settled after it, and
    /// every finalization, which all came after it. Returns the patches that
    /// had been reported finished.
    pub(super) fn revert_from(&mut self, plan: &ResolvedPlan, first: usize) -> Vec<PatchRef> {
        let start = self
            .settled
            .iter()
            .position(|&idx| idx == first)
            .expect("a held savepoint belongs to a settled patch");
        let mut reverted = Vec::new();
        for idx in self.settled.split_off(start) {
            if plan.is_excluded(idx) {
                continue;
            }
            if self.reported[idx].take().is_some() {
                reverted.push(self.specs[idx].reference.clone());
            }
            self.results[idx] = None;
            self.finalized[idx] = false;
        }
        for &idx in &self.settled {
            if std::mem::take(&mut self.finalized[idx]) {
                self.results[idx]
                    .as_mut()
                    .expect("a finalized patch has a result")
                    .logs
                    .truncate(self.execution_logs[idx]);
            }
        }
        reverted
    }

    fn result_mut(&mut self, idx: usize) -> &mut PatchResult {
        self.results[idx]
            .as_mut()
            .expect("execution creates a result before finalization")
    }

    /// Reports the patch finished unless it already was with this status.
    fn report(&mut self, idx: usize, observer: &mut impl FnMut(RunEvent)) {
        let result = self.results[idx]
            .as_ref()
            .expect("a reported patch has a result");
        if self.reported[idx].as_ref() == Some(&result.status) {
            return;
        }
        self.reported[idx] = Some(result.status.clone());
        observer(RunEvent::PatchFinished {
            patch: result.patch.clone(),
            status: result.status.clone(),
        });
    }

    pub(super) fn into_results(mut self, plan: &ResolvedPlan) -> Vec<PatchResult> {
        plan.order()
            .iter()
            .map(|&idx| {
                self.results[idx]
                    .take()
                    .expect("plan visits every patch exactly once after execution")
            })
            .collect()
    }
}
