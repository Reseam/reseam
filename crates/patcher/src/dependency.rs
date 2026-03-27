use std::collections::HashMap;

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
            if let Some(&dep_idx) = name_to_idx.get(dep_name.as_str()) {
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
            if let Some(&dep_idx) = name_to_idx.get(dep_name.as_str()) {
                dependents.entry(dep_idx).or_default().push(i);
            }
        }
    }
    dependents
}
