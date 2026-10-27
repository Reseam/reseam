// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use super::PatchStatus;
use crate::catalog::PatchCatalog;

use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::options::PatchOptions;
use crate::patch::{ApkVersion, PatchSpec};

pub use reseam_model::PatchSelection;

mod graph;
mod options;

use graph::{
    DependencyGraph, MissingDependency, dependency_edges, desired_patches, topological_order,
};
use options::resolve_options;

/// How a run's output reaches the device, which decides what a patch may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The output installs as an app of its own.
    Install,
    /// The output replaces the installed app's files, and the system keeps the
    /// manifest it read from the installed app. A patch that edits the manifest
    /// finishes unmountable: the run undoes it and leaves out the patches that need it.
    /// A patch that rewrites the output is unmountable before it runs.
    Mount,
}

impl Delivery {
    /// Whether the manifest changed since `revision` in a build that cannot apply it.
    pub(super) fn forbids_manifest_edit(self, revision: u64, ctx: &PatchContext<'_>) -> bool {
        self == Self::Mount && ctx.apk().manifest_revision() != revision
    }
}

pub(super) const CHANGES_MANIFEST: &str = "changes the manifest, which a mount does not apply";
const REWRITES_OUTPUT: &str = "rewrites the written APK, which a mount cannot check";

pub(super) fn unmountable_status() -> PatchStatus {
    PatchStatus::Unmountable {
        reason: CHANGES_MANIFEST.to_owned(),
    }
}

/// What a run of `specs` covers, decided from the patch declarations alone, so
/// a host can load only the code of the patches it [executes](Self::executes).
/// Indices are positions in the `specs` the plan was resolved from.
#[derive(Debug)]
pub struct ResolvedPlan {
    /// Every patch, dependencies first.
    topological: Vec<usize>,
    /// Every patch, dependents first.
    reverse_topological: Vec<usize>,
    order: Vec<usize>,
    finalizers: Vec<usize>,
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    roots: Vec<usize>,
    selected: Vec<bool>,
    desired: Vec<bool>,
    precluded: Vec<Option<PatchStatus>>,
    excluded: Vec<bool>,
    executable: Vec<bool>,
    options: Vec<PatchOptions>,
    delivery: Delivery,
}

