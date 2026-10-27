// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::VecDeque;

use super::super::PatchStatus;
use crate::catalog::PatchCatalog;
use crate::error::{PatcherError, Result, Unresolved};
use crate::patch::{PatchRef, PatchSpec};

/// The selection roots and the dependencies they need. A dependency is desired
/// only while an applicable, enabled patch needs it. Selection roots remain
/// desired so validation can report why they cannot run: a patch that cannot
/// run because a dependency is left out brings in only the dependencies that
/// keep it from running, and a patch that is left out brings in none.
pub(super) fn desired_patches(
    roots: &[usize],
    selected: &[bool],
    dependencies: &[Vec<usize>],
    precluded: &[Option<PatchStatus>],
    blocked: &[bool],
) -> Vec<bool> {
    let mut desired = selected.to_vec();
    let mut expanded = vec![false; selected.len()];
    let mut stack = roots.to_vec();
    while let Some(idx) = stack.pop() {
        if precluded[idx].is_some() || std::mem::replace(&mut expanded[idx], true) {
            continue;
        }
        for &dependency in &dependencies[idx] {
            if blocked[idx] && !blocked[dependency] {
                continue;
            }
            desired[dependency] = true;
            stack.push(dependency);
        }
    }
    desired
}

pub(super) struct MissingDependency {
    dependency: PatchRef,
    availability: BundleAvailability,
}

enum BundleAvailability {
    Loaded,
    Missing,
}

impl MissingDependency {
    pub(super) fn error(&self, patch: &PatchRef) -> PatcherError {
        let unresolved = Box::new(Unresolved {
            patch: patch.clone(),
            dependency: self.dependency.clone(),
        });
        match self.availability {
            BundleAvailability::Loaded => PatcherError::MissingDependency(unresolved),
            BundleAvailability::Missing => PatcherError::MissingBundle(unresolved),
        }
    }

    pub(super) fn skip_reason(&self) -> String {
        if matches!(self.availability, BundleAvailability::Loaded) {
            format!(
                "depends on {}, which bundle '{}' does not declare",
                self.dependency, self.dependency.bundle
            )
        } else {
            format!(
                "depends on {}; load bundle '{}' alongside",
                self.dependency, self.dependency.bundle
            )
        }
    }
}

pub(super) struct DependencyGraph {
    pub(super) dependencies: Vec<Vec<usize>>,
    pub(super) dependents: Vec<Vec<usize>>,
    pub(super) missing: Vec<Option<MissingDependency>>,
}

pub(super) fn dependency_edges(specs: &[&PatchSpec], index: &PatchCatalog<'_>) -> DependencyGraph {
    let mut dependencies = vec![Vec::new(); specs.len()];
    let mut dependents = vec![Vec::new(); specs.len()];
    let mut missing: Vec<Option<MissingDependency>> = (0..specs.len()).map(|_| None).collect();
    for (idx, spec) in specs.iter().enumerate() {
        for dependency in &spec.dependencies {
            let Some(dependency_idx) = index.find(dependency) else {
                missing[idx].get_or_insert_with(|| MissingDependency {
                    dependency: dependency.clone(),
                    availability: if specs
                        .iter()
                        .any(|spec| spec.reference.bundle == dependency.bundle)
                    {
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

pub(super) fn topological_order(
    specs: &[&PatchSpec],
    dependencies: &[Vec<usize>],
    dependents: &[Vec<usize>],
) -> Result<Vec<usize>> {
    let mut in_degree: Vec<usize> = dependencies.iter().map(Vec::len).collect();
    let mut queue: VecDeque<usize> = (0..specs.len()).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(specs.len());
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for &dependent in &dependents[idx] {
            in_degree[dependent] -= 1;
            if in_degree[dependent] == 0 {
                queue.push_back(dependent);
            }
        }
    }
    if order.len() != specs.len() {
        let names = (0..specs.len())
            .filter(|&i| in_degree[i] > 0)
            .map(|i| specs[i].reference.clone())
            .collect();
        return Err(PatcherError::DependencyCycle(names));
    }
    Ok(order)
}
