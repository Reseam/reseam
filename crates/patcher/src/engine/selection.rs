// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{PatchSelection, ResolvedPatchPlan};
use crate::dependency::PatchGraph;
use crate::error::{PatcherError, Result};
use crate::options::{validate_patch_options, PatchOptions};
use crate::patch::Patch;

pub fn resolve_patch_selection(
    patches: &[Box<dyn Patch>],
    selection: &PatchSelection,
) -> Result<ResolvedPatchPlan> {
    let graph = PatchGraph::build(patches)?;
    validate_patch_selection(&graph, selection)?;

    let desired = resolve_desired_patches(patches, &graph, selection);
    let disabled = resolve_disabled_patches(patches.len(), &graph, selection);
    let options = validate_selection_options(patches, &graph, &desired, &disabled, selection)?;

    let dependencies = (0..patches.len())
        .map(|idx| graph.dependencies(idx).to_vec())
        .collect();
    let dependents = (0..patches.len())
        .map(|idx| graph.dependents(idx).to_vec())
        .collect();

    Ok(ResolvedPatchPlan {
        order: graph.order().to_vec(),
        dependencies,
        dependents,
        desired,
        disabled,
        options,
    })
}

fn validate_patch_selection(graph: &PatchGraph<'_>, selection: &PatchSelection) -> Result<()> {
    for patch in &selection.selected {
        if graph.index_of(patch.as_str()).is_none() {
            return Err(PatcherError::UnknownPatch(patch.to_string()));
        }
    }

    for patch in &selection.disabled {
        if graph.index_of(patch.as_str()).is_none() {
            return Err(PatcherError::UnknownPatch(patch.to_string()));
        }
        if selection.selected.contains(patch.as_str()) {
            return Err(PatcherError::InvalidSelection(format!(
                "patch '{patch}' cannot be both selected and disabled"
            )));
        }
    }

    for patch in selection.options.keys() {
        if graph.index_of(patch.as_str()).is_none() {
            return Err(PatcherError::UnknownPatch(patch.to_string()));
        }
    }

    Ok(())
}

fn resolve_desired_patches(
    patches: &[Box<dyn Patch>],
    graph: &PatchGraph<'_>,
    selection: &PatchSelection,
) -> Vec<bool> {
    let mut desired = vec![false; patches.len()];
    let mut stack: Vec<usize> = if selection.selected.is_empty() {
        patches
            .iter()
            .enumerate()
            .filter(|(_, patch)| patch.enabled_by_default())
            .map(|(idx, _)| idx)
            .collect()
    } else {
        selection
            .selected
            .iter()
            .map(|patch| {
                graph
                    .index_of(patch.as_str())
                    .expect("validated patch selection should only contain known patches")
            })
            .collect()
    };

    while let Some(idx) = stack.pop() {
        if desired[idx] {
            continue;
        }
        desired[idx] = true;
        stack.extend(graph.dependencies(idx).iter().copied());
    }

    desired
}

fn resolve_disabled_patches(
    patch_count: usize,
    graph: &PatchGraph<'_>,
    selection: &PatchSelection,
) -> Vec<bool> {
    let mut disabled = vec![false; patch_count];
    for patch in &selection.disabled {
        let idx = graph
            .index_of(patch.as_str())
            .expect("validated patch selection should only contain known patches");
        disabled[idx] = true;
    }
    disabled
}

fn validate_selection_options(
    patches: &[Box<dyn Patch>],
    graph: &PatchGraph<'_>,
    desired: &[bool],
    disabled: &[bool],
    selection: &PatchSelection,
) -> Result<Vec<Option<PatchOptions>>> {
    let mut validated = vec![None; patches.len()];

    for (idx, patch) in patches.iter().enumerate() {
        if !desired[idx] || disabled[idx] {
            if selection.options.contains_key(patch.name()) {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch '{}' has options configured but is not enabled by the selection",
                    patch.name()
                )));
            }
            continue;
        }

        let resolved = validate_patch_options(
            patch.name(),
            patch.options(),
            selection.options.get(patch.name()),
        )?;
        if resolved.iter().next().is_some() || !patch.options().is_empty() {
            validated[idx] = Some(resolved);
        }
    }

    for patch in selection.options.keys() {
        let idx = graph
            .index_of(patch.as_str())
            .expect("validated patch selection should only contain known patches");
        if !desired[idx] || disabled[idx] {
            return Err(PatcherError::InvalidSelection(format!(
                "patch '{}' has options configured but is not enabled by the selection",
                patch
            )));
        }
    }

    Ok(validated)
}
