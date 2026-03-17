use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use crate::context::PatchContext;
use crate::dependency;
use crate::error::Result;
use crate::log::{LogEntry, PatchLog};
use crate::options::PatchOptions;
use crate::patch::Patch;

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

pub fn apply_patches(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
) -> Result<Vec<PatchResult>> {
    apply_patches_with_options(ctx, patches, &HashMap::new())
}

pub fn apply_patches_with_options(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
    options: &HashMap<String, PatchOptions>,
) -> Result<Vec<PatchResult>> {
    let order = dependency::sort_patches(patches)?;
    let dependents = dependency::find_dependents(patches);

    let package = ctx.package_name().map(|s| s.to_owned());
    let version = ctx.version_name().map(|s| s.to_owned());

    let mut results = Vec::with_capacity(patches.len());
    let mut applied: Vec<bool> = vec![false; patches.len()];
    let mut result_map: HashMap<usize, usize> = HashMap::new();

    for &idx in &order {
        let patch = &patches[idx];

        if let Some(reason) = check_compatibility(patch.as_ref(), &package, &version) {
            let r = PatchResult {
                name: patch.name().to_owned(),
                status: PatchStatus::Skipped { reason },
                logs: Vec::new(),
            };
            result_map.insert(idx, results.len());
            results.push(r);
            continue;
        }

        if let Some(opts) = options.get(patch.name()) {
            ctx.set_options(opts.clone());
        } else {
            ctx.clear_options();
        }

        ctx.set_log(PatchLog::new(patch.name().to_owned()));

        let exec_result = panic::catch_unwind(AssertUnwindSafe(|| patch.execute(ctx)));

        let logs = ctx.take_log_entries();

        match exec_result {
            Ok(Ok(())) => {
                applied[idx] = true;
                let r = PatchResult {
                    name: patch.name().to_owned(),
                    status: PatchStatus::Applied,
                    logs,
                };
                result_map.insert(idx, results.len());
                results.push(r);
            }
            Ok(Err(e)) => {
                let r = PatchResult {
                    name: patch.name().to_owned(),
                    status: PatchStatus::Failed {
                        reason: e.to_string(),
                    },
                    logs,
                };
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
                let r = PatchResult {
                    name: patch.name().to_owned(),
                    status: PatchStatus::Failed {
                        reason: format!("panic: {reason}"),
                    },
                    logs,
                };
                result_map.insert(idx, results.len());
                results.push(r);
                continue;
            }
        }

        for (&dep_idx, dep_list) in &dependents {
            if applied[dep_idx] {
                continue;
            }
            if dep_list.iter().all(|&d| applied[d]) {
                ctx.set_log(PatchLog::new(patches[dep_idx].name().to_owned()));

                let after_result = panic::catch_unwind(AssertUnwindSafe(|| {
                    patches[dep_idx].after_dependents(ctx)
                }));

                let after_logs = ctx.take_log_entries();
                if let Some(&ri) = result_map.get(&dep_idx) {
                    results[ri].logs.extend(after_logs);
                }

                if let Ok(Err(e)) = after_result {
                    if let Some(&ri) = result_map.get(&dep_idx) {
                        results[ri].status = PatchStatus::Failed {
                            reason: format!("after_dependents: {e}"),
                        };
                    }
                }
            }
        }
    }

    Ok(results)
}

fn check_compatibility(
    patch: &dyn Patch,
    package: &Option<String>,
    version: &Option<String>,
) -> Option<String> {
    let compat = patch.compatible_with();
    if compat.is_empty() {
        return None;
    }

    let pkg = match package {
        Some(pkg) => pkg,
        None => return Some("APK has no package name".to_owned()),
    };

    let matching = compat.iter().find(|c| c.package == *pkg);
    let entry = match matching {
        Some(e) => e,
        None => {
            return Some(format!("incompatible package: {pkg}"));
        }
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
