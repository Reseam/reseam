// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use crate::error::{PatcherError, Result};
use crate::patch::PatchSpec;

/// Patch identities shared by planning and CLI options. A reference
/// (`<bundle>/<id>`) resolves exactly; a bare ID or a display name must
/// identify one patch across the loaded bundles for the target package.
pub struct PatchIndex<'a> {
    patches: &'a [&'a PatchSpec],
    references: HashMap<Reference<'a>, usize>,
    ids: HashMap<&'a str, Vec<usize>>,
    names: HashMap<&'a str, Vec<usize>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Reference<'a> {
    bundle: &'a str,
    id: &'a str,
}

impl<'a> Reference<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let (bundle, id) = value.split_once('/')?;
        Some(Self { bundle, id })
    }
}

impl<'a> PatchIndex<'a> {
    pub fn new(patches: &'a [&'a PatchSpec]) -> Result<Self> {
        let mut references = HashMap::with_capacity(patches.len());
        let mut ids: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut names: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, patch) in patches.iter().enumerate() {
            let spec = patch;
            if references
                .insert(
                    Reference {
                        bundle: &spec.bundle,
                        id: &spec.id,
                    },
                    index,
                )
                .is_some()
            {
                return Err(PatcherError::Bundle(format!(
                    "bundle '{}' declares patch '{}' more than once",
                    spec.bundle, spec.id
                )));
            }
            ids.entry(&spec.id).or_default().push(index);
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

    pub(super) fn reference(&self, selector: &str) -> Option<usize> {
        self.references.get(&Reference::parse(selector)?).copied()
    }

    pub fn resolve(&self, selector: &str, package: Option<&str>) -> Result<usize> {
        if let Some(index) = self.reference(selector) {
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
            .map(|&index| self.patches[index].reference())
            .collect();
        references.sort_unstable();
        Err(PatcherError::InvalidSelection(format!(
            "patch '{selector}' is ambiguous; select one of: {}",
            references.join(", ")
        )))
    }
}
