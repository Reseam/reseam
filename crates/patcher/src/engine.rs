// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};

use crate::context::PatchContext;
use crate::dependency;
use crate::error::{PatcherError, Result};
use crate::log::{LogEntry, PatchLog};
use crate::options::{validate_patch_options, PatchOptions};
use crate::patch::Patch;
use tracing::{debug, info, info_span, warn};

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
pub struct ExecutionPlan {
    selected: HashSet<String>,
    disabled: HashSet<String>,
    options: HashMap<String, PatchOptions>,
}

impl ExecutionPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_patch(&mut self, patch: impl Into<String>) {
        self.selected.insert(patch.into());
    }

    pub fn disable_patch(&mut self, patch: impl Into<String>) {
        self.disabled.insert(patch.into());
    }

    pub fn set_patch_options(&mut self, patch: impl Into<String>, options: PatchOptions) {
        self.options.insert(patch.into(), options);
    }

    pub fn selected(&self) -> &HashSet<String> {
        &self.selected
    }

    pub fn disabled(&self) -> &HashSet<String> {
        &self.disabled
    }

    pub fn options(&self) -> &HashMap<String, PatchOptions> {
        &self.options
    }
}

pub fn apply_patches(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
) -> Result<Vec<PatchResult>> {
    apply_patches_with_plan(ctx, patches, &ExecutionPlan::default())
}

pub fn apply_patches_with_options(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    options: &HashMap<String, PatchOptions>,
) -> Result<Vec<PatchResult>> {
    let mut plan = ExecutionPlan::default();
    for (patch, patch_options) in options {
        plan.set_patch_options(patch.clone(), patch_options.clone());
    }
    apply_patches_with_plan(ctx, patches, &plan)
}

pub fn apply_patches_with_plan(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    plan: &ExecutionPlan,
) -> Result<Vec<PatchResult>> {
    apply_patches_with_plan_and_observer(ctx, patches, plan, |_| {})
}

