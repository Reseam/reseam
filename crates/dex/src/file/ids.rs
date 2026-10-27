// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;
use std::hash::Hash;

mod records;

use super::sorted_index::{SortedIndex, hash_of};
use crate::DexBytes;
use crate::error::Result;

/// A fixed-size id-table record that can be read straight from the buffer.
///
/// Records sort and hash by their DEX sort key, which is what the format
/// orders the table by and what lookups search for.
pub trait IdRecord: Clone {
    const SIZE: usize;
    type Key<'a>: Ord + Hash
    where
        Self: 'a;

    fn read(buf: &[u8], off: usize) -> Self;
    /// Checks what the record at `off` refers to; its table has already
    /// bounds-checked the record itself.
    fn validate(_buf: &[u8], _off: usize) -> Result<()> {
        Ok(())
    }
    fn key(&self) -> Self::Key<'_>;
}

/// An id table left in the file: records are decoded on access, and only the
/// entries interned after parse are owned.
#[derive(Debug, Clone)]
pub struct IdTable<T> {
    raw: Option<DexBytes>,
    off: usize,
    raw_len: usize,
    tail: Vec<T>,
    index: SortedIndex,
}

impl<T> Default for IdTable<T> {
    fn default() -> Self {
        Self {
            raw: None,
            off: 0,
            raw_len: 0,
            tail: Vec::new(),
            index: SortedIndex::default(),
        }
    }
}

impl<T: IdRecord> IdTable<T> {
    pub(crate) fn from_raw(raw: DexBytes, off: u32, count: u32) -> Result<Self> {
        let buf = raw.as_bytes();
        let off = off as usize;
        let count = count as usize;
        crate::error::require_array(buf, off, count, T::SIZE, "id table")?;
        for i in 0..count {
            T::validate(buf, off + i * T::SIZE)?;
        }
        let mut table = Self {
            raw: Some(raw),
            off,
            raw_len: count,
            ..Self::default()
        };
        table.index = SortedIndex::new(
            count,
            |i| key_order(&table.get(i - 1), &table.get(i)) == Ordering::Less,
            |i| hash_of(table.get(i).key()),
        );
        Ok(table)
    }

    pub fn from_vec(tail: Vec<T>) -> Self {
        let index = SortedIndex::new(
            tail.len(),
            |i| key_order(&tail[i - 1], &tail[i]) == Ordering::Less,
            |i| hash_of(tail[i].key()),
        );
        Self {
            tail,
            index,
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.raw_len + self.tail.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, i: usize) -> T {
        if i < self.raw_len {
            T::read(self.raw_bytes(), self.off + i * T::SIZE)
        } else {
            self.tail[i - self.raw_len].clone()
        }
    }

    pub fn try_get(&self, i: usize) -> Option<T> {
        (i < self.len()).then(|| self.get(i))
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = T> + '_ {
        (0..self.len()).map(|i| self.get(i))
    }

    pub fn to_vec(&self) -> Vec<T> {
        self.iter().collect()
    }

    pub fn push(&mut self, record: T) -> usize {
        let i = self.len();
        self.index.push(hash_of(record.key()), i);
        self.tail.push(record);
        i
    }

    /// Drops the entries pushed after the table held `len`.
    pub(crate) fn truncate(&mut self, len: usize) {
        assert!(len >= self.raw_len, "only pushed entries are truncated");
        self.index.truncate(len..self.len(), |i| {
            hash_of(self.tail[i - self.raw_len].key())
        });
        self.tail.truncate(len - self.raw_len);
    }

    /// Index of the entry whose sort key equals `probe`'s.
    pub fn find(&self, probe: &T) -> Option<usize> {
        self.index
            .search(|i| key_order(&self.get(i), probe))
            .or_else(|| {
                self.index
                    .appended(hash_of(probe.key()))
                    .find(|&i| key_order(&self.get(i), probe) == Ordering::Equal)
            })
    }

    /// Indices of the entries in one contiguous run of the sort order, as
    /// `locate` places them: `Less` before the run, `Equal` inside it,
    /// `Greater` after. The sorted prefix is binary-searched; entries interned
    /// since parse are checked one by one.
    pub fn matching<'a>(
        &'a self,
        locate: impl Fn(&T) -> Ordering + 'a,
    ) -> impl Iterator<Item = usize> + 'a {
        let start = self
            .index
            .partition_point(|i| locate(&self.get(i)) == Ordering::Less);
        let end = self
            .index
            .partition_point(|i| locate(&self.get(i)) != Ordering::Greater);
        (start..end).chain(
            (self.index.sorted_len()..self.len())
                .filter(move |&i| locate(&self.get(i)) == Ordering::Equal),
        )
    }

    /// Whether every entry is already in DEX sort order.
    pub fn is_sorted(&self) -> bool {
        self.index.sorted_len() == self.len()
    }

    /// How many leading entries are in DEX sort order.
    pub(crate) fn sorted_len(&self) -> usize {
        self.index.sorted_len()
    }

    pub fn heap_bytes(&self) -> u64 {
        (self.tail.len() * size_of::<T>() + self.index.heap_bytes()) as u64
    }

    fn raw_bytes(&self) -> &[u8] {
        self.raw
            .as_ref()
            .expect("raw entries exist only while the buffer is retained")
            .as_bytes()
    }
}

fn key_order<'a, T: IdRecord>(a: &'a T, b: &'a T) -> Ordering {
    a.key().cmp(&b.key())
}
