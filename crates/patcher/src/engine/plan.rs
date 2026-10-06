// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet, VecDeque};

use super::PatchIndex;

use crate::error::{PatcherError, Result};
use crate::options::PatchOptions;
use crate::patch::Patch;

pub use reseam_model::PatchSelection;

#[derive(Debug)]
pub(crate) struct ResolvedPlan {
    order: Vec<usize>,
    finalizers: Vec<usize>,
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    selected: Vec<bool>,
    desired: Vec<bool>,
    disabled: Vec<bool>,
    unmountable: Vec<bool>,
    unavailable: Vec<Option<String>>,
    options: Vec<PatchOptions>,
    ignore_versions: bool,
}

impl ResolvedPlan {
    /// `unmountable` names patches an earlier mount build found changing the
    /// manifest. They stay selected so the run can report them, but neither
    /// they, the patches that need them, nor dependencies only those need will run.
    pub fn resolve(
        patches: &[&Patch],
        selection: &PatchSelection,
        unmountable: &[String],
        package: Option<&str>,
        version: Option<&str>,
    ) -> Result<Self> {
        let specs: Vec<_> = patches.iter().map(|patch| patch.spec()).collect();
        let index = PatchIndex::new(&specs)?;
        let DependencyGraph {
            dependencies,
            dependents,
            missing,
        } = dependency_edges(patches, &index);
        let order = topological_order(patches, &dependencies, &dependents)?;
        let finalizers = topological_order(patches, &dependents, &dependencies)?;

        let lookup = |patch: &String| index.resolve(patch, package);
        let enabled: HashSet<usize> = selection.enable.iter().map(lookup).collect::<Result<_>>()?;
        for &idx in &enabled {
            if let Some(missing) = &missing[idx] {
                return Err(missing.error(patches[idx].reference().to_owned()));
            }
        }
        let unavailable: Vec<Option<String>> = missing
            .iter()
            .map(|missing| missing.as_ref().map(MissingDependency::skip_reason))
            .collect();
        let roots: Vec<usize> = (0..patches.len())
            .filter(|&i| {
                enabled.contains(&i) || patches[i].spec().in_preset(selection.preset, package)
            })
            .collect();
        let mut selected = vec![false; patches.len()];
        for &idx in &roots {
            selected[idx] = true;
        }

        let mut disabled = vec![false; patches.len()];
        for patch in &selection.disable {
            let idx = lookup(patch)?;
            if enabled.contains(&idx) {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch '{patch}' cannot be both selected and disabled"
                )));
            }
            disabled[idx] = true;
        }

        let mut excluded = vec![false; patches.len()];
        for patch in unmountable {
            excluded[lookup(patch)?] = true;
        }
        let mut needs_excluded = excluded.clone();
        for &idx in &order {
            needs_excluded[idx] |= dependencies[idx]
                .iter()
                .any(|&dependency| needs_excluded[dependency]);
        }

        // A dependency is desired only while an applicable, enabled patch
        // needs it. Selection roots remain desired so validation can report
        // why they cannot run, but incompatibility or explicit disabling
        // stops their dependency chain from propagating further.
        let can_propagate = |idx: usize| {
            if disabled[idx] || needs_excluded[idx] || unavailable[idx].is_some() {
                return false;
            }
            let spec = patches[idx].spec();
            if selection.ignore_versions {
                spec.package_incompatibility(package).is_none()
            } else {
                spec.incompatibility(package, version).is_none()
            }
        };
        let mut desired = selected.clone();
        let mut expanded = vec![false; patches.len()];
        let mut stack: Vec<usize> = roots
            .iter()
            .copied()
            .filter(|&idx| can_propagate(idx))
            .collect();
        while let Some(idx) = stack.pop() {
            if std::mem::replace(&mut expanded[idx], true) {
                continue;
            }
            for &dependency in &dependencies[idx] {
                desired[dependency] = true;
                if can_propagate(dependency) {
                    stack.push(dependency);
                }
            }
        }

        let options = resolve_options(patches, selection, &index, package, &desired, &disabled)?;

        Ok(Self {
            order,
            finalizers,
            dependencies,
            dependents,
            selected,
            desired,
            disabled,
            unmountable: excluded,
            unavailable,
            options,
            ignore_versions: selection.ignore_versions,
        })
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn dependencies(&self, idx: usize) -> &[usize] {
        &self.dependencies[idx]
    }

    pub fn finalizers(&self) -> &[usize] {
        &self.finalizers
    }

    pub fn is_desired(&self, idx: usize) -> bool {
        self.desired[idx]
    }

    pub fn required_by(&self, idx: usize) -> impl Iterator<Item = usize> + '_ {
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

    pub fn is_disabled(&self, idx: usize) -> bool {
        self.disabled[idx]
    }

    pub fn is_unmountable(&self, idx: usize) -> bool {
        self.unmountable[idx]
    }

    pub fn unavailable(&self, idx: usize) -> Option<&str> {
        self.unavailable[idx].as_deref()
    }

    pub fn options(&self, idx: usize) -> &PatchOptions {
        &self.options[idx]
    }

    pub fn ignores_versions(&self) -> bool {
        self.ignore_versions
    }
}

