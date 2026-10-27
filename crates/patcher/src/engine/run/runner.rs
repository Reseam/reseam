// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tracing::{info, info_span};

use super::results::Run;
use super::savepoints::Savepoints;
use super::{AppliedRun, Rewrite, guarded};
use crate::context::PatchContext;
use crate::engine::plan::unmountable_status;
use crate::engine::{PatchStatus, ResolvedPlan, RunEvent};
use crate::error::Result;
use crate::log::LogEntry;
use crate::patch::{Patch, PatchPhase, PatchSpec};

/// One run of a plan over a patch context: what has run, the savepoints that
/// can take it back, and who hears about it.
pub(super) struct Runner<'r, 'c, 'p, O> {
    ctx: &'r mut PatchContext<'c>,
    specs: &'r [&'r PatchSpec],
    code: &'r [Option<&'p Patch>],
    plan: ResolvedPlan,
    run: Run<'r>,
    savepoints: Savepoints,
    observer: O,
}

/// The phases a patch runs while the run holds the APK.
#[derive(Clone, Copy)]
enum Stage {
    Execute,
    Finalize,
}

impl<'r, 'c, 'p: 'r, O: FnMut(RunEvent)> Runner<'r, 'c, 'p, O> {
    pub(super) fn new(
        ctx: &'r mut PatchContext<'c>,
        specs: &'r [&'r PatchSpec],
        code: &'r [Option<&'p Patch>],
        plan: ResolvedPlan,
        observer: O,
    ) -> Self {
        Self {
            ctx,
            specs,
            code,
            plan,
            run: Run::new(specs, code),
            savepoints: Savepoints::default(),
            observer,
        }
    }

    /// Executes, then finalizes, the plan. Leaving a patch out can take back
    /// work already done, so the run then starts over from the plan as it now
    /// stands; patches already settled are not run again.
    pub(super) fn run(mut self) -> Result<AppliedRun<'p>> {
        while !(self.execute()? && self.finalize()?) {}
        self.savepoints.release_all(self.ctx)?;
        self.ctx.bind_app_entry()?;
        self.ctx.finish_hermes()?;
        info!("patch application finished");
        let rewrites = self
            .plan
            .order()
            .iter()
            .enumerate()
            .filter(|&(_, &idx)| self.specs[idx].rewrites_output && self.run.applied(idx))
            .map(|(result, &idx)| Rewrite {
                result,
                patch: self.code[idx].expect("an applied patch has its code loaded"),
                options: self.plan.options(idx).clone(),
            })
            .collect();
        Ok(AppliedRun {
            results: self.run.into_results(&self.plan),
            rewrites,
        })
    }

    /// Runs every patch the plan still executes; false when one was left out.
    fn execute(&mut self) -> Result<bool> {
        for idx in self.plan.order().to_vec() {
            if self.run.settled(idx) {
                continue;
            }
            let reference = &self.specs[idx].reference;
            let _span = info_span!("patch", patch = %reference).entered();
            if let Some(status) = self.run.blocked(&self.plan, idx) {
                self.run
                    .finish(&self.plan, idx, status, Vec::new(), &mut self.observer);
                continue;
            }
            let patch = self.code[idx]
                .expect("loaded_code holds the code of every patch the plan executes");
            let savepoint = self.savepoints.open(self.ctx, idx)?;
            self.ctx
                .begin_patch(reference, self.plan.options(idx).clone());
            (self.observer)(RunEvent::PatchStarted {
                patch: reference.clone(),
            });
            let (status, logs) = self.invoke(patch, Stage::Execute);
            if status != PatchStatus::Applied {
                self.savepoints.rollback(self.ctx, savepoint)?;
                self.leave_out(idx, status, logs)?;
                return Ok(false);
            }
            if self.plan.is_selected(idx) && !patch.finalizes() {
                self.savepoints.release(self.ctx, savepoint)?;
            }
            self.run
                .finish(&self.plan, idx, status, logs, &mut self.observer);
        }
        Ok(true)
    }

    /// Finalizes every applied patch that finalizes; false when one failed
    /// and was left out.
    fn finalize(&mut self) -> Result<bool> {
        for idx in self.plan.finalizers().to_vec() {
            let Some(patch) = self.code[idx].filter(|patch| patch.finalizes()) else {
                continue;
            };
            if !self.run.applied(idx) || self.run.finalized(idx) {
                continue;
            }
            let _span = info_span!("after_dependents", patch = %patch.reference()).entered();
            self.ctx
                .begin_patch(patch.reference(), self.plan.options(idx).clone());
            let (status, logs) = self.invoke(patch, Stage::Finalize);
            if status != PatchStatus::Applied {
                let mut all_logs = self.run.take_logs(idx);
                all_logs.extend(logs);
                self.leave_out(idx, status, all_logs)?;
                return Ok(false);
            }
            self.run.finalize(idx, logs, &mut self.observer);
        }
        Ok(true)
    }

    fn invoke(&mut self, patch: &Patch, stage: Stage) -> (PatchStatus, Vec<LogEntry>) {
        let manifest = self.ctx.apk().manifest_revision();
        let outcome = guarded(|| {
            patch.invoke(match stage {
                Stage::Execute => PatchPhase::Execute(self.ctx),
                Stage::Finalize => PatchPhase::Finalize(self.ctx),
            })
        });
        let logs = self.ctx.scope().log.take_entries();
        for log in &logs {
            (self.observer)(RunEvent::PatchLog(log.clone()));
        }
        let status = match outcome {
            Ok(())
                if self
                    .plan
                    .delivery()
                    .forbids_manifest_edit(manifest, self.ctx) =>
            {
                unmountable_status()
            }
            Ok(()) => PatchStatus::Applied,
            Err(reason) => PatchStatus::Failed {
                reason: match stage {
                    Stage::Execute => reason,
                    Stage::Finalize => format!("after_dependents: {reason}"),
                },
            },
        };
        (status, logs)
    }

    /// Leaves the patch at `idx` out of the plan with its final `status`. When
    /// the run already holds work the plan no longer allows, a dependency only
    /// this patch needed or, after a finalizer, patches that ran since this
    /// one, the run returns to before the earliest of it; those patches run
    /// again if the plan still covers them.
    fn leave_out(&mut self, idx: usize, status: PatchStatus, logs: Vec<LogEntry>) -> Result<()> {
        self.plan.exclude(idx, status.clone());
        let (plan, run) = (&self.plan, &self.run);
        if let Some((first, savepoint)) = self
            .savepoints
            .earliest(|held| held == idx || (run.applied(held) && !plan.executes(held)))
        {
            self.savepoints.rollback(self.ctx, savepoint)?;
            let patches = self.run.revert_from(&self.plan, first);
            if !patches.is_empty() {
                (self.observer)(RunEvent::PatchesReverted { patches });
            }
        }
        self.run
            .finish_excluded(&self.plan, idx, status, logs, &mut self.observer);
        assert!(
            (0..self.plan.len()).all(|held| !self.run.applied(held) || self.plan.executes(held)),
            "every patch the plan no longer runs held a savepoint at or before its work"
        );
        Ok(())
    }
}
