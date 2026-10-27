// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use tracing::{info, info_span};

use super::plan::unmountable_status;
use super::{PatchResult, PatchStatus, ResolvedPlan, RunEvent};
use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::log::LogEntry;
use crate::options::PatchOptions;
use crate::output::{OutputContext, WrittenApk};
use crate::patch::{Patch, PatchPhase, PatchSpec};

mod results;
mod savepoints;

use results::Run;
use savepoints::Savepoints;

/// Runs `plan` over `specs` in dependency order, then the finalize phase of
/// every applied patch that finalizes, then binds the app entry hook to
/// the final manifest. `patches` holds the code of at least every patch the
/// plan executes.
///
/// A patch that fails, panics or, in a mount build, edits the manifest leaves
/// nothing behind: the run returns to before it and goes on as the plan would
/// have without it, so the patches that need it are skipped and dependencies
/// only they needed are undone. Each patch is reported finished once its
/// result is final: after execution, or after finalization when it finalizes.
/// The applied patches that rewrite output run later, through
/// [`AppliedRun::rewrite_output`], once the host has written the APK.
pub fn apply_patches<'p>(
    ctx: &mut PatchContext<'_>,
    specs: &[&PatchSpec],
    mut plan: ResolvedPlan,
    patches: &[&'p Patch],
    mut observer: impl FnMut(RunEvent),
) -> Result<AppliedRun<'p>> {
    #[cfg(feature = "bridge")]
    let _bridge = crate::kotlin::run_state::RunGuard::enter()?;
    info!(
        patch_count = plan.order().len(),
        "starting patch application"
    );
    let code = loaded_code(specs, &plan, patches)?;
    let mut run = Run::new(specs, &code);
    let mut savepoints = Savepoints::default();

    'run: loop {
        for idx in plan.order().to_vec() {
            if run.settled(idx) {
                continue;
            }
            let reference = specs[idx].reference();
            let _span = info_span!("patch", patch = reference.as_str()).entered();
            if let Some(status) = run.blocked(&plan, idx) {
                run.finish(&plan, idx, status, Vec::new(), &mut observer);
                continue;
            }
            let patch =
                code[idx].expect("loaded_code holds the code of every patch the plan executes");
            let savepoint = savepoints.open(ctx, idx)?;
            ctx.begin_patch(&reference, plan.options(idx).clone());
            observer(RunEvent::PatchStarted {
                patch: reference.clone(),
            });
            let (status, logs) = invoke(ctx, &plan, patch, Stage::Execute, &mut observer);
            if status == PatchStatus::Applied {
                if plan.is_selected(idx) && !patch.finalizes() {
                    savepoints.release(ctx, savepoint)?;
                }
                run.finish(&plan, idx, status, logs, &mut observer);
            } else {
                savepoints.rollback(ctx, savepoint)?;
                leave_out(
                    ctx,
                    &mut plan,
                    &mut run,
                    &mut savepoints,
                    idx,
                    status,
                    logs,
                    &mut observer,
                )?;
                continue 'run;
            }
        }

        for idx in plan.finalizers().to_vec() {
            let Some(patch) = code[idx].filter(|patch| patch.finalizes()) else {
                continue;
            };
            if !run.applied(idx) || run.finalized(idx) {
                continue;
            }
            let _span = info_span!("after_dependents", patch = patch.reference()).entered();
            ctx.begin_patch(patch.reference(), plan.options(idx).clone());
            let (status, logs) = invoke(ctx, &plan, patch, Stage::Finalize, &mut observer);
            if status == PatchStatus::Applied {
                run.finalize(idx, logs, &mut observer);
            } else {
                let mut all_logs = run.take_logs(idx);
                all_logs.extend(logs);
                leave_out(
                    ctx,
                    &mut plan,
                    &mut run,
                    &mut savepoints,
                    idx,
                    status,
                    all_logs,
                    &mut observer,
                )?;
                continue 'run;
            }
        }
        break;
    }

    savepoints.release_all(ctx)?;
    ctx.bind_app_entry()?;
    ctx.finish_hermes()?;
    info!("patch application finished");
    let rewrites = plan
        .order()
        .iter()
        .enumerate()
        .filter(|&(_, &idx)| specs[idx].rewrites_output && run.applied(idx))
        .map(|(result, &idx)| Rewrite {
            result,
            patch: code[idx].expect("an applied patch has its code loaded"),
            options: plan.options(idx).clone(),
        })
        .collect();
    Ok(AppliedRun {
        results: run.into_results(&plan),
        rewrites,
    })
}

/// The outcome of [`apply_patches`]: each patch's result, and the applied
/// patches that rewrite the output once the host has written it.
pub struct AppliedRun<'p> {
    results: Vec<PatchResult>,
    rewrites: Vec<Rewrite<'p>>,
}

struct Rewrite<'p> {
    /// Position of the patch's result in [`AppliedRun::results`].
    result: usize,
    patch: &'p Patch,
    options: PatchOptions,
}

