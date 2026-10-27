// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

/// Lookup for a pool whose leading entries are in DEX sort order, as the
/// format requires of a parsed file: that prefix is binary-searched, and the
/// entries appended after it are found by hash. Entries are named by index;
/// the pool owning them supplies comparisons and hashes.
#[derive(Debug, Clone, Default)]
pub(crate) struct SortedIndex {
    sorted_len: usize,
    appended: FxHashMap<u64, SmallVec<[u32; 1]>>,
}

impl SortedIndex {
    /// Indexes `len` entries. `follows(i)` tells whether entry `i` sorts
    /// strictly after entry `i - 1`; `hash(i)` keys the entries past the
    /// sorted prefix.
    pub(crate) fn new(
        len: usize,
        follows: impl Fn(usize) -> bool,
        hash: impl Fn(usize) -> u64,
    ) -> Self {
        let sorted_len = (1..len).find(|&i| !follows(i)).unwrap_or(len);
        let mut index = Self {
            sorted_len,
            appended: FxHashMap::default(),
        };
        for i in sorted_len..len {
            index.push(hash(i), i);
        }
        index
    }

    /// How many leading entries are in DEX sort order.
    pub(crate) fn sorted_len(&self) -> usize {
        self.sorted_len
    }

    pub(crate) fn push(&mut self, hash: u64, index: usize) {
        self.appended.entry(hash).or_default().push(index as u32);
    }

    /// Forgets the appended entries in `dropped`, which end the pool.
    pub(crate) fn truncate(&mut self, dropped: Range<usize>, hash: impl Fn(usize) -> u64) {
        assert!(
            dropped.start >= self.sorted_len,
            "only appended entries are truncated"
        );
        for i in dropped {
            let hash = hash(i);
            let slots = self
                .appended
                .get_mut(&hash)
                .expect("appended entries are indexed");
            slots.retain(|slot| *slot as usize != i);
            if slots.is_empty() {
                self.appended.remove(&hash);
            }
        }
    }

    /// The sorted entry `order` places at `Equal`, given how each entry
    /// orders against the one sought.
    pub(crate) fn search(&self, order: impl Fn(usize) -> Ordering) -> Option<usize> {
        let (mut lo, mut hi) = (0, self.sorted_len);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match order(mid) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    /// The first sorted entry for which `before` is false; `before` must hold
    /// for a leading run of the sorted prefix.
    pub(crate) fn partition_point(&self, before: impl Fn(usize) -> bool) -> usize {
        let (mut lo, mut hi) = (0, self.sorted_len);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if before(mid) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The appended entries whose key hashes to `hash`.
    pub(crate) fn appended(&self, hash: u64) -> impl Iterator<Item = usize> + '_ {
        self.appended
            .get(&hash)
            .into_iter()
            .flatten()
            .map(|&i| i as usize)
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.appended.len() * 24
    }
}

pub(crate) fn hash_of(key: impl Hash) -> u64 {
    let mut hasher = FxHasher::default();
    key.hash(&mut hasher);
    hasher.finish()
}
