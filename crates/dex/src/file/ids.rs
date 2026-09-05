// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use super::DexBytes;
use crate::error::{invalid_offset, Result};
use crate::read::header::{u16_at, u32_at};
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
        let end = off + count * T::SIZE;
        if end > buf.len() {
            return Err(invalid_offset("id table", off as u32, buf.len() as u32));
        }
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

    pub(crate) fn truncate(&mut self, len: usize) {
        if len >= self.raw_len {
            self.tail.truncate(len - self.raw_len);
        } else {
            self.raw_len = len;
            self.tail.clear();
        }
        self.rebuild_index();
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

    /// Whether every entry is already in DEX sort order.
    pub fn is_sorted(&self) -> bool {
        self.sorted_len == self.len()
    }

    pub fn heap_bytes(&self) -> u64 {
        (self.tail.len() * size_of::<T>() + self.index.len() * 24) as u64
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
    if off + size > buf.len() {
        return Err(invalid_offset(
            "id table entry",
            off as u32,
            buf.len() as u32,
        ));
    }
    Ok(())
}

fn u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

impl IdRecord for StringIdx {
    const SIZE: usize = 4;

    fn read(buf: &[u8], off: usize) -> Self {
        StringIdx(u32(buf, off))
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
        let params_off = u32(buf, off + 8) as usize;
        let parameters = if params_off == 0 {
            TypeList::new()
        } else {
            read_type_list(buf, params_off)
        };
        Prototype {
            shorty: StringIdx(u32(buf, off)),
            return_type: TypeIdx(u32(buf, off + 4)),
            parameters,
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)?;
        let params_off = u32_at(buf, off + 8)?;
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
            class: TypeIdx(u16(buf, off) as u32),
            type_: TypeIdx(u16(buf, off + 2) as u32),
            name: StringIdx(u32(buf, off + 4)),
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        (self.class, self.name, self.type_).cmp(&(other.class, other.name, other.type_))
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        (self.class, self.name, self.type_).hash(state);
    }
}

impl IdRecord for MethodId {
    const SIZE: usize = 8;

    fn read(buf: &[u8], off: usize) -> Self {
        MethodId {
            class: TypeIdx(u16(buf, off) as u32),
            proto: ProtoIdx(u16(buf, off + 2)),
            name: StringIdx(u32(buf, off + 4)),
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        (self.class, self.name, self.proto).cmp(&(other.class, other.name, other.proto))
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        (self.class, self.name, self.proto).hash(state);
    }
}

/// Reads a validated `type_list` at `off`.
pub(crate) fn read_type_list(buf: &[u8], off: usize) -> TypeList {
    let size = u32(buf, off) as usize;
    (0..size)
        .map(|i| TypeIdx(u16(buf, off + 4 + i * 2) as u32))
        .collect()
}

pub(crate) fn validate_type_list(buf: &[u8], off: usize) -> Result<()> {
    let size = u32_at(buf, off)? as usize;
    if size > 0 {
        u16_at(buf, off + 4 + (size - 1) * 2)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_methods(records: &[(u16, u16, u32)]) -> IdTable<MethodId> {
        let mut buf = Vec::new();
        for &(class, proto, name) in records {
            buf.extend_from_slice(&class.to_le_bytes());
            buf.extend_from_slice(&proto.to_le_bytes());
            buf.extend_from_slice(&name.to_le_bytes());
        }
        IdTable::from_raw(DexBytes::from_vec(buf), 0, records.len() as u32).unwrap()
    }

    fn method(class: u32, name: u32, proto: u16) -> MethodId {
        MethodId {
            class: TypeIdx(class),
            proto: ProtoIdx(proto),
            name: StringIdx(name),
        }
    }

    #[test]
    fn raw_records_decode_and_lookup() {
        let table = raw_methods(&[(0, 0, 1), (0, 0, 2), (1, 3, 0), (0, 9, 9)]);
        assert_eq!(table.len(), 4);
        assert_eq!(table.sorted_len, 3);
        assert_eq!(table.get(2).name, StringIdx(0));
        assert_eq!(table.find(&method(0, 2, 0)), Some(1));
        assert_eq!(table.find(&method(0, 9, 9)), Some(3));
        assert_eq!(table.find(&method(5, 5, 5)), None);
    }

    #[test]
    fn pushed_records_are_found_and_truncated() {
        let mut table = raw_methods(&[(0, 0, 1)]);
        let i = table.push(method(2, 2, 2));
        assert_eq!(i, 1);
        assert_eq!(table.find(&method(2, 2, 2)), Some(1));
        table.truncate(1);
        assert_eq!(table.find(&method(2, 2, 2)), None);
        table.truncate(0);
        assert!(table.is_empty());
    }

    #[test]
    fn owned_tables_sort_prefix() {
        let table = IdTable::from_vec(vec![method(0, 0, 0), method(0, 1, 0), method(0, 0, 5)]);
        assert_eq!(table.sorted_len, 2);
        assert_eq!(table.find(&method(0, 0, 5)), Some(2));
        assert_eq!(table.find(&method(0, 1, 0)), Some(1));
    }
}