pub fn apply_patches_with_plan_and_observer<F>(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    plan: &ExecutionPlan,
    mut observer: F,
) -> Result<Vec<PatchResult>>
where
    F: FnMut(ProgressEvent),
{
    info!(
        patch_count = patches.len(),
        configured_patches = plan.options.len(),
        selected_patches = plan.selected.len(),
        disabled_patches = plan.disabled.len(),
        "starting patch application"
    );
    let order = dependency::sort_patches(patches)?;
    let dependents = dependency::find_dependents(patches);
    let name_to_idx: HashMap<&str, usize> = patches
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name(), i))
        .collect();
    validate_execution_plan(patches, &name_to_idx, plan)?;
    let desired = resolve_desired_patches(patches, &name_to_idx, plan)?;
    let validated_options = validate_plan_options(patches, &desired, plan)?;

    let package = ctx.package_name().map(|s| s.to_owned());
    let version = ctx.version_name().map(|s| s.to_owned());

    let mut results = Vec::with_capacity(patches.len());
    let mut applied: Vec<bool> = vec![false; patches.len()];
    let mut after_dependents_fired: Vec<bool> = vec![false; patches.len()];
    let mut result_map: HashMap<usize, usize> = HashMap::new();
    let mut merged_extensions: HashSet<String> = HashSet::new();

    for &idx in &order {
        let patch = &patches[idx];
        let patch_span = info_span!("patch", patch = patch.name());
        let _patch_guard = patch_span.enter();

        if !desired.contains(&idx) {
            debug!("patch skipped because it is not selected by the execution plan");
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped {
                    reason: "not selected".to_owned(),
                },
                logs: Vec::new(),
            };
            observer(ProgressEvent::PatchFinished {
                patch: r.name.clone(),
                status: r.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if plan.disabled.contains(patch.name()) {
            warn!("patch skipped because it is explicitly disabled");
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped {
                    reason: "disabled explicitly".to_owned(),
                },
                logs: Vec::new(),
            };
            observer(ProgressEvent::PatchFinished {
                patch: r.name.clone(),
                status: r.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if let Some(reason) =
            dependency_skip_reason(patches, &name_to_idx, &applied, &result_map, &results, idx)
        {
            warn!(reason, "patch skipped due to dependency state");
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            observer(ProgressEvent::PatchFinished {
                patch: r.name.clone(),
                status: r.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if let Some(reason) =
            check_compatibility(patch.as_ref(), package.as_deref(), version.as_deref())
        {
            warn!(reason, "patch skipped due to compatibility check");
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            observer(ProgressEvent::PatchFinished {
                patch: r.name.clone(),
                status: r.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if let Some(opts) = validated_options.get(patch.name()) {
            ctx.set_options(opts.clone());
        } else {
            ctx.clear_options();
        }

        ctx.set_log(PatchLog::new(patch.name().to_owned()));
        observer(ProgressEvent::PatchStarted {
            patch: patch.name().to_owned(),
        });

        // Merge extension DEX files declared by this patch (deduplicated).
        let ext_paths = patch.extension_dex();
        if !ext_paths.is_empty() {
            debug!(
                extension_count = ext_paths.len(),
                "patch declares extension DEX files"
            );
            let new_paths: Vec<&str> = ext_paths
                .iter()
                .filter(|p| merged_extensions.insert((*p).clone()))
                .map(|p| p.as_str())
                .collect();
            if !new_paths.is_empty() {
                if let Err(e) = ctx.merge_extension_dex(&new_paths) {
                    warn!(error = %e, "failed to merge patch extension DEX");
                    let r = PatchResult {
                        name: patch.name().to_owned(),
                        status: PatchStatus::Failed {
                            reason: format!("extension merge: {e}"),
                        },
                        logs: ctx.take_log_entries(),
                    };
                    for log in &r.logs {
                        observer(ProgressEvent::PatchLog(log.clone()));
                    }
                    observer(ProgressEvent::PatchFinished {
                        patch: r.name.clone(),
                        status: r.status.clone(),
                    });
                    result_map.insert(idx, results.len());
                    results.push(r);
                    continue;
                }
            }
        }

        let exec_result = panic::catch_unwind(AssertUnwindSafe(|| patch.execute(ctx)));

        let logs = ctx.take_log_entries();
        for log in &logs {
            observer(ProgressEvent::PatchLog(log.clone()));
        }

        match exec_result {
            Ok(Ok(())) => {
                info!("patch applied successfully");
                applied[idx] = true;
                let r = PatchResult {
                    name: patch.name().to_owned(),
                    status: PatchStatus::Applied,
                    logs,
                };
                observer(ProgressEvent::PatchFinished {
                    patch: r.name.clone(),
                    status: r.status.clone(),
                });
                result_map.insert(idx, results.len());
                results.push(r);
            }
            Ok(Err(e)) => {
                warn!(error = %e, "patch execution returned an error");
                let r = PatchResult {
                    name: patch.name().to_owned(),
                    status: PatchStatus::Failed {
                        reason: e.to_string(),
                    },
                    logs,
                };
                observer(ProgressEvent::PatchFinished {
                    patch: r.name.clone(),
                    status: r.status.clone(),
                });
                result_map.insert(idx, results.len());
                results.push(r);
                continue;
            }
            Err(panic_info) => {
                let reason = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };
                warn!(panic = %reason, "patch panicked during execution");
                let r = PatchResult {
                    name: patch.name().to_owned(),
                    status: PatchStatus::Failed {
                        reason: format!("panic: {reason}"),
                    },
                    logs,
                };
                observer(ProgressEvent::PatchFinished {
                    patch: r.name.clone(),
                    status: r.status.clone(),
                });
                result_map.insert(idx, results.len());
                results.push(r);
                continue;
            }
        }
    }

    // Run afterDependents hooks after ALL patches have been processed.
    // This ensures that skipped/disabled dependents don't block the hook.
    for &dep_idx in dependents.keys() {
        if after_dependents_fired[dep_idx] {
            continue;
        }
        let Some(&result_idx) = result_map.get(&dep_idx) else {
            continue;
        };
        if !matches!(results[result_idx].status, PatchStatus::Applied) {
            continue;
        }
        after_dependents_fired[dep_idx] = true;
        ctx.set_log(PatchLog::new(patches[dep_idx].name().to_owned()));
        debug!(
            patch = patches[dep_idx].name(),
            "running after_dependents hook"
        );

        let after_result =
            panic::catch_unwind(AssertUnwindSafe(|| patches[dep_idx].after_dependents(ctx)));

        let after_logs = ctx.take_log_entries();
        if let Some(&ri) = result_map.get(&dep_idx) {
            for log in &after_logs {
                observer(ProgressEvent::PatchLog(log.clone()));
            }
            results[ri].logs.extend(after_logs);
        }

        match after_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!(
                    patch = patches[dep_idx].name(),
                    error = %e,
                    "after_dependents hook failed"
                );
                results[result_idx].status = PatchStatus::Failed {
                    reason: format!("after_dependents: {e}"),
                };
                applied[dep_idx] = false;
                observer(ProgressEvent::PatchFinished {
                    patch: results[result_idx].name.clone(),
                    status: results[result_idx].status.clone(),
                });
            }
            Err(panic_info) => {
                let reason = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };
                warn!(
                    patch = patches[dep_idx].name(),
                    panic = %reason,
                    "after_dependents hook panicked"
                );
                results[result_idx].status = PatchStatus::Failed {
                    reason: format!("after_dependents panic: {reason}"),
                };
                applied[dep_idx] = false;
                observer(ProgressEvent::PatchFinished {
                    patch: results[result_idx].name.clone(),
                    status: results[result_idx].status.clone(),
                });
            }
        }
    }

    info!(result_count = results.len(), "patch application finished");
    Ok(results)
}

pub fn validate_patches_with_plan(
    patches: &[Box<dyn Patch>],
    plan: &ExecutionPlan,
    package: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<PatchResult>> {
    info!(
        patch_count = patches.len(),
        configured_patches = plan.options.len(),
        selected_patches = plan.selected.len(),
        disabled_patches = plan.disabled.len(),
        "starting patch validation"
    );
    let order = dependency::sort_patches(patches)?;
    let name_to_idx: HashMap<&str, usize> = patches
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name(), i))
        .collect();
    validate_execution_plan(patches, &name_to_idx, plan)?;
    let desired = resolve_desired_patches(patches, &name_to_idx, plan)?;
    validate_plan_options(patches, &desired, plan)?;

    let mut results = Vec::with_capacity(patches.len());
    let mut applied: Vec<bool> = vec![false; patches.len()];
    let mut result_map: HashMap<usize, usize> = HashMap::new();

    for &idx in &order {
        let patch = &patches[idx];

        if !desired.contains(&idx) {
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped {
                    reason: "not selected".to_owned(),
                },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if plan.disabled.contains(patch.name()) {
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped {
                    reason: "disabled explicitly".to_owned(),
                },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if let Some(reason) =
            dependency_skip_reason(patches, &name_to_idx, &applied, &result_map, &results, idx)
        {
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if let Some(reason) = check_compatibility(patch.as_ref(), package, version) {
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        applied[idx] = true;
        let r = PatchResult {
            name: patch.name().to_owned(),
            status: PatchStatus::Applied,
            logs: Vec::new(),
        };
        result_map.insert(idx, results.len());
        results.push(r);
    }

    info!(result_count = results.len(), "patch validation finished");
    Ok(results)
}

fn validate_execution_plan(
    patches: &[Box<dyn Patch>],
    name_to_idx: &HashMap<&str, usize>,
    plan: &ExecutionPlan,
) -> Result<()> {
    for name in &plan.selected {
        if !name_to_idx.contains_key(name.as_str()) {
            return Err(PatcherError::UnknownPatch(name.clone()));
        }
    }
    for name in &plan.disabled {
        if !name_to_idx.contains_key(name.as_str()) {
            return Err(PatcherError::UnknownPatch(name.clone()));
        }
        if plan.selected.contains(name) {
            return Err(PatcherError::InvalidSelection(format!(
                "patch '{name}' cannot be both selected and disabled"
            )));
        }
    }
    for name in plan.options.keys() {
        if !name_to_idx.contains_key(name.as_str()) {
            return Err(PatcherError::UnknownPatch(name.clone()));
        }
    }
    if patches
        .iter()
        .map(|patch| patch.name())
        .collect::<HashSet<_>>()
        .len()
        != patches.len()
    {
        return Err(PatcherError::InvalidSelection(
            "patch names must be unique within a bundle".to_owned(),
        ));
    }
    Ok(())
}

fn resolve_desired_patches(
    patches: &[Box<dyn Patch>],
    name_to_idx: &HashMap<&str, usize>,
    plan: &ExecutionPlan,
) -> Result<HashSet<usize>> {
    let mut desired = HashSet::new();
    let mut stack: Vec<usize> = if plan.selected.is_empty() {
        patches
            .iter()
            .enumerate()
            .filter(|(_, patch)| patch.enabled_by_default())
            .map(|(idx, _)| idx)
            .collect()
    } else {
        plan.selected
            .iter()
            .map(|name| name_to_idx[name.as_str()])
            .collect()
    };

    while let Some(idx) = stack.pop() {
        if !desired.insert(idx) {
            continue;
        }
        for dep in patches[idx].depends_on() {
            let dep_idx =
                *name_to_idx
                    .get(dep.as_str())
                    .ok_or_else(|| PatcherError::MissingDependency {
                        patch: patches[idx].name().to_owned(),
                        dependency: dep.clone(),
                    })?;
            stack.push(dep_idx);
        }
    }

    Ok(desired)
}

fn validate_plan_options(
    patches: &[Box<dyn Patch>],
    desired: &HashSet<usize>,
    plan: &ExecutionPlan,
) -> Result<HashMap<String, PatchOptions>> {
    let mut validated = HashMap::new();

    for (idx, patch) in patches.iter().enumerate() {
        if !desired.contains(&idx) || plan.disabled.contains(patch.name()) {
            if plan.options.contains_key(patch.name()) {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch '{}' has options configured but is not enabled by the execution plan",
                    patch.name()
                )));
            }
            continue;
        }

        let resolved = validate_patch_options(
            patch.name(),
            patch.options(),
            plan.options.get(patch.name()),
        )?;
        if resolved.iter().next().is_some() || !patch.options().is_empty() {
            validated.insert(patch.name().to_owned(), resolved);
        }
    }

    Ok(validated)
}

fn dependency_skip_reason(
    patches: &[Box<dyn Patch>],
    name_to_idx: &HashMap<&str, usize>,
    applied: &[bool],
    result_map: &HashMap<usize, usize>,
    results: &[PatchResult],
    idx: usize,
) -> Option<String> {
    for dep_name in patches[idx].depends_on() {
        let dep_idx = name_to_idx[dep_name.as_str()];
        if applied[dep_idx] {
            continue;
        }

        let dependency_result = result_map.get(&dep_idx).and_then(|ri| results.get(*ri));
        let detail = match dependency_result {
            Some(result) => match &result.status {
                PatchStatus::Applied => continue,
                PatchStatus::Skipped { reason } => format!("skipped: {reason}"),
                PatchStatus::Failed { reason } => format!("failed: {reason}"),
            },
            None => "was not executed".to_owned(),
        };
        return Some(format!("dependency '{}' {}", dep_name, detail));
    }
    None
}

fn check_compatibility(
    patch: &dyn Patch,
    package: Option<&str>,
    version: Option<&str>,
) -> Option<String> {
    let compat = patch.compatible_with();
    if compat.is_empty() {
        return None;
    }

    let pkg = match package {
        Some(pkg) => pkg,
        None => return Some("APK has no package name".to_owned()),
    };

    let entry = match compat.iter().find(|c| c.package == pkg) {
        Some(e) => e,
        None => return Some(format!("incompatible package: {pkg}")),
    };

    if !entry.versions.is_empty() {
        match version {
            Some(ver) if !entry.versions.iter().any(|v| v == ver) => {
                return Some(format!("incompatible version: {ver}"));
            }
            None => return Some("APK has no version name".to_owned()),
            _ => {}
        }
    }

    None
}