fn resolve_options(
    patches: &[&Patch],
    selection: &PatchSelection,
    index: &PatchIndex<'_>,
    package: Option<&str>,
    desired: &[bool],
    disabled: &[bool],
) -> Result<Vec<PatchOptions>> {
    let mut configured = HashMap::new();
    for (patch, options) in &selection.options {
        let idx = index.resolve(patch, package)?;
        if configured
            .insert(idx, PatchOptions::from(options.clone()))
            .is_some()
        {
            return Err(PatcherError::InvalidSelection(format!(
                "options for patch '{}' were supplied under multiple selectors",
                patches[idx].reference()
            )));
        }
        if !desired[idx] || disabled[idx] {
            return Err(PatcherError::InvalidSelection(format!(
                "patch '{patch}' has options configured but is not enabled by the selection"
            )));
        }
    }
    patches
        .iter()
        .enumerate()
        .map(|(idx, patch)| {
            if !desired[idx] || disabled[idx] {
                return Ok(PatchOptions::default());
            }
            PatchOptions::resolve(
                patch.reference(),
                &patch.spec().options,
                configured.get(&idx),
            )
        })
        .collect()
}

struct MissingDependency {
    dependency: String,
    bundle: String,
    availability: BundleAvailability,
}

enum BundleAvailability {
    Loaded,
    Missing,
}

impl MissingDependency {
    fn error(&self, patch: String) -> PatcherError {
        let (dependency, bundle) = (self.dependency.clone(), self.bundle.clone());
        if matches!(self.availability, BundleAvailability::Loaded) {
            PatcherError::MissingDependency {
                patch,
                dependency,
                bundle,
            }
        } else {
            PatcherError::MissingBundle {
                patch,
                dependency,
                bundle,
            }
        }
    }

    fn skip_reason(&self) -> String {
        if matches!(self.availability, BundleAvailability::Loaded) {
            format!(
                "depends on {}, which bundle '{}' does not declare",
                self.dependency, self.bundle
            )
        } else {
            format!(
                "depends on {}; load bundle '{}' alongside",
                self.dependency, self.bundle
            )
        }
    }
}

struct DependencyGraph {
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    missing: Vec<Option<MissingDependency>>,
}

fn dependency_edges(patches: &[&Patch], index: &PatchIndex<'_>) -> DependencyGraph {
    let mut dependencies = vec![Vec::new(); patches.len()];
    let mut dependents = vec![Vec::new(); patches.len()];
    let mut missing: Vec<Option<MissingDependency>> = (0..patches.len()).map(|_| None).collect();
    for (idx, patch) in patches.iter().enumerate() {
        for dependency in &patch.spec().dependencies {
            let Some(dependency_idx) = index.reference(dependency) else {
                let bundle = dependency.split_once('/').map_or("", |(bundle, _)| bundle);
                missing[idx].get_or_insert_with(|| MissingDependency {
                    dependency: dependency.clone(),
                    bundle: bundle.to_owned(),
                    availability: if patches.iter().any(|patch| patch.spec().bundle == bundle) {
                        BundleAvailability::Loaded
                    } else {
                        BundleAvailability::Missing
                    },
                });
                continue;
            };
            dependencies[idx].push(dependency_idx);
            dependents[dependency_idx].push(idx);
        }
    }
    DependencyGraph {
        dependencies,
        dependents,
        missing,
    }
}

fn topological_order(
    patches: &[&Patch],
    dependencies: &[Vec<usize>],
    dependents: &[Vec<usize>],
) -> Result<Vec<usize>> {
    let mut in_degree: Vec<usize> = dependencies.iter().map(Vec::len).collect();
    let mut queue: VecDeque<usize> = (0..patches.len()).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(patches.len());
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for &dependent in &dependents[idx] {
            in_degree[dependent] -= 1;
            if in_degree[dependent] == 0 {
                queue.push_back(dependent);
            }
        }
    }
    if order.len() != patches.len() {
        let names = (0..patches.len())
            .filter(|&i| in_degree[i] > 0)
            .map(|i| patches[i].reference().to_owned())
            .collect();
        return Err(PatcherError::DependencyCycle(names));
    }
    Ok(order)
}