impl AppliedRun<'_> {
    /// Each patch's result, in the order the run covered them.
    pub fn results(&self) -> &[PatchResult] {
        &self.results
    }

    /// Whether an applied patch rewrites the output, so the host must call
    /// [`Self::rewrite_output`] between writing and signing.
    pub fn rewrites_output(&self) -> bool {
        !self.rewrites.is_empty()
    }

    /// Runs the applied patches that rewrite output over `apk`, dependencies
    /// first; their logs join their results. The output is already written, so
    /// a patch can no longer be left out of it, and a failure ends the run.
    pub fn rewrite_output(
        &mut self,
        apk: &mut WrittenApk,
        mut observer: impl FnMut(RunEvent),
    ) -> Result<()> {
        #[cfg(feature = "bridge")]
        let _bridge = crate::kotlin::run_state::RunGuard::enter()?;
        for rewrite in &self.rewrites {
            let reference = rewrite.patch.reference();
            let _span = info_span!("rewrite_output", patch = reference).entered();
            let mut ctx = OutputContext::new(apk, reference, rewrite.options.clone());
            let outcome = guarded(|| rewrite.patch.invoke(PatchPhase::RewriteOutput(&mut ctx)));
            let logs = ctx.take_log_entries();
            for log in &logs {
                observer(RunEvent::PatchLog(log.clone()));
            }
            self.results[rewrite.result].logs.extend(logs);
            outcome.map_err(|reason| PatcherError::RewriteOutput {
                patch: reference.to_owned(),
                reason,
            })?;
        }
        Ok(())
    }

    pub fn into_results(self) -> Vec<PatchResult> {
        self.results
    }
}

/// The outcome of `plan` without touching the APK or loading patch code: which
/// patches would run and which would be skipped.
pub fn validate_patches(specs: &[&PatchSpec], plan: &ResolvedPlan) -> Vec<PatchResult> {
    let code = vec![None; specs.len()];
    let mut run = Run::new(specs, &code);
    for &idx in plan.order() {
        let status = run.blocked(plan, idx).unwrap_or(PatchStatus::Applied);
        run.finish(plan, idx, status, Vec::new(), &mut |_| {});
    }
    run.into_results(plan)
}

/// The phases a patch runs while the run holds the APK.
#[derive(Clone, Copy)]
enum Stage {
    Execute,
    Finalize,
}

fn invoke(
    ctx: &mut PatchContext<'_>,
    plan: &ResolvedPlan,
    patch: &Patch,
    stage: Stage,
    observer: &mut impl FnMut(RunEvent),
) -> (PatchStatus, Vec<LogEntry>) {
    let manifest = ctx.apk().manifest_revision();
    let outcome = guarded(|| {
        patch.invoke(match stage {
            Stage::Execute => PatchPhase::Execute(ctx),
            Stage::Finalize => PatchPhase::Finalize(ctx),
        })
    });
    let logs = ctx.take_log_entries();
    for log in &logs {
        observer(RunEvent::PatchLog(log.clone()));
    }
    let status = match outcome {
        Ok(()) if plan.delivery().forbids_manifest_edit(manifest, ctx) => unmountable_status(),
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

/// Leaves the patch at `idx` out of the plan with its final `status`. When the
/// run already holds work the plan no longer allows, a dependency only this
/// patch needed or, after a finalizer, patches that ran since this one, the run
/// returns to before the earliest of it; those patches run again if the plan
/// still covers them.
#[expect(
    clippy::too_many_arguments,
    reason = "the run's parts are separate borrows of apply_patches"
)]
fn leave_out(
    ctx: &mut PatchContext<'_>,
    plan: &mut ResolvedPlan,
    run: &mut Run<'_>,
    savepoints: &mut Savepoints,
    idx: usize,
    status: PatchStatus,
    logs: Vec<LogEntry>,
    observer: &mut impl FnMut(RunEvent),
) -> Result<()> {
    plan.exclude(idx, status.clone());
    if let Some((first, savepoint)) =
        savepoints.earliest(|held| held == idx || (run.applied(held) && !plan.executes(held)))
    {
        savepoints.rollback(ctx, savepoint)?;
        let patches = run.revert_from(plan, first);
        if !patches.is_empty() {
            observer(RunEvent::PatchesReverted { patches });
        }
    }
    run.finish_excluded(plan, idx, status, logs, observer);
    assert!(
        (0..plan.len()).all(|held| !run.applied(held) || plan.executes(held)),
        "every patch the plan no longer runs held a savepoint at or before its work"
    );
    Ok(())
}

/// The code of each patch in `specs`, by position, when `patches` holds it.
fn loaded_code<'a>(
    specs: &[&PatchSpec],
    plan: &ResolvedPlan,
    patches: &[&'a Patch],
) -> Result<Vec<Option<&'a Patch>>> {
    let by_reference: HashMap<&str, &'a Patch> = patches
        .iter()
        .map(|&patch| (patch.reference(), patch))
        .collect();
    specs
        .iter()
        .enumerate()
        .map(|(idx, spec)| {
            let reference = spec.reference();
            match by_reference.get(reference.as_str()) {
                Some(&patch) => Ok(Some(patch)),
                None if plan.executes(idx) => Err(PatcherError::NotLoaded(reference)),
                None => Ok(None),
            }
        })
        .collect()
}

fn guarded(hook: impl FnOnce() -> Result<()>) -> std::result::Result<(), String> {
    match panic::catch_unwind(AssertUnwindSafe(hook)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(panic) => Err(format!(
            "panic: {}",
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string))
                .unwrap_or_else(|| "unknown panic".to_owned())
        )),
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
