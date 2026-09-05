// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet, VecDeque};

use serde::Deserialize;

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
}

/// A selection checked against a patch list: dependency order, the patches
/// to run, and their validated options. Indices are into the patch list.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPlan {
    order: Vec<usize>,
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    desired: Vec<bool>,
    disabled: Vec<bool>,
    options: Vec<PatchOptions>,
}

impl ResolvedPlan {
    pub fn resolve(patches: &[&dyn Patch], selection: &PatchSelection) -> Result<Self> {
        let index = index_by_name(patches)?;
        let (dependencies, dependents) = dependency_edges(patches, &index)?;
        let order = topological_order(patches, &dependencies, &dependents)?;

        let lookup = |patch: &String| {
            index
                .get(patch.as_str())
                .copied()
                .ok_or_else(|| PatcherError::UnknownPatch(patch.clone()))
        };
        let mut desired = vec![false; patches.len()];
        let mut stack: Vec<usize> = if selection.enable.is_empty() {
            (0..patches.len())
                .filter(|&i| patches[i].spec().enabled_by_default)
                .collect()
        } else {
            selection.enable.iter().map(lookup).collect::<Result<_>>()?
        };
        while let Some(idx) = stack.pop() {
            if !std::mem::replace(&mut desired[idx], true) {
                stack.extend(&dependencies[idx]);
            }
        }

        let mut disabled = vec![false; patches.len()];
        for patch in &selection.disable {
            let idx = lookup(patch)?;
            if selection.enable.contains(patch) {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch '{patch}' cannot be both selected and disabled"
                )));
            }
            disabled[idx] = true;
        }

        for patch in selection.options.keys() {
            let idx = lookup(patch)?;
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
                    patch.name(),
                    &patch.spec().options,
                    selection.options.get(patch.name()),
                )
            })
            .collect::<Result<_>>()?;

        Ok(Self {
            order,
            dependencies,
            dependents,
            desired,
            disabled,
            options,
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

    pub fn is_disabled(&self, idx: usize) -> bool {
        self.disabled[idx]
    }

    pub fn options(&self, idx: usize) -> &PatchOptions {
        &self.options[idx]
    }
}

fn index_by_name<'a>(patches: &[&'a dyn Patch]) -> Result<HashMap<&'a str, usize>> {
    let mut index = HashMap::with_capacity(patches.len());
    for (idx, patch) in patches.iter().enumerate() {
        if index.insert(patch.name(), idx).is_some() {
            return Err(PatcherError::InvalidSelection(format!(
                "patch name '{}' is used more than once in the bundle",
                patch.name()
            )));
        }
    }
    Ok(index)
}

type Edges = (Vec<Vec<usize>>, Vec<Vec<usize>>);

fn dependency_edges(patches: &[&dyn Patch], index: &HashMap<&str, usize>) -> Result<Edges> {
    let mut dependencies = vec![Vec::new(); patches.len()];
    let mut dependents = vec![Vec::new(); patches.len()];
    for (idx, patch) in patches.iter().enumerate() {
        for dependency in &patch.spec().dependencies {
            let Some(&dependency_idx) = index.get(dependency.as_str()) else {
                return Err(PatcherError::MissingDependency {
                    patch: patch.name().to_owned(),
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
            .map(|i| patches[i].name().to_owned())
            .collect();
        return Err(PatcherError::DependencyCycle(names));
    }
    Ok(order)
}
