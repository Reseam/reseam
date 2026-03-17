use std::collections::{HashMap, HashSet};

use crate::error::{PatcherError, Result};
use crate::patch::Patch;

pub fn sort_patches(patches: &[Box<dyn Patch>]) -> Result<Vec<usize>> {
    let name_to_idx: HashMap<&str, usize> = patches
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name(), i))
        .collect();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); patches.len()];
    let mut in_degree: Vec<usize> = vec![0; patches.len()];

    for (i, patch) in patches.iter().enumerate() {
        for dep_name in patch.depends_on() {
            if let Some(&dep_idx) = name_to_idx.get(dep_name) {
                adj[dep_idx].push(i);
                in_degree[i] += 1;
            }
        }
    }

    let mut queue: std::collections::VecDeque<usize> = in_degree
        .iter()
        .enumerate()
        .filter(|(_, &d)| d == 0)
        .map(|(i, _)| i)
        .collect();

    let mut order = Vec::with_capacity(patches.len());
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for &next in &adj[idx] {
            in_degree[next] -= 1;
            if in_degree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    if order.len() != patches.len() {
        let cycle_names: Vec<String> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, &d)| d > 0)
            .map(|(i, _)| patches[i].name().to_owned())
            .collect();
        return Err(PatcherError::DependencyCycle { names: cycle_names });
    }

    Ok(order)
}

pub fn find_dependents(patches: &[Box<dyn Patch>]) -> HashMap<usize, Vec<usize>> {
    let name_to_idx: HashMap<&str, usize> = patches
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name(), i))
        .collect();

    let mut dependents: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, patch) in patches.iter().enumerate() {
        for dep_name in patch.depends_on() {
            if let Some(&dep_idx) = name_to_idx.get(dep_name) {
                dependents.entry(dep_idx).or_default().push(i);
            }
        }
    }
    dependents
}

pub fn collect_after_dependents_order(
    patches: &[Box<dyn Patch>],
    execution_order: &[usize],
) -> Vec<usize> {
    let dependents = find_dependents(patches);
    let mut completed: HashSet<usize> = HashSet::new();
    let mut after_order = Vec::new();

    for &idx in execution_order {
        completed.insert(idx);

        for (&dep_idx, dep_list) in &dependents {
            if completed.contains(&dep_idx) {
                continue;
            }
            if dep_list.iter().all(|d| completed.contains(d)) {
                if !after_order.contains(&dep_idx) {
                    after_order.push(dep_idx);
                }
            }
        }
    }

    for (&dep_idx, dep_list) in &dependents {
        if dep_list.iter().all(|d| completed.contains(d)) && !after_order.contains(&dep_idx) {
            after_order.push(dep_idx);
        }
    }

    after_order
}
