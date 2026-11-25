// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use rustc_hash::FxHashSet;

use crate::Function;
use crate::error::Result;

/// Every address in loaded data the library's code computes, with the function computing it,
/// sorted so each lookup by string is a search.
pub(crate) struct References {
    /// The functions that compute an address, once each.
    functions: Vec<Function>,
    /// Each computed address and the index of the function computing it, by address.
    references: Vec<(u64, u32)>,
}

impl References {
    /// Indexes each computed address and the function computing it, as they are found. A
    /// function's references arrive together, so it is kept once.
    pub(crate) fn new(found: impl IntoIterator<Item = Result<(u64, Function)>>) -> Result<Self> {
        let mut functions: Vec<Function> = Vec::new();
        let mut references = found
            .into_iter()
            .map(|found| {
                let (target, function) = found?;
                if functions.last() != Some(&function) {
                    functions.push(function);
                }
                Ok((target, (functions.len() - 1) as u32))
            })
            .collect::<Result<Vec<_>>>()?;
        references.sort_unstable();
        Ok(Self {
            functions,
            references,
        })
    }

    /// The functions whose code computes one of `targets`, once for each reference.
    pub(crate) fn referencing(&self, targets: &FxHashSet<u64>) -> Vec<Function> {
        targets
            .iter()
            .flat_map(|&target| {
                let first = self
                    .references
                    .partition_point(|&(address, _)| address < target);
                self.references[first..]
                    .iter()
                    .take_while(move |&&(address, _)| address == target)
            })
            .map(|&(_, function)| self.functions[function as usize])
            .collect()
    }
}
