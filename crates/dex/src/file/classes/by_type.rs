// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use super::ClassTable;
use crate::model::TypeIdx;

#[derive(Debug, Clone)]
pub(super) struct TypeIndex {
    pub(super) ordered: Vec<(TypeIdx, u32)>,
    pub(super) changed: BTreeSet<u32>,
}

impl ClassTable {
    pub fn index_of_type(&self, type_idx: TypeIdx) -> Option<usize> {
        let index = self.by_type.get_or_init(|| {
            let mut ordered: Vec<_> = self
                .headers()
                .enumerate()
                .map(|(i, header)| (header.class_type, i as u32))
                .collect();
            ordered.sort_unstable_by_key(|&(type_, _)| type_);
            TypeIndex {
                ordered,
                changed: BTreeSet::new(),
            }
        });
        // A granted mutable class can change its type after the snapshot was built.
        // Search those slots directly; all other snapshot keys remain valid.
        if let Some(&slot) = index
            .changed
            .iter()
            .find(|&&slot| self.header(slot as usize).class_type == type_idx)
        {
            return Some(slot as usize);
        }
        let first = index
            .ordered
            .partition_point(|&(type_, _)| type_ < type_idx);
        index.ordered[first..]
            .iter()
            .take_while(|&&(type_, _)| type_ == type_idx)
            .find(|&&(_, slot)| !index.changed.contains(&slot))
            .map(|&(_, slot)| slot as usize)
    }
}
