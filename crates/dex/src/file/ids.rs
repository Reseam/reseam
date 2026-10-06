// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use super::DexBytes;
use crate::error::Result;
use crate::read::{read_u32, u16_at, u32_at};
use crate::types::{FieldId, MethodId, ProtoIdx, Prototype, StringIdx, TypeIdx, TypeList};

/// A fixed-size id-table record that can be read straight from the buffer.
///
/// Records compare and hash by their DEX sort key, which is what the format
/// orders the table by and what lookups search for.
pub trait IdRecord: Clone {
    const SIZE: usize;

    fn read(buf: &[u8], off: usize) -> Self;
    fn validate(buf: &[u8], off: usize) -> Result<()>;
    fn key_cmp(&self, other: &Self) -> Ordering;
    fn key_hash<H: Hasher>(&self, state: &mut H);
}

/// An id table left in the file: records are decoded on access, and only the
/// entries interned after parse are owned.
///
/// The leading `sorted_len` entries are in DEX sort order (the format
/// requires it), so lookups binary-search them; entries after that are found
/// through a small hash index.
#[derive(Debug, Clone)]
pub struct IdTable<T> {
    raw: Option<DexBytes>,
    off: usize,
    raw_len: usize,
    tail: Vec<T>,
    sorted_len: usize,
    index: FxHashMap<u64, SmallVec<[u32; 1]>>,
}

impl<T> Default for IdTable<T> {
    fn default() -> Self {
        Self {
            raw: None,
            off: 0,
            raw_len: 0,
            tail: Vec::new(),
            sorted_len: 0,
            index: FxHashMap::default(),
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
        table.rebuild_index();
        Ok(table)
    }

    pub fn from_vec(tail: Vec<T>) -> Self {
        let mut table = Self {
            tail,
            ..Self::default()
        };
        table.rebuild_index();
        table
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
        self.index
            .entry(hash_key(&record))
            .or_default()
            .push(i as u32);
        self.tail.push(record);
        i
    }

    /// Index of the entry whose sort key equals `probe`'s.
    pub fn find(&self, probe: &T) -> Option<usize> {
        if let Ok(i) = self.binary_search(probe) {
            return Some(i);
        }
        self.index
            .get(&hash_key(probe))?
            .iter()
            .map(|&i| i as usize)
            .find(|&i| self.get(i).key_cmp(probe) == Ordering::Equal)
    }

    /// Indices of the entries in one contiguous run of the sort order, as
    /// `locate` places them: `Less` before the run, `Equal` inside it,
    /// `Greater` after. The sorted prefix is binary-searched; entries interned
    /// since parse are checked one by one.
    pub fn matching<'a>(
        &'a self,
        locate: impl Fn(&T) -> Ordering + 'a,
    ) -> impl Iterator<Item = usize> + 'a {
        let start = self.partition_point(|entry| locate(entry) == Ordering::Less);
        let end = self.partition_point(|entry| locate(entry) != Ordering::Greater);
        (start..end).chain(
            (self.sorted_len..self.len()).filter(move |&i| locate(&self.get(i)) == Ordering::Equal),
        )
    }

    /// Whether every entry is already in DEX sort order.
    pub fn is_sorted(&self) -> bool {
        self.sorted_len == self.len()
    }

    pub fn heap_bytes(&self) -> u64 {
        (self.tail.len() * size_of::<T>() + self.index.len() * 24) as u64
    }

    fn partition_point(&self, before: impl Fn(&T) -> bool) -> usize {
        let mut lo = 0usize;
        let mut hi = self.sorted_len;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if before(&self.get(mid)) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    fn binary_search(&self, probe: &T) -> std::result::Result<usize, usize> {
        let mut lo = 0usize;
        let mut hi = self.sorted_len;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.get(mid).key_cmp(probe) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Ok(mid),
            }
        }
        Err(lo)
    }

    fn rebuild_index(&mut self) {
        let len = self.len();
        self.sorted_len = (1..len)
            .find(|&i| self.get(i - 1).key_cmp(&self.get(i)) != Ordering::Less)
            .unwrap_or(len);
        let mut index: FxHashMap<u64, SmallVec<[u32; 1]>> = FxHashMap::default();
        for i in self.sorted_len..len {
            index
                .entry(hash_key(&self.get(i)))
                .or_default()
                .push(i as u32);
        }
        self.index = index;
    }

    fn raw_bytes(&self) -> &[u8] {
        self.raw
            .as_ref()
            .expect("raw entries exist only while the buffer is retained")
            .as_bytes()
    }
}

fn hash_key<T: IdRecord>(record: &T) -> u64 {
    let mut hasher = FxHasher::default();
    record.key_hash(&mut hasher);
    hasher.finish()
}

fn check(buf: &[u8], off: usize, size: usize) -> Result<()> {
    crate::error::require_len(buf, off, size, "id table entry")
}

impl IdRecord for StringIdx {
    const SIZE: usize = 4;

    fn read(buf: &[u8], off: usize) -> Self {
        StringIdx(u32_at(buf, off))
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl IdRecord for Prototype {
    const SIZE: usize = 12;

    fn read(buf: &[u8], off: usize) -> Self {
        let params_off = u32_at(buf, off + 8) as usize;
        let parameters = if params_off == 0 {
            TypeList::new()
        } else {
            read_type_list(buf, params_off)
        };
        Prototype {
            shorty: StringIdx(u32_at(buf, off)),
            return_type: TypeIdx(u32_at(buf, off + 4)),
            parameters,
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)?;
        let params_off = u32_at(buf, off + 8);
        if params_off != 0 {
            validate_type_list(buf, params_off as usize)?;
        }
        Ok(())
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.return_type
            .cmp(&other.return_type)
            .then_with(|| self.parameters.cmp(&other.parameters))
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.return_type.hash(state);
        self.parameters.hash(state);
    }
}

impl IdRecord for FieldId {
    const SIZE: usize = 8;

    fn read(buf: &[u8], off: usize) -> Self {
        FieldId {
            class: TypeIdx(u32::from(u16_at(buf, off))),
            type_: TypeIdx(u32::from(u16_at(buf, off + 2))),
            name: StringIdx(u32_at(buf, off + 4)),
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

impl IdRecord for MethodId {
    const SIZE: usize = 8;

    fn read(buf: &[u8], off: usize) -> Self {
        MethodId {
            class: TypeIdx(u32::from(u16_at(buf, off))),
            proto: ProtoIdx(u32::from(u16_at(buf, off + 2))),
            name: StringIdx(u32_at(buf, off + 4)),
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

pub(crate) fn read_type_list(buf: &[u8], off: usize) -> TypeList {
    let size = u32_at(buf, off) as usize;
    (0..size)
        .map(|i| TypeIdx(u32::from(u16_at(buf, off + 4 + i * 2))))
        .collect()
}

pub(crate) fn validate_type_list(buf: &[u8], off: usize) -> Result<()> {
    let size = read_u32(buf, off)? as usize;
    crate::error::require_array(buf, off + 4, size, 2, "type list")?;
    Ok(())
}
