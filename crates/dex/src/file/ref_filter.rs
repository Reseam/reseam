// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::hash::{Hash, Hasher};

use rayon::prelude::*;
use rustc_hash::FxHasher;
use smallvec::SmallVec;

use super::DexFile;
use crate::error::Result;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::walk_instructions;
use crate::read::read_u32;
use crate::types::{FieldIdx, MethodIdx, StringIdx};

const BITS_PER_KEY: usize = 16;
const MAX_WORDS: usize = 64;

/// What a search looks for, hashed the way method filters are built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefKey(u64);

impl RefKey {
    pub fn method(idx: MethodIdx) -> Self {
        Self::of(0, u64::from(idx.0))
    }

    pub fn field(idx: FieldIdx) -> Self {
        Self::of(1, u64::from(idx.0))
    }

    pub fn string(idx: StringIdx) -> Self {
        Self::of(2, u64::from(idx.0))
    }

    pub fn literal(value: i64) -> Self {
        Self::of(3, value as u64)
    }

    fn of(kind: u8, value: u64) -> Self {
        let mut hasher = FxHasher::default();
        (kind, value).hash(&mut hasher);
        let mut h = hasher.finish();
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        Self(h ^ (h >> 33))
    }

    fn bits(self, words: usize) -> [usize; 2] {
        let mask = words * 64 - 1;
        [self.0 as usize & mask, (self.0 >> 32) as usize & mask]
    }

    fn insert(self, filter: &mut [u64]) {
        for bit in self.bits(filter.len()) {
            filter[bit / 64] |= 1 << (bit % 64);
        }
    }

    fn is_in(self, filter: &[u64]) -> bool {
        !filter.is_empty()
            && self
                .bits(filter.len())
                .iter()
                .all(|&bit| filter[bit / 64] & 1 << (bit % 64) != 0)
    }
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

    pub(crate) fn admits(&self, filter: &[u64]) -> bool {
        self.all.iter().all(|key| key.is_in(filter))
            && (self.any.is_empty() || self.any.iter().any(|key| key.is_in(filter)))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RefFilter {
    class_start: Vec<u32>,
    method_start: Vec<u32>,
    words: Vec<u64>,
}

#[derive(Clone, Copy)]
pub(crate) struct ClassFilter<'a> {
    starts: &'a [u32],
    words: &'a [u64],
}

impl ClassFilter<'_> {
    pub(crate) fn method(&self, slot: usize) -> &[u64] {
        &self.words[self.starts[slot] as usize..self.starts[slot + 1] as usize]
    }

    pub(crate) fn admits_any(&self, query: &RefQuery) -> bool {
        (0..self.starts.len() - 1).any(|slot| query.admits(self.method(slot)))
    }
}

impl RefFilter {
    pub(crate) fn build(dex: &DexFile) -> Result<Self> {
        let per_class: Vec<(Vec<u32>, Vec<u64>)> = (0..dex.classes.len())
            .into_par_iter()
            .map(|class_idx| class_filters(dex, class_idx))
            .collect::<Result<_>>()?;
        let mut class_start = Vec::with_capacity(per_class.len() + 1);
        let mut method_start = vec![0u32];
        let mut words = Vec::new();
        for (sizes, class_words) in per_class {
            class_start.push(method_start.len() as u32 - 1);
            for size in sizes {
                method_start.push(
                    method_start
                        .last()
                        .expect("prefix offsets always include the initial zero")
                        + size,
                );
            }
            words.extend(class_words);
        }
        class_start.push(method_start.len() as u32 - 1);
        Ok(Self {
            class_start,
            method_start,
            words,
        })
    }

    pub(crate) fn class(&self, class_idx: usize) -> ClassFilter<'_> {
        let start = self.class_start[class_idx] as usize;
        let end = self.class_start[class_idx + 1] as usize;
        ClassFilter {
            starts: &self.method_start[start..=end],
            words: &self.words,
        }
    }

    pub(crate) fn heap_bytes(&self) -> u64 {
        (self.words.len() * 8 + (self.method_start.len() + self.class_start.len()) * 4) as u64
    }
}

fn class_filters(dex: &DexFile, class_idx: usize) -> Result<(Vec<u32>, Vec<u64>)> {
    let Some(offset) = dex.raw_class_data_offset(class_idx) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let buf = dex.raw_bytes(offset)?;
    let skeleton = read_class_skeleton_at(buf, offset as usize, dex.parse_options)?;
    let mut sizes = Vec::new();
    let mut words = Vec::new();
    let mut keys = Vec::new();
    for header in skeleton
        .direct_methods
        .iter()
        .chain(&skeleton.virtual_methods)
    {
        keys.clear();
        if header.code_off != 0 {
            method_keys(buf, header.code_off, &mut keys)?;
        }
        let size = if keys.is_empty() {
            0
        } else {
            (keys.len() * BITS_PER_KEY)
                .div_ceil(64)
                .next_power_of_two()
                .min(MAX_WORDS)
        };
        let start = words.len();
        words.resize(start + size, 0);
        for key in &keys {
            key.insert(&mut words[start..]);
        }
        sizes.push(size as u32);
    }
    Ok((sizes, words))
}

fn method_keys(buf: &[u8], code_off: u32, keys: &mut Vec<RefKey>) -> Result<()> {
    let base = code_off as usize;
    let insns_size = read_u32(buf, base + 12)? as usize;
    walk_instructions(buf, base + 16, insns_size, |insn| {
        if let Some(m) = insn.method_ref(buf) {
            keys.push(RefKey::method(m));
        } else if let Some(f) = insn.field_ref(buf) {
            keys.push(RefKey::field(f));
        } else if let Some(s) = insn.string_ref(buf) {
            keys.push(RefKey::string(s));
        } else if let Some(l) = insn.literal(buf) {
            keys.push(RefKey::literal(l));
        }
        true
    })?;
    keys.sort_unstable_by_key(|key| key.0);
    keys.dedup();
    Ok(())
}
