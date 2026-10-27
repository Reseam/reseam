// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use tracing::{info, info_span};

use super::{PatchResult, PatchStatus, ResolvedPlan, RunEvent};
use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::log::PatchScope;
use crate::options::PatchOptions;
use crate::output::{OutputContext, WrittenApk};
use crate::patch::{Patch, PatchPhase, PatchRef, PatchSpec};

mod results;
mod runner;
mod savepoints;

use results::Run;
use runner::Runner;

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
    plan: ResolvedPlan,
    patches: &[&'p Patch],
    observer: impl FnMut(RunEvent),
) -> Result<AppliedRun<'p>> {
    #[cfg(feature = "bridge")]
    let _bridge = crate::bridge::run_state::RunGuard::enter()?;
    info!(
        patch_count = plan.order().len(),
        "starting patch application"
    );
    let code = loaded_code(specs, &plan, patches)?;
    Runner::new(ctx, specs, &code, plan, observer).run()
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
        let _bridge = crate::bridge::run_state::RunGuard::enter()?;
        for rewrite in &self.rewrites {
            let reference = rewrite.patch.reference();
            let _span = info_span!("rewrite_output", patch = %reference).entered();
            let mut ctx =
                OutputContext::new(apk, PatchScope::new(reference, rewrite.options.clone()));
            let outcome = guarded(|| rewrite.patch.invoke(PatchPhase::RewriteOutput(&mut ctx)));
            let logs = ctx.scope().log.take_entries();
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

/// The code of each patch in `specs`, by position, when `patches` holds it.
fn loaded_code<'a>(
    specs: &[&PatchSpec],
    plan: &ResolvedPlan,
    patches: &[&'a Patch],
) -> Result<Vec<Option<&'a Patch>>> {
    let by_reference: HashMap<&PatchRef, &'a Patch> = patches
        .iter()
        .map(|&patch| (patch.reference(), patch))
        .collect();
    specs
        .iter()
        .enumerate()
        .map(|(idx, spec)| match by_reference.get(&spec.reference) {
            Some(&patch) => Ok(Some(patch)),
            None if plan.executes(idx) => Err(PatcherError::NotLoaded(spec.reference.clone())),
            None => Ok(None),
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
