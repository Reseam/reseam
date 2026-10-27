// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use super::PatchSelection;
use crate::catalog::PatchCatalog;
use crate::error::{PatcherError, Result};
use crate::options::PatchOptions;
use crate::patch::PatchSpec;

pub(super) fn resolve_options(
    specs: &[&PatchSpec],
    selection: &PatchSelection,
    index: &PatchCatalog<'_>,
    package: Option<&str>,
    desired: &[bool],
    disabled: &[bool],
) -> Result<Vec<PatchOptions>> {
    let mut configured = HashMap::new();
    for (patch, options) in &selection.options {
        let idx = index.resolve(patch, package)?;
        if configured
            .insert(idx, PatchOptions::from(options.clone()))
            .is_some()
        {
            return Err(PatcherError::InvalidSelection(format!(
                "options for patch '{}' were supplied under multiple selectors",
                specs[idx].reference()
            )));
        }
        if !desired[idx] || disabled[idx] {
            return Err(PatcherError::InvalidSelection(format!(
                "patch '{patch}' has options configured but is not enabled by the selection"
            )));
        }
    }
    specs
        .iter()
        .enumerate()
        .map(|(idx, spec)| {
            if !desired[idx] || disabled[idx] {
                return Ok(PatchOptions::default());
            }
            PatchOptions::resolve(&spec.reference(), &spec.options, configured.get(&idx))
        })
        .collect()
}
