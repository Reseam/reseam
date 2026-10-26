// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod build;
mod postings;
mod resident;

pub(crate) use resident::ClassRefs;

use smallvec::SmallVec;

use crate::model::{FieldIdx, MethodIdx, Pool, StringIdx};
use postings::Postings;

/// A reference a search looks for in method bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKey {
    Method(MethodIdx),
    Field(FieldIdx),
    String(StringIdx),
    Literal(i64),
}

/// The references a scan requires: every key in `all`, and at least one of
/// `any` when it is not empty.
#[derive(Debug, Clone, Default)]
pub struct RefQuery {
    all: SmallVec<[RefKey; 4]>,
    any: SmallVec<[RefKey; 4]>,
}

impl RefQuery {
    pub fn all_of(keys: impl IntoIterator<Item = RefKey>) -> Self {
        Self {
            all: keys.into_iter().collect(),
            any: SmallVec::new(),
        }
    }

    pub fn any_of(keys: impl IntoIterator<Item = RefKey>) -> Self {
        Self {
            all: SmallVec::new(),
            any: keys.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty() && self.any.is_empty()
    }
}

/// A method's position in its DEX: classes in order, each class's direct
/// methods before its virtual ones. Method references are 16-bit, so a DEX
/// defines at most 65536 methods.
pub(crate) type Slot = u16;

/// The methods of one class a query matched, by position within the class.
pub(crate) struct ClassMatches {
    pub(crate) class_idx: usize,
    pub(crate) positions: SmallVec<[Slot; 4]>,
}

/// Which source methods reference each method, field, string and literal.
/// Classes that are resident when it is built, or become resident later, are
/// not covered; scans visit those in full.
#[derive(Debug, Clone)]
pub(crate) struct RefIndex {
    class_start: Vec<u32>,
    /// The classes resident when it was built, ascending. A rollback can return
    /// one to its file record, which the index still does not cover.
    uncovered: Vec<u32>,
    keys: PoolKeys,
    pools: Postings,
    literal_values: Vec<i64>,
    literals: Postings,
}

/// Method, field and string ids numbered as one key range: method ids, then
/// field ids, then string ids.
#[derive(Debug, Clone)]
pub(crate) struct PoolKeys {
    field_base: usize,
    string_base: usize,
    end: usize,
}

impl PoolKeys {
    pub(crate) fn new(methods: usize, fields: usize, strings: usize) -> Self {
        Self {
            field_base: methods,
            string_base: methods + fields,
            end: methods + fields + strings,
        }
    }

    /// The key of a method, field or string id; `None` when the id is outside its pool.
    fn pool_key(&self, pool: Pool, id: u32) -> Option<usize> {
        let (base, end) = match pool {
            Pool::Method => (0, self.field_base),
            Pool::Field => (self.field_base, self.string_base),
            Pool::String => (self.string_base, self.end),
            _ => return None,
        };
        Some(base + id as usize).filter(|&key| key < end)
    }
}

impl RefIndex {
    fn postings(&self, key: RefKey) -> &[Slot] {
        match key {
            RefKey::Method(idx) => self.pool_postings(Pool::Method, idx.0),
            RefKey::Field(idx) => self.pool_postings(Pool::Field, idx.0),
            RefKey::String(idx) => self.pool_postings(Pool::String, idx.0),
            RefKey::Literal(value) => self
                .literal_values
                .binary_search(&value)
                .map_or(&[], |key| self.literals.get(key)),
        }
    }

    fn pool_postings(&self, pool: Pool, id: u32) -> &[Slot] {
        self.keys
            .pool_key(pool, id)
            .map_or(&[], |key| self.pools.get(key))
    }

    /// The methods holding every `all` key and, when `any` is not empty, one
    /// of its keys, grouped by class in class order.
    pub(crate) fn matches(&self, query: &RefQuery) -> Vec<ClassMatches> {
        let mut required: SmallVec<[&[Slot]; 4]> =
            query.all.iter().map(|&key| self.postings(key)).collect();
        required.sort_by_key(|slots| slots.len());
        let mut slots: Option<Vec<Slot>> = None;
        for postings in required {
            slots = Some(match slots {
                None => postings.to_vec(),
                Some(current) => intersect(&current, postings),
            });
        }
        if !query.any.is_empty() {
            let mut union: Vec<Slot> = query
                .any
                .iter()
                .flat_map(|&key| self.postings(key).iter().copied())
                .collect();
            union.sort_unstable();
            union.dedup();
            slots = Some(match slots {
                None => union,
                Some(current) => intersect(&current, &union),
            });
        }
        let mut grouped: Vec<ClassMatches> = Vec::new();
        for slot in slots.unwrap_or_default() {
            let class_idx = self
                .class_start
                .partition_point(|&start| start <= u32::from(slot))
                - 1;
            let position = Slot::try_from(u32::from(slot) - self.class_start[class_idx])
                .expect("a position within a class is below its slot");
            match grouped.last_mut() {
                Some(last) if last.class_idx == class_idx => last.positions.push(position),
                _ => grouped.push(ClassMatches {
                    class_idx,
                    positions: SmallVec::from_slice(&[position]),
                }),
            }
        }
        grouped
    }

    pub(crate) fn uncovered(&self) -> &[u32] {
        &self.uncovered
    }

    pub(crate) fn heap_bytes(&self) -> u64 {
        (self.class_start.len() * size_of::<u32>()
            + self.uncovered.len() * size_of::<u32>()
            + self.pools.heap_bytes()
            + self.literal_values.len() * size_of::<i64>()
            + self.literals.heap_bytes()) as u64
    }
}

fn intersect(sorted: &[Slot], other: &[Slot]) -> Vec<Slot> {
    sorted
        .iter()
        .copied()
        .filter(|slot| other.binary_search(slot).is_ok())
        .collect()
}
