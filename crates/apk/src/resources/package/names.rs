// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::OnceLock;

use reseam_storage::Bytes;
use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::resources::ResType;

/// The entry index of each name, built by the first lookup. A clone starts without it, so the
/// copies of the table the undo journal keeps don't each hold one.
#[derive(Debug, Default)]
pub(super) struct NameIndex(OnceLock<EntryNames>);

impl Clone for NameIndex {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl NameIndex {
    pub(super) fn find(
        &self,
        types: &[ResType],
        data: &Bytes,
        type_id: u8,
        key: u32,
    ) -> Result<Option<usize>> {
        self.0
            .get_or_init(|| {
                let names = EntryNames::build(types);
                data.release_pages();
                names
            })
            .find(types, type_id, key)
    }

    pub(super) fn record(&mut self, type_id: u8, key: u32, index: usize) {
        if let Some(names) = self.0.get_mut()
            && names.index_of(type_id, key).is_none()
        {
            let index = u32::try_from(index).expect("entry indexes are 16-bit");
            names.added.insert((type_id, key), index);
        }
    }

    pub(super) fn clear(&mut self) {
        self.0.take();
    }
}

/// Each key's first entry index in its type, as a scan of the configurations in chunk order and
/// their entries in index order finds it.
#[derive(Debug)]
struct EntryNames {
    by_type: Vec<Box<[(u32, u32)]>>,
    added: FxHashMap<(u8, u32), u32>,
    unreadable: FxHashMap<u8, (usize, usize)>,
}

impl EntryNames {
    fn build(types: &[ResType]) -> Self {
        let mut unreadable = FxHashMap::default();
        let type_count = types
            .iter()
            .map(|t| usize::from(t.id) + 1)
            .max()
            .unwrap_or(0);
        let by_type = (0..type_count)
            .map(|id| {
                let chunks = || {
                    types
                        .iter()
                        .enumerate()
                        .filter(move |(_, t)| usize::from(t.id) == id)
                };
                let present = chunks()
                    .map(|(_, t)| {
                        (0..t.len())
                            .filter(|&i| matches!(t.entry_key(i), Ok(Some(_))))
                            .count()
                    })
                    .sum();
                let mut keyed = Vec::with_capacity(present);
                for (ordinal, (position, res_type)) in chunks().enumerate() {
                    for index in 0..res_type.len() {
                        match res_type.entry_key(index) {
                            Ok(Some(key)) => keyed.push((key, ordinal as u32, index as u32)),
                            Ok(None) => {}
                            Err(_) => {
                                unreadable.entry(res_type.id).or_insert((position, index));
                            }
                        }
                    }
                }
                keyed.sort_unstable();
                keyed.dedup_by_key(|&mut (key, _, _)| key);
                keyed
                    .into_iter()
                    .map(|(key, _, index)| (key, index))
                    .collect()
            })
            .collect();
        Self {
            by_type,
            added: FxHashMap::default(),
            unreadable,
        }
    }

    fn index_of(&self, type_id: u8, key: u32) -> Option<u32> {
        self.by_type
            .get(usize::from(type_id))
            .and_then(|entries| {
                let at = entries.binary_search_by_key(&key, |&(k, _)| k).ok()?;
                Some(entries[at].1)
            })
            .or_else(|| self.added.get(&(type_id, key)).copied())
    }

    /// A key it lacks fails with the first unreadable entry of its type, as a scan that reached
    /// the end of the type would.
    fn find(&self, types: &[ResType], type_id: u8, key: u32) -> Result<Option<usize>> {
        if let Some(index) = self.index_of(type_id, key) {
            return Ok(Some(index as usize));
        }
        match self.unreadable.get(&type_id) {
            Some(&(position, index)) => types[position].entry_key(index).map(|_| None),
            None => Ok(None),
        }
    }
}
