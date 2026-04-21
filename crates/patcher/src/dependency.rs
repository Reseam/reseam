// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, VecDeque};

use crate::error::{PatcherError, Result};
use crate::patch::Patch;

#[derive(Debug, Clone)]
pub struct PatchGraph<'a> {
    patch_index: HashMap<&'a str, usize>,
    dependencies: Vec<Vec<usize>>,
    dependents: Vec<Vec<usize>>,
    order: Vec<usize>,
}

impl<'a> PatchGraph<'a> {
    pub fn build(patches: &'a [Box<dyn Patch>]) -> Result<Self> {
        let mut patch_index = HashMap::with_capacity(patches.len());
        for (idx, patch) in patches.iter().enumerate() {
            if patch_index.insert(patch.name(), idx).is_some() {
                return Err(PatcherError::InvalidSelection(
                    "patch names must be unique within a bundle".to_owned(),
                ));
            }
        }

        let mut dependencies = vec![Vec::new(); patches.len()];
        let mut dependents = vec![Vec::new(); patches.len()];
        let mut in_degree = vec![0usize; patches.len()];

        for (idx, patch) in patches.iter().enumerate() {
            for dependency in patch.depends_on() {
                let Some(&dependency_idx) = patch_index.get(dependency.as_str()) else {
                    return Err(PatcherError::MissingDependency {
                        patch: patch.name().to_owned(),
                        dependency: dependency.to_string(),
                    });
                };

                dependencies[idx].push(dependency_idx);
                dependents[dependency_idx].push(idx);
                in_degree[idx] += 1;
            }
        }

        let mut queue: VecDeque<usize> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, degree)| **degree == 0)
            .map(|(idx, _)| idx)
            .collect();

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
            let cycle_names = in_degree
                .iter()
                .enumerate()
                .filter(|(_, degree)| **degree > 0)
                .map(|(idx, _)| patches[idx].name().to_owned())
                .collect();
            return Err(PatcherError::DependencyCycle { names: cycle_names });
        }

        Ok(Self {
            patch_index,
            dependencies,
            dependents,
            order,
        })
    }

    pub fn index_of(&self, patch_id: &str) -> Option<usize> {
        self.patch_index.get(patch_id).copied()
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
}

pub fn sort_patches(patches: &[Box<dyn Patch>]) -> Result<Vec<usize>> {
    Ok(PatchGraph::build(patches)?.order().to_vec())
}

pub fn find_dependents(patches: &[Box<dyn Patch>]) -> Result<Vec<Vec<usize>>> {
    let graph = PatchGraph::build(patches)?;
    Ok((0..patches.len())
        .map(|idx| graph.dependents(idx).to_vec())
        .collect())
}
