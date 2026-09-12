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

        ctx.begin_patch(&patch.reference(), plan.options(idx).clone());
        observer(ProgressEvent::PatchStarted {
            patch: patch.reference(),
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
        let _span = info_span!("after_dependents", patch = patch.reference()).entered();
        ctx.begin_patch(&patch.reference(), plan.options(idx).clone());
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
        if let Some(reason) = self.plan.unavailable(idx) {
            return Some(reason.to_owned());
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
        let patch = self.patches[idx].reference();
        observer(ProgressEvent::PatchFinished {
            patch: patch.clone(),
            status: status.clone(),
        });
        self.results[idx] = Some(PatchResult {
            patch,
            hidden: self.patches[idx].spec().hidden,
            required_by: self
                .plan
                .required_by(idx)
                .map(|dependent| self.patches[dependent].reference())
                .collect(),
            status,
            logs,
        });
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
            patch: result.patch.clone(),
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
    use crate::patch::{CompatiblePackage, PatchSpec};

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
        declared_in("bundle", id, package, versions, &[])
    }

    fn declared_in(
        bundle: &str,
        id: &str,
        package: &str,
        versions: &[&str],
        dependencies: &[&str],
    ) -> Declared {
        Declared(PatchSpec {
            bundle: bundle.to_owned(),
            id: id.to_owned(),
            name: id.to_owned(),
            hidden: false,
            description: String::new(),
            enabled_by_default: true,
            dependencies: dependencies.iter().map(|d| (*d).to_owned()).collect(),
            compatibility: [CompatiblePackage {
                package: package.to_owned(),
                versions: versions.iter().map(|v| (*v).to_owned()).collect(),
            }]
            .into_iter()
            .collect(),
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
    fn patches_resolve_across_bundles_by_reference_or_unique_id() {
        let official = declared_in("official", "pairip", "com.example", &[], &[]);
        let fork = declared_in("fork", "pairip", "com.example", &[], &[]);
        let uses = declared_in(
            "fork",
            "uses-pairip",
            "com.example",
            &[],
            &["official/pairip"],
        );
        let patches: Vec<&dyn Patch> = vec![&official, &fork, &uses];
        let selection = PatchSelection {
            enable: ["uses-pairip".to_owned()].into(),
            ..Default::default()
        };
        let results = validate_patches(&patches, &selection, Some("com.example"), None).unwrap();
        let applied: Vec<&str> = results
            .iter()
            .filter(|result| result.status == PatchStatus::Applied)
            .map(|result| result.patch.as_str())
            .collect();
        assert_eq!(applied, ["official/pairip", "fork/uses-pairip"]);

        let ambiguous = PatchSelection {
            enable: ["pairip".to_owned()].into(),
            ..Default::default()
        };
        let error = validate_patches(&patches, &ambiguous, Some("com.example"), None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("fork/pairip") && error.contains("official/pairip"),
            "{error}"
        );
    }

    #[test]
    fn missing_dependencies_skip_unless_selected() {
        let uses = declared_in(
            "fork",
            "uses-pairip",
            "com.example",
            &[],
            &["official/pairip"],
        );
        let patches: Vec<&dyn Patch> = vec![&uses];
        let results = validate_patches(
            &patches,
            &PatchSelection::default(),
            Some("com.example"),
            None,
        )
        .unwrap();
        assert_eq!(
            results[0].status,
            PatchStatus::Skipped {
                reason: "depends on official/pairip; load bundle 'official' alongside".to_owned()
            }
        );
        let selection = PatchSelection {
            enable: ["uses-pairip".to_owned()].into(),
            ..Default::default()
        };
        let error = validate_patches(&patches, &selection, Some("com.example"), None)
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "missing bundle: patch fork/uses-pairip depends on official/pairip; load bundle 'official' alongside"
        );

        let official = declared_in("official", "other", "com.example", &[], &[]);
        let patches: Vec<&dyn Patch> = vec![&official, &uses];
        let results = validate_patches(
            &patches,
            &PatchSelection::default(),
            Some("com.example"),
            None,
        )
        .unwrap();
        assert_eq!(
            results[1].status,
            PatchStatus::Skipped {
                reason: "depends on official/pairip, which bundle 'official' does not declare"
                    .to_owned()
            }
        );
        let error = validate_patches(&patches, &selection, Some("com.example"), None)
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "missing dependency: patch fork/uses-pairip depends on official/pairip, which bundle 'official' does not declare"
        );
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
