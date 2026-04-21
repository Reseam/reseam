// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use tracing::{debug, info, info_span, warn};

use super::{
    resolve_patch_selection, ExecutionPlan, PatchResult, PatchSelection, PatchStatus,
    ProgressEvent, ResolvedPatchPlan,
};
use crate::context::PatchContext;
use crate::error::Result;
use crate::log::PatchLog;
use crate::options::PatchOptions;
use crate::patch::Patch;

pub fn apply_patches(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
) -> Result<Vec<PatchResult>> {
    apply_patches_with_selection(ctx, patches, &PatchSelection::default())
}

pub fn apply_patches_with_options(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    options: &HashMap<String, PatchOptions>,
) -> Result<Vec<PatchResult>> {
    let mut selection = PatchSelection::default();
    for (patch, patch_options) in options {
        selection.set_patch_options(patch.clone(), patch_options.clone());
    }
    apply_patches_with_selection(ctx, patches, &selection)
}

pub fn apply_patches_with_selection(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    selection: &PatchSelection,
) -> Result<Vec<PatchResult>> {
    apply_patches_with_selection_and_observer(ctx, patches, selection, |_| {})
}

pub fn apply_patches_with_plan(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    plan: &ExecutionPlan,
) -> Result<Vec<PatchResult>> {
    apply_patches_with_selection(ctx, patches, plan)
}

