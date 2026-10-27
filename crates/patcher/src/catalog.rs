// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use crate::error::{BundleError, PatcherError, Result};
use crate::patch::{PatchRef, PatchSpec};

/// Patch identities shared by planning and CLI options. A reference
/// (`<bundle>/<id>`) resolves exactly; a bare ID or a display name must
/// identify one patch across the loaded bundles for the target package.
pub struct PatchCatalog<'a> {
    patches: &'a [&'a PatchSpec],
    references: HashMap<&'a PatchRef, usize>,
    ids: HashMap<&'a str, Vec<usize>>,
    names: HashMap<&'a str, Vec<usize>>,
}

impl<'a> PatchCatalog<'a> {
    pub fn new(patches: &'a [&'a PatchSpec]) -> Result<Self> {
        let mut references = HashMap::with_capacity(patches.len());
        let mut ids: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut names: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, spec) in patches.iter().enumerate() {
            if references.insert(&spec.reference, index).is_some() {
                return Err(BundleError::DuplicatePatch(spec.reference.clone()).into());
            }
            ids.entry(&spec.reference.id).or_default().push(index);
            if !spec.hidden {
                names.entry(&spec.name).or_default().push(index);
            }
        }
        Ok(Self {
            patches,
            references,
            ids,
            names,
        })
    }

    /// The position of the patch `reference` names.
    pub(crate) fn find(&self, reference: &PatchRef) -> Option<usize> {
        self.references.get(reference).copied()
    }

    pub fn resolve(&self, selector: &str, package: Option<&str>) -> Result<usize> {
        if let Some(index) = PatchRef::parse(selector).and_then(|reference| self.find(&reference)) {
            return Ok(index);
        }
        let named = self
            .ids
            .get(selector)
            .or_else(|| self.names.get(selector))
            .ok_or_else(|| PatcherError::UnknownPatch(selector.to_owned()))?;
        let compatible: Vec<_> = named
            .iter()
            .copied()
            .filter(|&index| {
                self.patches[index]
                    .compatibility
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
        let mut references: Vec<_> = candidates
            .iter()
            .map(|&index| self.patches[index].reference.to_string())
            .collect();
        references.sort_unstable();
        Err(PatcherError::InvalidSelection(format!(
            "patch '{selector}' is ambiguous; select one of: {}",
            references.join(", ")
        )))
    }
}
