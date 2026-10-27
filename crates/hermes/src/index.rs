// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::error::{HermesError, Result};
use crate::opcode::IdKind;
use crate::string::{StringId, fingerprint};
use crate::switch::switch_table;
use crate::{FunctionId, HermesFile};

/// An immutable search index of the original app functions. Build it once and
/// retain it alongside the source. Names, referenced strings and referencing
/// functions use inverted postings; each function is decoded once, in parallel,
/// and its instructions are not kept. Edits and linked modules do not change app
/// identities or the indexed original bodies.
pub(crate) struct FunctionIndex {
    texts: FxHashMap<u64, Vec<StringId>>,
    names: FxHashMap<StringId, Vec<FunctionId>>,
    strings: FxHashMap<StringId, Vec<FunctionId>>,
    creators: FxHashMap<FunctionId, Vec<FunctionId>>,
    parameters: Vec<u32>,
}

/// What one function's bytecode references, read in a single decode.
struct Scan {
    name: StringId,
    parameters: u32,
    strings: Vec<StringId>,
    functions: Vec<FunctionId>,
}

impl Scan {
    fn read(file: &HermesFile<'_>, id: FunctionId) -> Result<Self> {
        let function = file.function(id)?;
        let mut strings = Vec::new();
        let mut functions = Vec::new();
        for instruction in function.instructions()? {
            for (operand, value) in instruction.operands() {
                match operand.id {
                    IdKind::String => strings.push(StringId(value as u32)),
                    IdKind::Function => functions.push(FunctionId(value as u32)),
                    _ => {}
                }
            }
            if let Some(table) = switch_table(&function, &instruction)? {
                strings.extend(table.entries().filter_map(|(key, _)| key));
            }
        }
        strings.sort_unstable();
        strings.dedup();
        functions.sort_unstable();
        functions.dedup();
        Ok(Self {
            name: function.name(),
            parameters: function.parameter_count(),
            strings,
            functions,
        })
    }
}

impl FunctionIndex {
    /// Indexes names, declared parameter counts, instruction string references
    /// (including string-switch keys) and function references. Invalid
    /// instructions or IDs fail construction; file bytes are not kept.
    pub(crate) fn build(file: &HermesFile<'_>) -> Result<Self> {
        let scans = (0..file.function_count())
            .into_par_iter()
            .map(|id| Scan::read(file, FunctionId(id)))
            .collect::<Result<Vec<_>>>()?;
        let mut index = Self {
            texts: FxHashMap::default(),
            names: FxHashMap::default(),
            strings: FxHashMap::default(),
            creators: FxHashMap::default(),
            parameters: Vec::with_capacity(scans.len()),
        };
        for (id, scan) in scans
            .into_iter()
            .enumerate()
            .map(|(id, scan)| (FunctionId(id as u32), scan))
        {
            index.names.entry(scan.name).or_default().push(id);
            index.parameters.push(scan.parameters);
            for string in scan.strings {
                index.strings.entry(string).or_default().push(id);
            }
            for function in scan.functions {
                index.creators.entry(function).or_default().push(id);
            }
        }
        let mut used: Vec<_> = index
            .names
            .keys()
            .chain(index.strings.keys())
            .copied()
            .collect();
        used.sort_unstable();
        used.dedup();
        for id in used {
            index
                .texts
                .entry(file.string(id)?.fingerprint())
                .or_default()
                .push(id);
        }
        Ok(index)
    }

    /// The original app functions whose instructions reference `function`, such
    /// as by creating its closure, in ascending order.
    pub(crate) fn creators(&self, function: FunctionId) -> &[FunctionId] {
        self.creators.get(&function).map_or(&[], Vec::as_slice)
    }

    fn candidates(
        &self,
        file: &HermesFile<'_>,
        text: &str,
        postings: &FxHashMap<StringId, Vec<FunctionId>>,
    ) -> Result<Vec<FunctionId>> {
        let mut candidates = Vec::new();
        for &id in self
            .texts
            .get(&fingerprint(text.encode_utf16()))
            .into_iter()
            .flatten()
        {
            if file.string(id)?.equals(text)
                && let Some(functions) = postings.get(&id)
            {
                candidates.extend_from_slice(functions);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        Ok(candidates)
    }

    /// The one original app function matching every constraint. Queries decode no bytecode.
    pub(crate) fn find_function(
        &self,
        file: &HermesFile<'_>,
        name: Option<&str>,
        strings: &[&str],
        parameters: Option<u32>,
    ) -> Result<FunctionId> {
        let mut constraints = Vec::with_capacity(strings.len() + usize::from(name.is_some()));
        if let Some(name) = name {
            constraints.push(self.candidates(file, name, &self.names)?);
        }
        for text in strings {
            constraints.push(self.candidates(file, text, &self.strings)?);
        }
        constraints.sort_unstable_by_key(Vec::len);
        let candidates = constraints
            .first()
            .cloned()
            .unwrap_or_else(|| (0..file.function_count()).map(FunctionId).collect());
        let matches: Vec<_> = candidates
            .into_iter()
            .filter(|id| {
                parameters.is_none_or(|p| self.parameters[id.0 as usize] == p)
                    && constraints
                        .iter()
                        .skip(1)
                        .all(|ids| ids.binary_search(id).is_ok())
            })
            .collect();
        if let [id] = matches[..] {
            Ok(id)
        } else {
            Err(HermesError::Match {
                name: name.map(str::to_owned),
                strings: strings.iter().map(|&text| text.to_owned()).collect(),
                parameters,
                count: matches.len(),
            })
        }
    }
}
