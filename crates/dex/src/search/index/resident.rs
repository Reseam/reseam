// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use smallvec::SmallVec;

use super::{RefKey, RefQuery, Slot};
use crate::instruction::Instruction;
use crate::model::class::ClassData;

/// The references of a resident class, which the file's index does not cover:
/// each method's distinct keys, sorted, methods in slot order.
#[derive(Debug, Clone)]
pub(crate) struct ClassRefs {
    start: Vec<u32>,
    keys: Vec<RefKey>,
}

impl ClassRefs {
    pub(crate) fn of(data: &ClassData) -> Self {
        let mut start = vec![0];
        let mut keys = Vec::new();
        let mut method = Vec::new();
        for encoded in data.direct_methods.iter().chain(&data.virtual_methods) {
            method.clear();
            if let Some(code) = &encoded.code {
                method.extend(code.instructions.iter().filter_map(reference));
            }
            method.sort_unstable();
            method.dedup();
            keys.extend_from_slice(&method);
            start.push(keys.len() as u32);
        }
        Self { start, keys }
    }

    /// The positions of the methods holding every `all` key of `query` and,
    /// when `any` is not empty, one of its keys.
    pub(crate) fn matching(&self, query: &RefQuery) -> SmallVec<[Slot; 4]> {
        self.start
            .windows(2)
            .enumerate()
            .filter(|(_, range)| {
                let keys = &self.keys[range[0] as usize..range[1] as usize];
                let holds = |key: &RefKey| keys.binary_search(key).is_ok();
                query.all.iter().all(holds) && (query.any.is_empty() || query.any.iter().any(holds))
            })
            .map(|(position, _)| {
                Slot::try_from(position).expect("a class defines at most 65536 methods")
            })
            .collect()
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.start.len() * size_of::<u32>() + self.keys.len() * size_of::<RefKey>()
    }
}

/// The key an instruction contributes, by the rule the file index uses: its
/// method, field or string reference, else its literal.
fn reference(instruction: &Instruction) -> Option<RefKey> {
    instruction
        .method_ref()
        .map(RefKey::Method)
        .or_else(|| instruction.field_ref().map(RefKey::Field))
        .or_else(|| instruction.string_ref().map(RefKey::String))
        .or_else(|| instruction.literal().map(RefKey::Literal))
}
