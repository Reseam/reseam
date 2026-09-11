// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use crate::error::{PatcherError, Result};
use crate::patch::Patch;

/// Patch identities and display-name aliases shared by planning and CLI options.
/// IDs resolve exactly; names must identify one patch for the target package.
pub struct PatchIndex<'a> {
    patches: &'a [&'a dyn Patch],
    pub(super) ids: HashMap<&'a str, usize>,
    names: HashMap<&'a str, Vec<usize>>,
}

impl<'a> PatchIndex<'a> {
    pub fn new(patches: &'a [&'a dyn Patch]) -> Result<Self> {
        let mut ids = HashMap::with_capacity(patches.len());
        let mut names: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, patch) in patches.iter().enumerate() {
            if ids.insert(patch.id(), index).is_some() {
                return Err(PatcherError::InvalidSelection(format!(
                    "patch ID '{}' is used more than once in the bundle",
                    patch.id()
                )));
            }
            if !patch.spec().hidden {
                names.entry(&patch.spec().name).or_default().push(index);
            }
        }
        Ok(Self {
            patches,
            ids,
            names,
        })
    }

    pub fn resolve(&self, selector: &str, package: Option<&str>) -> Result<usize> {
        if let Some(&index) = self.ids.get(selector) {
            return Ok(index);
        }
        let named = self
            .names
            .get(selector)
            .ok_or_else(|| PatcherError::UnknownPatch(selector.to_owned()))?;
        let compatible: Vec<_> = named
            .iter()
            .copied()
            .filter(|&index| {
                self.patches[index]
                    .spec()
                    .package_incompatibility(package)
                    .is_none()
            })
            .collect();
        // A globally unique name may still be selected for an incompatible app;
        // normal validation then reports why that patch is skipped.
        let candidates = if compatible.is_empty() {
            named.as_slice()
        } else {
            &compatible
        };
        if let [index] = candidates {
            return Ok(*index);
        }
        let mut ids: Vec<_> = candidates
            .iter()
            .map(|&index| self.patches[index].id())
            .collect();
        ids.sort_unstable();
        Err(PatcherError::InvalidSelection(format!(
            "patch name '{selector}' is ambiguous; select a patch ID: {}",
            ids.join(", ")
        )))
    }
}