pub fn apply_patches_with_selection_and_observer<F>(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    selection: &PatchSelection,
    mut observer: F,
) -> Result<Vec<PatchResult>>
where
    F: FnMut(ProgressEvent),
{
    info!(
        patch_count = patches.len(),
        configured_patches = selection.options.len(),
        selected_patches = selection.selected.len(),
        disabled_patches = selection.disabled.len(),
        "starting patch application"
    );

    let resolved = resolve_patch_selection(patches, selection)?;
    let package = ctx.package_name().map(str::to_owned);
    let version = ctx.version_name().map(str::to_owned);

    let mut results = Vec::with_capacity(patches.len());
    let mut applied = vec![false; patches.len()];
    let mut after_dependents_fired = vec![false; patches.len()];
    let mut result_map: HashMap<usize, usize> = HashMap::with_capacity(patches.len());
    let mut merged_extensions = HashSet::new();

    for &idx in resolved.order() {
        let patch = &patches[idx];
        let patch_name = patch.name();
        let patch_span = info_span!("patch", patch = patch_name);
        let _patch_guard = patch_span.enter();

        if !resolved.is_desired(idx) {
            debug!("patch skipped because it is not selected");
            let result = skipped_result(patch_name, "not selected");
            observer(ProgressEvent::PatchFinished {
                patch: result.name.clone(),
                status: result.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if resolved.is_disabled(idx) {
            warn!("patch skipped because it is explicitly disabled");
            let result = skipped_result(patch_name, "disabled explicitly");
            observer(ProgressEvent::PatchFinished {
                patch: result.name.clone(),
                status: result.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if let Some(reason) =
            dependency_skip_reason(patches, &resolved, &applied, &result_map, &results, idx)
        {
            warn!(reason, "patch skipped due to dependency state");
            let result = PatchResult {
                name: patch_name.to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            observer(ProgressEvent::PatchFinished {
                patch: result.name.clone(),
                status: result.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if let Some(reason) = patch
            .spec()
            .compatibility_reason(package.as_deref(), version.as_deref())
        {
            warn!(reason, "patch skipped due to compatibility check");
            let result = PatchResult {
                name: patch_name.to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            observer(ProgressEvent::PatchFinished {
                patch: result.name.clone(),
                status: result.status.clone(),
            });
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if let Some(options) = resolved.options_for(idx) {
            ctx.set_options(options.clone());
        } else {
            ctx.clear_options();
        }

        ctx.set_log(PatchLog::new(patch_name.to_owned()));
        observer(ProgressEvent::PatchStarted {
            patch: patch_name.to_owned(),
        });

        let extension_paths = patch.extension_dex();
        if !extension_paths.is_empty() {
            debug!(
                extension_count = extension_paths.len(),
                "patch declares extension DEX files"
            );
            let new_paths: Vec<&Path> = extension_paths
                .iter()
                .filter(|path| merged_extensions.insert((*path).clone()))
                .map(|path| path.as_path())
                .collect();
            if !new_paths.is_empty() {
                if let Err(error) = ctx.merge_extension_dex(&new_paths) {
                    warn!(error = %error, "failed to merge patch extension DEX");
                    let result = PatchResult {
                        name: patch_name.to_owned(),
                        status: PatchStatus::Failed {
                            reason: format!("extension merge: {error}"),
                        },
                        logs: ctx.take_log_entries(),
                    };
                    for log in &result.logs {
                        observer(ProgressEvent::PatchLog(log.clone()));
                    }
                    observer(ProgressEvent::PatchFinished {
                        patch: result.name.clone(),
                        status: result.status.clone(),
                    });
                    result_map.insert(idx, results.len());
                    results.push(result);
                    continue;
                }
            }
        }

        let execution_result = panic::catch_unwind(AssertUnwindSafe(|| patch.execute(ctx)));

        let logs = ctx.take_log_entries();
        for log in &logs {
            observer(ProgressEvent::PatchLog(log.clone()));
        }

        match execution_result {
            Ok(Ok(())) => {
                info!("patch applied successfully");
                applied[idx] = true;
                let result = PatchResult {
                    name: patch_name.to_owned(),
                    status: PatchStatus::Applied,
                    logs,
                };
                observer(ProgressEvent::PatchFinished {
                    patch: result.name.clone(),
                    status: result.status.clone(),
                });
                result_map.insert(idx, results.len());
                results.push(result);
            }
            Ok(Err(error)) => {
                warn!(error = %error, "patch execution returned an error");
                let result = PatchResult {
                    name: patch_name.to_owned(),
                    status: PatchStatus::Failed {
                        reason: error.to_string(),
                    },
                    logs,
                };
                observer(ProgressEvent::PatchFinished {
                    patch: result.name.clone(),
                    status: result.status.clone(),
                });
                result_map.insert(idx, results.len());
                results.push(result);
            }
            Err(panic_info) => {
                let reason = panic_reason(panic_info);
                warn!(panic = %reason, "patch panicked during execution");
                let result = PatchResult {
                    name: patch_name.to_owned(),
                    status: PatchStatus::Failed {
                        reason: format!("panic: {reason}"),
                    },
                    logs,
                };
                observer(ProgressEvent::PatchFinished {
                    patch: result.name.clone(),
                    status: result.status.clone(),
                });
                result_map.insert(idx, results.len());
                results.push(result);
            }
        }
    }

    for (idx, dependents) in resolved.dependents.iter().enumerate() {
        if dependents.is_empty() || after_dependents_fired[idx] {
            continue;
        }

        let Some(&result_idx) = result_map.get(&idx) else {
            continue;
        };
        if !matches!(results[result_idx].status, PatchStatus::Applied) {
            continue;
        }

        after_dependents_fired[idx] = true;
        let patch = &patches[idx];
        ctx.set_log(PatchLog::new(patch.name().to_owned()));
        debug!(patch = patch.name(), "running after_dependents hook");

        let after_result = panic::catch_unwind(AssertUnwindSafe(|| patch.after_dependents(ctx)));

        let after_logs = ctx.take_log_entries();
        for log in &after_logs {
            observer(ProgressEvent::PatchLog(log.clone()));
        }
        results[result_idx].logs.extend(after_logs);

        match after_result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(patch = patch.name(), error = %error, "after_dependents hook failed");
                results[result_idx].status = PatchStatus::Failed {
                    reason: format!("after_dependents: {error}"),
                };
                applied[idx] = false;
                observer(ProgressEvent::PatchFinished {
                    patch: results[result_idx].name.clone(),
                    status: results[result_idx].status.clone(),
                });
            }
            Err(panic_info) => {
                let reason = panic_reason(panic_info);
                warn!(
                    patch = patch.name(),
                    panic = %reason,
                    "after_dependents hook panicked"
                );
                results[result_idx].status = PatchStatus::Failed {
                    reason: format!("after_dependents panic: {reason}"),
                };
                applied[idx] = false;
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

pub fn apply_patches_with_plan_and_observer<F>(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    plan: &ExecutionPlan,
    observer: F,
) -> Result<Vec<PatchResult>>
where
    F: FnMut(ProgressEvent),
{
    apply_patches_with_selection_and_observer(ctx, patches, plan, observer)
}

pub fn validate_patches_with_selection(
    patches: &[Box<dyn Patch>],
    selection: &PatchSelection,
    package: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<PatchResult>> {
    info!(
        patch_count = patches.len(),
        configured_patches = selection.options.len(),
        selected_patches = selection.selected.len(),
        disabled_patches = selection.disabled.len(),
        "starting patch validation"
    );

    let resolved = resolve_patch_selection(patches, selection)?;
    let mut results = Vec::with_capacity(patches.len());
    let mut applied = vec![false; patches.len()];
    let mut result_map: HashMap<usize, usize> = HashMap::with_capacity(patches.len());

    for &idx in resolved.order() {
        let patch = &patches[idx];
        let patch_name = patch.name();

        if !resolved.is_desired(idx) {
            let result = skipped_result(patch_name, "not selected");
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if resolved.is_disabled(idx) {
            let result = skipped_result(patch_name, "disabled explicitly");
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if let Some(reason) =
            dependency_skip_reason(patches, &resolved, &applied, &result_map, &results, idx)
        {
            let result = PatchResult {
                name: patch_name.to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        if let Some(reason) = patch.spec().compatibility_reason(package, version) {
            let result = PatchResult {
                name: patch_name.to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(result);
            continue;
        }

        applied[idx] = true;
        let result = PatchResult {
            name: patch_name.to_owned(),
            status: PatchStatus::Applied,
            logs: Vec::new(),
        };
        result_map.insert(idx, results.len());
        results.push(result);
    }

    info!(result_count = results.len(), "patch validation finished");
    Ok(results)
}

pub fn validate_patches_with_plan(
    patches: &[Box<dyn Patch>],
    plan: &ExecutionPlan,
    package: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<PatchResult>> {
    validate_patches_with_selection(patches, plan, package, version)
}

fn dependency_skip_reason(
    patches: &[Box<dyn Patch>],
    resolved: &ResolvedPatchPlan,
    applied: &[bool],
    result_map: &HashMap<usize, usize>,
    results: &[PatchResult],
    idx: usize,
) -> Option<String> {
    for &dependency_idx in resolved.dependencies_for(idx) {
        if applied[dependency_idx] {
            continue;
        }

        let dependency_result = result_map
            .get(&dependency_idx)
            .and_then(|result_idx| results.get(*result_idx));
        let dependency_name = patches[dependency_idx].name();
        let detail = match dependency_result {
            Some(result) => match &result.status {
                PatchStatus::Applied => continue,
                PatchStatus::Skipped { reason } => format!("skipped: {reason}"),
                PatchStatus::Failed { reason } => format!("failed: {reason}"),
            },
            None => "was not executed".to_owned(),
        };
        return Some(format!("dependency '{dependency_name}' {detail}"));
    }

    None
}

fn skipped_result(patch_name: &str, reason: &str) -> PatchResult {
    PatchResult {
        name: patch_name.to_owned(),
        status: PatchStatus::Skipped {
            reason: reason.to_owned(),
        },
        logs: Vec::new(),
    }
}

fn panic_reason(panic_info: Box<dyn std::any::Any + Send>) -> String {
    if let Some(reason) = panic_info.downcast_ref::<String>() {
        reason.clone()
    } else if let Some(reason) = panic_info.downcast_ref::<&str>() {
        reason.to_string()
    } else {
        "unknown panic".to_owned()
    }
}
