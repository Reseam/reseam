// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeSet, HashMap};

use reseam_model::{
    AppIdentity, InstallMethod, OptionIssue, PatchRef, PatchSelection, PatchSpec, PatchStatus,
    PlanRequest, PlannedPatch, PlannedRun, SelectionPlan,
};
use reseam_patcher::catalog::PatchCatalog;
use reseam_patcher::engine::{self, PlanOptions, ResolvedPlan};
use reseam_patcher::error::PatcherError;

use crate::error::Result;

/// Plans `request` as a run would resolve it, from patch metadata alone, so a
/// host can show what a selection does before it has the APK. Selection
/// entries that name no patch are reported instead of failing, and options for
/// a patch the plan does not run are ignored; every other problem a run rejects
/// the selection for fails here too.
pub fn plan(request: &PlanRequest) -> Result<SelectionPlan> {
    let specs: Vec<&PatchSpec> = request.patches.iter().map(|patch| &patch.spec).collect();
    plan_specs(
        &specs,
        &request.app,
        &request.selection,
        request.install_method,
    )
}

/// [`plan`] over the catalog `specs`.
pub(crate) fn plan_specs(
    specs: &[&PatchSpec],
    app: &AppIdentity,
    selection: &PatchSelection,
    install_method: InstallMethod,
) -> Result<SelectionPlan> {
    let package = app.package_name.as_deref();
    let (selection, unknown) = known_selection(specs, selection, package)?;
    let plan = ResolvedPlan::resolve(
        specs,
        &selection,
        install_method.into(),
        package,
        app.version(),
    )?;
    let mut problems = PlanOptions::problems(specs, &selection, &plan, package)?;
    let mut results: HashMap<PatchRef, (PatchStatus, Vec<PatchRef>)> =
        engine::validate_patches(specs, &plan)
            .into_iter()
            .map(|result| (result.patch, (result.status, result.required_by)))
            .collect();
    let patches = specs
        .iter()
        .enumerate()
        .filter_map(|(idx, spec)| {
            let reference = spec.reference();
            let result = results.remove(&reference);
            if result.is_none() && !spec.listed_for(package) {
                return None;
            }
            // A patch the selection turns off is off, whatever the run would
            // report for it.
            let disabled = plan.is_disabled(idx);
            let (run, required_by) = match result {
                Some((status, required_by)) if !disabled => (planned_run(status), required_by),
                _ => (PlannedRun::Off, Vec::new()),
            };
            Some(PlannedPatch {
                patch: reference,
                incompatibility: spec.compatibility.incompatibility(package, app.version()),
                presets: spec.presets(package),
                selected: plan.is_selected(idx) && !disabled,
                run,
                required_by,
                option_problems: std::mem::take(&mut problems[idx])
                    .into_iter()
                    .map(|(key, problem)| OptionIssue { key, problem })
                    .collect(),
            })
        })
        .collect();
    Ok(SelectionPlan { patches, unknown })
}

/// `selection` without the entries that name no patch, and those entries.
fn known_selection(
    specs: &[&PatchSpec],
    selection: &PatchSelection,
    package: Option<&str>,
) -> Result<(PatchSelection, Vec<String>)> {
    let catalog = PatchCatalog::new(specs)?;
    let mut unknown = BTreeSet::new();
    for selector in selection
        .enable
        .iter()
        .chain(&selection.disable)
        .chain(selection.options.keys())
    {
        match catalog.resolve(selector, package) {
            Ok(_) => {}
            Err(PatcherError::UnknownPatch(_)) => {
                unknown.insert(selector.clone());
            }
            Err(error) => return Err(error.into()),
        }
    }
    let known = |selectors: &[String]| {
        selectors
            .iter()
            .filter(|selector| !unknown.contains(*selector))
            .cloned()
            .collect()
    };
    let known_selection = PatchSelection {
        enable: known(&selection.enable),
        disable: known(&selection.disable),
        options: selection
            .options
            .iter()
            .filter(|(selector, _)| !unknown.contains(*selector))
            .map(|(selector, options)| (selector.clone(), options.clone()))
            .collect(),
        ..selection.clone()
    };
    Ok((known_selection, unknown.into_iter().collect()))
}

fn planned_run(status: PatchStatus) -> PlannedRun {
    match status {
        PatchStatus::Applied => PlannedRun::Runs,
        PatchStatus::Skipped { reason } => PlannedRun::Skipped { reason },
        PatchStatus::Unmountable { reason } => PlannedRun::Unmountable { reason },
        PatchStatus::Failed { .. } => unreachable!("validation runs no patch, so none fails"),
    }
}
