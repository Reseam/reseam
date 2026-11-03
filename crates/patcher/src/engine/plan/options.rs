// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use super::{PatchSelection, ResolvedPlan};
use crate::catalog::PatchCatalog;
use crate::error::{PatcherError, Result};
use crate::options::PatchOptions;
use crate::patch::PatchSpec;
use reseam_model::OptionProblem;

/// The option values each patch of a plan runs with, by position in the specs
/// the plan was resolved from.
#[derive(Debug)]
pub struct PlanOptions {
    options: Vec<PatchOptions>,
}

impl PlanOptions {
    /// Checks the options `selection` sets against the patches `plan` covers,
    /// and fills in defaults. Options for a patch the plan does not run, or a
    /// required option without a value, fail.
    pub fn resolve(
        specs: &[&PatchSpec],
        selection: &PatchSelection,
        plan: &ResolvedPlan,
        package: Option<&str>,
    ) -> Result<Self> {
        let configured = configured(specs, selection, plan, package)?;
        let options = specs
            .iter()
            .enumerate()
            .map(|(idx, spec)| {
                if !runs_with_options(plan, idx) {
                    return Ok(PatchOptions::default());
                }
                PatchOptions::resolve(&spec.reference(), &spec.options, configured.get(&idx))
            })
            .collect::<Result<_>>()?;
        Ok(Self { options })
    }

    /// The [problems](PatchOptions::problems) with the option values each
    /// patch `plan` covers would run with, by position. Selections that set
    /// options for a patch the plan does not run fail as they do in
    /// [`resolve`](Self::resolve).
    pub fn problems(
        specs: &[&PatchSpec],
        selection: &PatchSelection,
        plan: &ResolvedPlan,
        package: Option<&str>,
    ) -> Result<Vec<Vec<(String, OptionProblem)>>> {
        let configured = configured(specs, selection, plan, package)?;
        Ok(specs
            .iter()
            .enumerate()
            .map(|(idx, spec)| {
                if runs_with_options(plan, idx) {
                    PatchOptions::problems(&spec.options, configured.get(&idx))
                } else {
                    Vec::new()
                }
            })
            .collect())
    }

    pub(crate) fn get(&self, idx: usize) -> &PatchOptions {
        &self.options[idx]
    }
}

fn runs_with_options(plan: &ResolvedPlan, idx: usize) -> bool {
    plan.covers(idx) && !plan.is_disabled(idx)
}

/// The options `selection` sets, by position of the patch they are for.
fn configured(
    specs: &[&PatchSpec],
    selection: &PatchSelection,
    plan: &ResolvedPlan,
    package: Option<&str>,
) -> Result<HashMap<usize, PatchOptions>> {
    let index = PatchCatalog::new(specs)?;
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
        if !runs_with_options(plan, idx) {
            return Err(PatcherError::InvalidSelection(format!(
                "patch '{patch}' has options configured but is not enabled by the selection"
            )));
        }
    }
    Ok(configured)
}
