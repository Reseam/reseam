// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Finding functions and branches in the original library.

use std::collections::BTreeMap;

use rustc_hash::FxHashSet;

use super::Editor;
use crate::code::Code;
use crate::data::data_addresses;
use crate::error::{ElfError, Result};
use crate::{Branch, Function, Pattern, Query};

impl Editor {
    /// The function of the original library matching `query`.
    pub fn find_function(&self, query: &Query<'_>) -> Result<Function> {
        let mut candidates = self.candidates(query)?;
        let count = candidates.len();
        match query.nth {
            None if count == 1 => Ok(candidates.remove(0)),
            Some(index) if index < count => Ok(candidates.remove(index)),
            _ => Err(ElfError::Match {
                query: query.to_string(),
                count,
            }),
        }
    }

    fn candidates(&self, query: &Query<'_>) -> Result<Vec<Function>> {
        let mut candidates = query
            .symbol
            .map(|symbol| self.image.symbol_functions(symbol))
            .transpose()?
            .map(|functions| by_address(functions.into_iter()));
        let constraints = (query.strings.iter())
            .map(|text| self.referencing(text.as_bytes()))
            .chain(query.patterns.iter().map(|pattern| self.holding(pattern)));
        for matching in constraints {
            let matching = matching?;
            candidates = Some(match candidates {
                Some(mut candidates) => {
                    candidates.retain(|address, _| matching.contains_key(address));
                    candidates
                }
                None => matching,
            });
        }
        Ok(candidates.unwrap_or_default().into_values().collect())
    }

    /// The functions whose code computes the address of `text`, by start address.
    fn referencing(&self, text: &[u8]) -> Result<BTreeMap<u64, Function>> {
        let targets: FxHashSet<u64> = data_addresses(&self.image, text)?.into_iter().collect();
        if targets.is_empty() {
            return Ok(BTreeMap::new());
        }
        Ok(by_address(
            self.code()?
                .references(&self.image)?
                .referencing(&targets)
                .into_iter(),
        ))
    }

    /// The functions whose code holds a match of `pattern`, by start address.
    fn holding(&self, pattern: &Pattern) -> Result<BTreeMap<u64, Function>> {
        let code = self.code()?;
        let mut functions = BTreeMap::new();
        for section in self
            .image
            .sections()
            .iter()
            .filter(|section| section.is_code())
        {
            for offset in pattern.matches(self.image.contents(section)?) {
                let start = section.address + offset as u64;
                let last = start + pattern.len() as u64 - 1;
                if let Some(function) = code.function(&self.image, start)?
                    && function.contains(last)
                {
                    functions.insert(function.address, function);
                }
            }
        }
        Ok(functions)
    }

    /// The one conditional branch whose bytes a match of `pattern` in `function`'s code covers.
    pub fn find_branch(&self, function: Function, pattern: &Pattern) -> Result<Branch> {
        let code = self.code()?;
        let mut branches = Vec::new();
        for offset in pattern.matches(self.function_code(function)?) {
            let start = function.address + offset as u64;
            branches.extend(
                code.conditional_branches(&self.image, start..start + pattern.len() as u64)?,
            );
        }
        match branches[..] {
            [branch] => Ok(branch),
            _ => Err(ElfError::Branch {
                pattern: pattern.to_string(),
                function,
                count: branches.len(),
            }),
        }
    }

    fn function_code(&self, function: Function) -> Result<&[u8]> {
        let size =
            usize::try_from(function.size).map_err(|_| ElfError::Unmapped(function.address))?;
        self.image.loaded(function.address, size)
    }

    /// Where the functions lie, read on first use. AArch64 functions are bounded by the unwind
    /// table, 32-bit ARM ones by walking the code.
    fn code(&self) -> Result<&Code> {
        if let Some(code) = self.code.get() {
            return Ok(code);
        }
        let code = Code::read(&self.image)?;
        Ok(self.code.get_or_init(|| code))
    }
}

fn by_address(functions: impl Iterator<Item = Function>) -> BTreeMap<u64, Function> {
    functions
        .map(|function| (function.address, function))
        .collect()
}
