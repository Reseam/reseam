// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::panic::{self, AssertUnwindSafe};

use tracing::{info, info_span};

use super::{PatchResult, PatchSelection, PatchStatus, ProgressEvent, ResolvedPlan};
use crate::context::PatchContext;
use crate::error::Result;
use crate::log::LogEntry;
use crate::patch::{Patch, PatchPhase};

/// Runs the selected patches in dependency order, then the `after_dependents`
/// hook of every applied patch that finalizes, then binds the app entry hook to
/// the final manifest. A patch that fails or panics does not stop the run;
/// patches depending on it are skipped. Each patch is reported finished once its
/// result is final: after execution, or after finalization when it finalizes.
pub fn apply_patches(
    ctx: &mut PatchContext<'_>,
    patches: &[&Patch],
    selection: &PatchSelection,
    mut observer: impl FnMut(ProgressEvent),
) -> Result<Vec<PatchResult>> {
    #[cfg(feature = "bridge")]
    let _bridge = crate::kotlin::handles::RunGuard::enter()?;
    info!(patch_count = patches.len(), "starting patch application");
    let package = ctx.apk().package_name().map(Cow::into_owned);
    let version = ctx.apk().version_name().map(Cow::into_owned);
    let plan = ResolvedPlan::resolve(patches, selection, package.as_deref(), version.as_deref())?;
    let mut run = Run::new(patches, &plan);

    for &idx in plan.order() {
        let patch = &patches[idx];
        let _span = info_span!("patch", patch = patch.reference()).entered();
        if let Some(reason) = run.skip_reason(idx, package.as_deref(), version.as_deref()) {
            run.finish(
                idx,
                PatchStatus::Skipped { reason },
                Vec::new(),
                &mut observer,
            );
            continue;
        }

        ctx.begin_patch(patch.reference(), plan.options(idx).clone());
        observer(ProgressEvent::Started {
            patch: patch.reference().to_owned(),
        });
        let outcome = guarded(|| patch.invoke(PatchPhase::Execute, ctx));
        let logs = ctx.take_log_entries();
        for log in &logs {
            observer(ProgressEvent::Log(log.clone()));
        }
        let status = match outcome {
            Ok(()) => PatchStatus::Applied,
            Err(reason) => PatchStatus::Failed { reason },
        };
        run.finish(idx, status, logs, &mut observer);
    }

    for &idx in plan.finalizers() {
        let patch = patches[idx];
        if !patch.finalizes() || !run.applied(idx) {
            continue;
        }
        let _span = info_span!("after_dependents", patch = patch.reference()).entered();
        ctx.begin_patch(patch.reference(), plan.options(idx).clone());
        let outcome = guarded(|| patch.invoke(PatchPhase::Finalize, ctx));
        let logs = ctx.take_log_entries();
        for log in &logs {
            observer(ProgressEvent::Log(log.clone()));
        }
        run.append_logs(idx, logs);
        if let Err(reason) = outcome {
            run.fail(idx, format!("after_dependents: {reason}"));
        }
        run.terminal(idx, &mut observer);
    }

    ctx.bind_app_entry()?;
    ctx.finish_hermes()?;
    info!("patch application finished");
    Ok(run.into_results())
}

/// The outcome of `apply_patches` without touching the APK: which patches
/// would run and which would be skipped, given the selection and the app's
/// package and version.
pub fn validate_patches(
    patches: &[&Patch],
    selection: &PatchSelection,
    package: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<PatchResult>> {
    let plan = ResolvedPlan::resolve(patches, selection, package, version)?;
    let mut run = Run::new(patches, &plan);
    for &idx in plan.order() {
        let status = match run.skip_reason(idx, package, version) {
            Some(reason) => PatchStatus::Skipped { reason },
            None => PatchStatus::Applied,
        };
        run.finish(idx, status, Vec::new(), &mut |_| {});
    }
    Ok(run.into_results())
}

struct Run<'a> {
    patches: &'a [&'a Patch],
    plan: &'a ResolvedPlan,
    results: Vec<Option<PatchResult>>,
}

impl<'a> Run<'a> {
    fn new(patches: &'a [&'a Patch], plan: &'a ResolvedPlan) -> Self {
        Self {
            patches,
            plan,
            results: (0..patches.len()).map(|_| None).collect(),
        }
    }

    fn skip_reason(
        &self,
        idx: usize,
        package: Option<&str>,
        version: Option<&str>,
    ) -> Option<String> {
        if !self.plan.is_desired(idx) {
            return Some("not selected".to_owned());
        }
        if self.plan.is_disabled(idx) {
            return Some("disabled explicitly".to_owned());
        }
        if let Some(reason) = self.plan.unavailable(idx) {
            return Some(reason.to_owned());
        }
        let spec = self.patches[idx].spec();
        if let Some(reason) = if self.plan.ignores_versions() {
            spec.package_incompatibility(package)
        } else {
            spec.incompatibility(package, version)
        } {
            return Some(reason);
        }
        for &dependency in self.plan.dependencies(idx) {
            let detail = match self.results[dependency]
                .as_ref()
                .map(|result| &result.status)
            {
                Some(PatchStatus::Applied) => continue,
                Some(PatchStatus::Skipped { reason }) => format!("skipped: {reason}"),
                Some(PatchStatus::Failed { reason }) => format!("failed: {reason}"),
                None => "was not executed".to_owned(),
            };
            return Some(format!(
                "dependency '{}' {detail}",
                self.patches[dependency].reference()
            ));
        }
        None
    }

    fn applied(&self, idx: usize) -> bool {
        matches!(
            self.results[idx].as_ref().map(|r| &r.status),
            Some(PatchStatus::Applied)
        )
    }

    fn finish(
        &mut self,
        idx: usize,
        status: PatchStatus,
        logs: Vec<LogEntry>,
        observer: &mut impl FnMut(ProgressEvent),
    ) {
        let patch = self.patches[idx].reference().to_owned();
        self.results[idx] = Some(PatchResult {
            patch,
            hidden: self.patches[idx].spec().hidden,
            required_by: self
                .plan
                .required_by(idx)
                .map(|dependent| self.patches[dependent].reference().to_owned())
                .collect(),
            status,
            logs,
        });
        if !self.applied(idx) || !self.patches[idx].finalizes() {
            self.terminal(idx, observer);
        }
    }

    fn result_mut(&mut self, idx: usize) -> &mut PatchResult {
        self.results[idx]
            .as_mut()
            .expect("execution creates a result before finalization")
    }

    fn append_logs(&mut self, idx: usize, logs: Vec<LogEntry>) {
        self.result_mut(idx).logs.extend(logs);
    }

    fn fail(&mut self, idx: usize, reason: String) {
        self.result_mut(idx).status = PatchStatus::Failed { reason };
    }

    fn terminal(&mut self, idx: usize, observer: &mut impl FnMut(ProgressEvent)) {
        let result = self.result_mut(idx);
        observer(ProgressEvent::Finished {
            patch: result.patch.clone(),
            status: result.status.clone(),
        });
    }

    fn into_results(mut self) -> Vec<PatchResult> {
        self.plan
            .order()
            .iter()
            .map(|&idx| {
                self.results[idx]
                    .take()
                    .expect("plan visits every patch exactly once after execution")
            })
            .collect()
    }
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
