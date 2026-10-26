// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Slot;

/// The slots referencing each key, keys numbered from zero.
#[derive(Debug, Clone)]
pub(super) struct Postings {
    start: Vec<u32>,
    slots: Vec<Slot>,
}

impl Postings {
    /// From `(key, slot)` pairs with every key below `keys`. `pairs` yields the
    /// same sequence each time it is cloned; each key's slots keep its order.
    pub(super) fn counted(keys: usize, pairs: impl Iterator<Item = (usize, Slot)> + Clone) -> Self {
        let mut start = vec![0u32; keys + 1];
        for (key, _) in pairs.clone() {
            start[key + 1] += 1;
        }
        for key in 0..keys {
            start[key + 1] += start[key];
        }
        let mut slots = vec![0; start[keys] as usize];
        // `start[key]` serves as the key's write cursor and ends at its successor's start.
        for (key, slot) in pairs {
            slots[start[key] as usize] = slot;
            start[key] += 1;
        }
        start.copy_within(0..keys, 1);
        start[0] = 0;
        Self { start, slots }
    }

    pub(super) fn get(&self, key: usize) -> &[Slot] {
        match self.start.get(key..key + 2) {
            Some(&[from, to]) => &self.slots[from as usize..to as usize],
            _ => &[],
        }
    }

    pub(super) fn heap_bytes(&self) -> usize {
        self.start.len() * size_of::<u32>() + self.slots.len() * size_of::<Slot>()
    }
}
