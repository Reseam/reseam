// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Slot;

/// Keys a word of [`Postings::present`] covers.
const WORD: usize = u64::BITS as usize;

/// The slots referencing each key, keys numbered from zero. Many keys have no
/// slots, such as strings no code loads, so only the keys that have any keep
/// a place in `start`.
#[derive(Debug, Clone)]
pub(super) struct Postings {
    /// One bit per key, set for the keys that have slots.
    present: Vec<u64>,
    /// The keys with slots before each word of `present`.
    rank: Vec<u32>,
    /// Where the slots of each key that has any begin, and the end of the last.
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
        // Each key's cursor now holds its end, and its start is the end before it.
        let mut present = vec![0u64; keys.div_ceil(WORD)];
        let mut rank = Vec::with_capacity(present.len());
        let mut starts = vec![0];
        for (word, ends) in start[..keys].chunks(WORD).enumerate() {
            rank.push(starts.len() as u32 - 1);
            for (bit, &end) in ends.iter().enumerate() {
                if end != starts[starts.len() - 1] {
                    present[word] |= 1 << bit;
                    starts.push(end);
                }
            }
        }
        starts.shrink_to_fit();
        Self {
            present,
            rank,
            start: starts,
            slots,
        }
    }

    pub(super) fn get(&self, key: usize) -> &[Slot] {
        let (word, bit) = (key / WORD, key % WORD);
        match self.present.get(word) {
            Some(&present) if present & (1 << bit) != 0 => {
                let before = (present & ((1 << bit) - 1)).count_ones();
                let place = (self.rank[word] + before) as usize;
                &self.slots[self.start[place] as usize..self.start[place + 1] as usize]
            }
            _ => &[],
        }
    }

    pub(super) fn heap_bytes(&self) -> usize {
        self.present.len() * size_of::<u64>()
            + (self.rank.len() + self.start.len()) * size_of::<u32>()
            + self.slots.len() * size_of::<Slot>()
    }
}

#[cfg(test)]
mod tests {
    use super::{Postings, Slot, WORD};

    #[test]
    fn each_key_gets_its_slots_in_order_and_other_keys_get_none() {
        let last = 3 * WORD - 1;
        let pairs: [(usize, Slot); 7] = [
            (last, 9),
            (0, 4),
            (WORD, 1),
            (0, 7),
            (WORD - 1, 2),
            (last, 3),
            (WORD + 5, 8),
        ];
        let postings = Postings::counted(last + 1, pairs.iter().copied());
        for key in 0..=last + WORD {
            let expected: Vec<Slot> = pairs
                .iter()
                .filter(|&&(of, _)| of == key)
                .map(|&(_, slot)| slot)
                .collect();
            assert_eq!(postings.get(key), expected, "key {key}");
        }
    }
}
