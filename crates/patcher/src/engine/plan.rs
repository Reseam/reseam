// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet, VecDeque};

use serde::Deserialize;

use super::PatchIndex;

use crate::error::{PatcherError, Result};
use crate::options::PatchOptions;
use crate::patch::Patch;

/// What the caller asked for: an empty `enable` set means every patch that
/// is enabled by default.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PatchSelection {
    pub enable: HashSet<String>,
    pub disable: HashSet<String>,
    pub options: HashMap<String, PatchOptions>,
    /// Run patches on app versions they were not declared for. The package
    /// check still applies.
    pub ignore_versions: bool,
}

/// A selection checked against a patch list: dependency order, the patches
/// to run, and their validated options. Indices are into the patch list.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPlan {
    order: Vec<usize>,
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    /// Asked for directly, rather than pulled in as someone's dependency.
    selected: Vec<bool>,
    desired: Vec<bool>,
    disabled: Vec<bool>,
    options: Vec<PatchOptions>,
    ignore_versions: bool,
}

impl ResolvedPlan {
    pub fn resolve(
        patches: &[&dyn Patch],
        selection: &PatchSelection,
        package: Option<&str>,
    ) -> Result<Self> {
        let index = PatchIndex::new(patches)?;
        let (dependencies, dependents) = dependency_edges(patches, &index.ids)?;
        let order = topological_order(patches, &dependencies, &dependents)?;

        let lookup = |patch: &String| index.resolve(patch, package);
        let enabled: HashSet<usize> = selection.enable.iter().map(lookup).collect::<Result<_>>()?;
        let mut desired = vec![false; patches.len()];
        let mut stack: Vec<usize> = if selection.enable.is_empty() {
            (0..patches.len())
                .filter(|&i| patches[i].spec().enabled_by_default)
                .collect()
        } else {
            enabled.iter().copied().collect()
        };
        let mut selected = vec![false; patches.len()];
        for &idx in &stack {
            selected[idx] = true;
        }
        while let Some(idx) = stack.pop() {
            if !std::mem::replace(&mut desired[idx], true) {
                stack.extend(&dependencies[idx]);
            }
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

        let mut configured = HashMap::new();
        for (patch, options) in &selection.options {
            let idx = lookup(patch)?;
            if configured.insert(idx, options).is_some() {
                return Err(PatcherError::InvalidSelection(format!(
                    "options for patch '{}' were supplied under multiple selectors",
                    patches[idx].id()
                )));
            }
            if !desired[idx] || disabled[idx] {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch '{patch}' has options configured but is not enabled by the selection"
                )));
            }
        }
        let options = patches
            .iter()
            .enumerate()
            .map(|(idx, patch)| {
                if !desired[idx] || disabled[idx] {
                    return Ok(PatchOptions::default());
                }
                PatchOptions::resolve(
                    patch.id(),
                    &patch.spec().options,
                    configured.get(&idx).copied(),
                )
            })
            .collect::<Result<_>>()?;

        Ok(Self {
            order,
            dependencies,
            dependents,
            selected,
            desired,
            disabled,
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

    pub fn dependents(&self, idx: usize) -> &[usize] {
        &self.dependents[idx]
    }

    pub fn is_desired(&self, idx: usize) -> bool {
        self.desired[idx]
    }

    /// The running patches that pulled `idx` in, empty when it was asked for
    /// directly. Only direct dependents: a longer chain is noise to a reader.
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

    pub fn options(&self, idx: usize) -> &PatchOptions {
        &self.options[idx]
    }

    pub fn ignores_versions(&self) -> bool {
        self.ignore_versions
    }
}

type Edges = (Vec<Vec<usize>>, Vec<Vec<usize>>);

fn dependency_edges(patches: &[&dyn Patch], index: &HashMap<&str, usize>) -> Result<Edges> {
    let mut dependencies = vec![Vec::new(); patches.len()];
    let mut dependents = vec![Vec::new(); patches.len()];
    for (idx, patch) in patches.iter().enumerate() {
        for dependency in &patch.spec().dependencies {
            let Some(&dependency_idx) = index.get(dependency.as_str()) else {
                return Err(PatcherError::MissingDependency {
                    patch: patch.id().to_owned(),
                    dependency: dependency.clone(),
                });
            };
            dependencies[idx].push(dependency_idx);
            dependents[dependency_idx].push(idx);
        }
    }
    Ok((dependencies, dependents))
}

fn topological_order(
    patches: &[&dyn Patch],
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
            .map(|i| patches[i].id().to_owned())
            .collect();
        return Err(PatcherError::DependencyCycle(names));
    }
    Ok(order)
}
