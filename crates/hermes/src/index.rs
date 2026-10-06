// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use rustc_hash::FxHashMap;

use crate::error::{HermesError, Result, invalid};
use crate::model::{Encoding, FOOTER_SIZE, StringId, StringValue};
use crate::opcode::IdKind;
use crate::parse::switch_table;
use crate::{FunctionId, HermesFile};

/// An immutable search index of the original app functions. Build it once and
/// retain it alongside the source. Names and referenced strings use inverted
/// postings; only one function's instructions are decoded at a time. Edits and
/// linked modules do not change app identities or the indexed original bodies.
pub struct FunctionIndex {
    base_hash: [u8; FOOTER_SIZE],
    texts: FxHashMap<u64, Vec<StringId>>,
    names: FxHashMap<StringId, Vec<FunctionId>>,
    strings: FxHashMap<StringId, Vec<FunctionId>>,
    parameters: Vec<u32>,
}

impl FunctionIndex {
    /// Indexes names, declared parameter counts and instruction string
    /// references, including string-switch keys. Invalid instructions or string
    /// IDs fail construction; file bytes and decoded instructions are not kept.
    pub fn build(file: &HermesFile<'_>) -> Result<Self> {
        let mut index = Self {
            base_hash: file.footer(),
            texts: FxHashMap::default(),
            names: FxHashMap::default(),
            strings: FxHashMap::default(),
            parameters: Vec::with_capacity(file.function_count() as usize),
        };
        for id in (0..file.function_count()).map(FunctionId) {
            let function = file.function(id)?;
            index.names.entry(function.name()).or_default().push(id);
            index.parameters.push(function.parameter_count());
            let mut referenced = Vec::new();
            for instruction in function.instructions()? {
                referenced.extend(
                    instruction
                        .operands()
                        .filter(|(operand, _)| operand.id == IdKind::String)
                        .map(|(_, value)| StringId(value as u32)),
                );
                if let Some(table) = switch_table(&function, &instruction)? {
                    referenced.extend(table.entries().filter_map(|(key, _)| key));
                }
            }
            referenced.sort_unstable();
            referenced.dedup();
            for string in referenced {
                index.strings.entry(string).or_default().push(id);
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

    fn candidates(
        &self,
        file: &HermesFile<'_>,
        text: &str,
        postings: &FxHashMap<StringId, Vec<FunctionId>>,
    ) -> Result<Vec<FunctionId>> {
        let units: Vec<_> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let fingerprint = StringValue {
            encoding: Encoding::Utf16,
            bytes: &units,
        }
        .fingerprint();
        let mut candidates = Vec::new();
        for &id in self.texts.get(&fingerprint).into_iter().flatten() {
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

    /// Returns exactly one original app function matching every constraint.
    /// Matching is exact and case-sensitive; parameter counts exclude `this`.
    /// Zero or multiple matches report the query and count. A different source
    /// footer is an error. Queries decode no bytecode and allocate no file data.
    pub fn find_function(
        &self,
        file: &HermesFile<'_>,
        name: Option<&str>,
        strings: &[&str],
        parameters: Option<u32>,
    ) -> Result<FunctionId> {
        if file.footer() != self.base_hash {
            return Err(invalid(0, "function index belongs to another source file"));
        }
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
                query: format!("name={name:?}, strings={strings:?}, parameters={parameters:?}"),
                count: matches.len(),
            })
        }
    }
}