impl ResolvedPlan {
    pub fn resolve(
        specs: &[&PatchSpec],
        selection: &PatchSelection,
        delivery: Delivery,
        package: Option<&str>,
        version: ApkVersion<'_>,
    ) -> Result<Self> {
        let index = PatchCatalog::new(specs)?;
        let DependencyGraph {
            dependencies,
            dependents,
            missing,
        } = dependency_edges(specs, &index);
        let order = topological_order(specs, &dependencies, &dependents)?;
        let finalizers = topological_order(specs, &dependents, &dependencies)?;

        let lookup = |patch: &String| index.resolve(patch, package);
        let enabled: HashSet<usize> = selection.enable.iter().map(lookup).collect::<Result<_>>()?;
        for &idx in &enabled {
            if let Some(missing) = &missing[idx] {
                return Err(missing.error(&specs[idx].reference));
            }
        }
        let unavailable: Vec<Option<String>> = missing
            .iter()
            .map(|missing| missing.as_ref().map(MissingDependency::skip_reason))
            .collect();
        let roots: Vec<usize> = (0..specs.len())
            .filter(|&i| enabled.contains(&i) || specs[i].in_preset(selection.preset, package))
            .collect();
        let mut selected = vec![false; specs.len()];
        for &idx in &roots {
            selected[idx] = true;
        }

        let mut disabled = vec![false; specs.len()];
        for patch in &selection.disable {
            let idx = lookup(patch)?;
            if enabled.contains(&idx) {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch '{patch}' cannot be both selected and disabled"
                )));
            }
            disabled[idx] = true;
        }

        let precluded: Vec<Option<PatchStatus>> = specs
            .iter()
            .enumerate()
            .map(|(idx, spec)| {
                let reason = if disabled[idx] {
                    Some("disabled explicitly".to_owned())
                } else if let Some(reason) = &unavailable[idx] {
                    Some(reason.clone())
                } else if selection.ignore_versions {
                    spec.compatibility
                        .package_incompatibility(package)
                        .map(|reason| reason.to_string())
                } else {
                    spec.compatibility
                        .incompatibility(package, version)
                        .map(|reason| reason.to_string())
                };
                match reason {
                    Some(reason) => Some(PatchStatus::Skipped { reason }),
                    None if delivery == Delivery::Mount && spec.rewrites_output => {
                        Some(PatchStatus::Unmountable {
                            reason: REWRITES_OUTPUT.to_owned(),
                        })
                    }
                    None => None,
                }
            })
            .collect();

        let mut plan = Self {
            topological: order,
            reverse_topological: finalizers,
            order: Vec::new(),
            finalizers: Vec::new(),
            dependencies,
            dependents,
            roots,
            selected,
            desired: Vec::new(),
            precluded,
            excluded: vec![false; specs.len()],
            executable: Vec::new(),
            options: Vec::new(),
            delivery,
        };
        plan.refresh();
        plan.options =
            resolve_options(specs, selection, &index, package, &plan.desired, &disabled)?;
        Ok(plan)
    }

    /// Leaves out the patch at `idx`, which ended with `status` instead of
    /// applying. Patches that need it no longer run, nor do dependencies only
    /// those needed; selected patches stay covered so the run reports them.
    pub(crate) fn exclude(&mut self, idx: usize, status: PatchStatus) {
        self.precluded[idx] = Some(status);
        self.excluded[idx] = true;
        self.refresh();
    }

    fn refresh(&mut self) {
        let mut blocked: Vec<bool> = self.precluded.iter().map(Option::is_some).collect();
        for &idx in &self.topological {
            blocked[idx] |= self.dependencies[idx]
                .iter()
                .any(|&dependency| blocked[dependency]);
        }
        self.desired = desired_patches(
            &self.roots,
            &self.selected,
            &self.dependencies,
            &self.precluded,
            &blocked,
        );
        // An excluded patch keeps the result that says why it was left out.
        for (desired, &excluded) in self.desired.iter_mut().zip(&self.excluded) {
            *desired |= excluded;
        }
        self.order = self
            .topological
            .iter()
            .copied()
            .filter(|&idx| self.desired[idx])
            .collect();
        self.finalizers = self
            .reverse_topological
            .iter()
            .copied()
            .filter(|&idx| self.desired[idx])
            .collect();
        self.executable = vec![false; self.desired.len()];
        for &idx in &self.order {
            self.executable[idx] = self.precluded[idx].is_none()
                && self.dependencies[idx]
                    .iter()
                    .all(|&dependency| self.executable[dependency]);
        }
    }

    /// Whether the patch at `idx` is in the selection, as opposed to only a
    /// dependency of it.
    pub(crate) fn is_selected(&self, idx: usize) -> bool {
        self.selected[idx]
    }

    pub(crate) fn is_excluded(&self, idx: usize) -> bool {
        self.excluded[idx]
    }

    /// How many patches the plan was resolved from.
    pub(crate) fn len(&self) -> usize {
        self.selected.len()
    }

    /// Whether the run may execute the patch at `idx`: it is covered, and
    /// neither it nor a dependency is ruled out before the run starts. A patch
    /// that executes can still be skipped when a dependency fails during the run.
    pub fn executes(&self, idx: usize) -> bool {
        self.executable[idx]
    }

    /// The patches this run covers, in dependency order: the selection and the dependencies it needs.
    pub(crate) fn order(&self) -> &[usize] {
        &self.order
    }

    pub(crate) fn dependencies(&self, idx: usize) -> &[usize] {
        &self.dependencies[idx]
    }

    pub(crate) fn finalizers(&self) -> &[usize] {
        &self.finalizers
    }

    pub(crate) fn required_by(&self, idx: usize) -> impl Iterator<Item = usize> + '_ {
        let dependents = if self.selected[idx] {
            &[][..]
        } else {
            &self.dependents[idx]
        };
        dependents
            .iter()
            .copied()
            .filter(|&dependent| self.desired[dependent])
    }

    /// Why the patch cannot run regardless of how its dependencies fare.
    pub(crate) fn precluded(&self, idx: usize) -> Option<&PatchStatus> {
        self.precluded[idx].as_ref()
    }

    pub(crate) fn options(&self, idx: usize) -> &PatchOptions {
        &self.options[idx]
    }

    pub(crate) fn delivery(&self) -> Delivery {
        self.delivery
    }
}
