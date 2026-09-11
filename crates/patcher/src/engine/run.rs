// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::panic::{self, AssertUnwindSafe};

use tracing::{info, info_span};

use super::{PatchResult, PatchSelection, PatchStatus, ProgressEvent, ResolvedPlan};
use crate::context::PatchContext;
use crate::error::Result;
use crate::log::LogEntry;
use crate::patch::Patch;

/// Runs the selected patches in dependency order, then every applied patch's
/// `after_dependents` hook. A patch that fails or panics does not stop the
/// run; patches depending on it are skipped.
pub fn apply_patches(
    ctx: &mut PatchContext,
    patches: &[&dyn Patch],
    selection: &PatchSelection,
    mut observer: impl FnMut(ProgressEvent),
) -> Result<Vec<PatchResult>> {
    info!(patch_count = patches.len(), "starting patch application");
    let package = ctx.apk().package_name().map(Cow::into_owned);
    let version = ctx.apk().version_name().map(Cow::into_owned);
    let plan = ResolvedPlan::resolve(patches, selection, package.as_deref())?;
    let mut run = Run::new(patches, &plan);

    for &idx in plan.order() {
        let patch = &patches[idx];
        let _span = info_span!("patch", patch = patch.id()).entered();
        if let Some(reason) = run.skip_reason(idx, package.as_deref(), version.as_deref()) {
            run.finish(
                idx,
                PatchStatus::Skipped { reason },
                Vec::new(),
                &mut observer,
            );
            continue;
        }

        ctx.begin_patch(patch.id(), plan.options(idx).clone());
        observer(ProgressEvent::PatchStarted {
            patch: patch.id().to_owned(),
        });
        let outcome = guarded(|| patch.execute(ctx));
        let logs = ctx.take_log_entries();
        for log in &logs {
            observer(ProgressEvent::PatchLog(log.clone()));
        }
        let status = match outcome {
            Ok(()) => PatchStatus::Applied,
            Err(reason) => PatchStatus::Failed { reason },
        };
        run.finish(idx, status, logs, &mut observer);
    }

    for (idx, patch) in patches.iter().enumerate() {
        if plan.dependents(idx).is_empty() || !run.applied(idx) {
            continue;
        }
        let _span = info_span!("after_dependents", patch = patch.id()).entered();
        ctx.begin_patch(patch.id(), plan.options(idx).clone());
        let outcome = guarded(|| patch.after_dependents(ctx));
        let logs = ctx.take_log_entries();
        for log in &logs {
            observer(ProgressEvent::PatchLog(log.clone()));
        }
        run.append_logs(idx, logs);
        if let Err(reason) = outcome {
            run.fail(idx, format!("after_dependents: {reason}"), &mut observer);
        }
    }

    info!("patch application finished");
    Ok(run.into_results())
}

/// The outcome of `apply_patches` without touching the APK: which patches
/// would run and which would be skipped, given the selection and the app's
/// package and version.
pub fn validate_patches(
    patches: &[&dyn Patch],
    selection: &PatchSelection,
    package: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<PatchResult>> {
    let plan = ResolvedPlan::resolve(patches, selection, package)?;
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
    patches: &'a [&'a dyn Patch],
    plan: &'a ResolvedPlan,
    results: Vec<Option<PatchResult>>,
}

impl<'a> Run<'a> {
    fn new(patches: &'a [&'a dyn Patch], plan: &'a ResolvedPlan) -> Self {
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
                self.patches[dependency].id()
            ));
        }
        let spec = self.patches[idx].spec();
        if self.plan.ignores_versions() {
            spec.package_incompatibility(package)
        } else {
            spec.incompatibility(package, version)
        }
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
        let name = self.patches[idx].id().to_owned();
        observer(ProgressEvent::PatchFinished {
            patch: name.clone(),
            status: status.clone(),
        });
        self.results[idx] = Some(PatchResult { name, status, logs });
    }

    fn append_logs(&mut self, idx: usize, logs: Vec<LogEntry>) {
        if let Some(result) = &mut self.results[idx] {
            result.logs.extend(logs);
        }
    }

    fn fail(&mut self, idx: usize, reason: String, observer: &mut impl FnMut(ProgressEvent)) {
        let Some(result) = &mut self.results[idx] else {
            return;
        };
        result.status = PatchStatus::Failed { reason };
        observer(ProgressEvent::PatchFinished {
            patch: result.name.clone(),
            status: result.status.clone(),
        });
    }

    fn into_results(self) -> Vec<PatchResult> {
        self.plan
            .order()
            .iter()
            .filter_map(|&idx| self.results[idx].clone())
            .collect()
    }
}

/// Runs a patch hook, turning an error or a panic into a reason string.
fn guarded(hook: impl FnOnce() -> Result<()>) -> std::result::Result<(), String> {
    match panic::catch_unwind(AssertUnwindSafe(hook)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(panic) => Err(format!(
            "panic: {}",
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".to_owned())
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Compatibility, PatchSpec};

    struct Declared(PatchSpec);

    impl Patch for Declared {
        fn spec(&self) -> &PatchSpec {
            &self.0
        }

        fn execute(&self, _ctx: &mut PatchContext) -> Result<()> {
            Ok(())
        }
    }

    fn declared(id: &str, package: &str, versions: &[&str]) -> Declared {
        Declared(PatchSpec {
            id: id.to_owned(),
            name: id.to_owned(),
            hidden: false,
            description: String::new(),
            enabled_by_default: true,
            dependencies: Vec::new(),
            compatibility: vec![Compatibility {
                package: package.to_owned(),
                versions: versions.iter().map(|v| (*v).to_owned()).collect(),
            }],
            options: Vec::new(),
        })
    }

    fn statuses(selection: &PatchSelection) -> Vec<PatchStatus> {
        let pinned = declared("pinned", "com.example", &["1.0"]);
        let other = declared("other", "com.other", &[]);
        let patches: Vec<&dyn Patch> = vec![&pinned, &other];
        validate_patches(&patches, selection, Some("com.example"), Some("2.0"))
            .unwrap()
            .into_iter()
            .map(|result| result.status)
            .collect()
    }

    #[test]
    fn version_mismatch_skips_unless_versions_are_ignored() {
        assert_eq!(
            statuses(&PatchSelection::default()),
            vec![
                PatchStatus::Skipped {
                    reason: "expected one of [1.0], got 2.0".to_owned()
                },
                PatchStatus::Skipped {
                    reason: "incompatible package: com.example".to_owned()
                },
            ]
        );
        assert_eq!(
            statuses(&PatchSelection {
                ignore_versions: true,
                ..Default::default()
            }),
            vec![
                PatchStatus::Applied,
                PatchStatus::Skipped {
                    reason: "incompatible package: com.example".to_owned()
                },
            ]
        );
    }
}
