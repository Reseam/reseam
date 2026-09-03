// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A per-method summary of what a method references, so a search over the
//! whole DEX walks only the methods that can match.
//!
//! Every method gets a 64-bit mask with two bits set per pool reference or
//! literal it uses. A query is the same mask for what it looks for; a method
//! whose mask lacks any query bit cannot contain the reference. False
//! positives only cost a walk, never a miss. Resident classes are never
//! filtered since a patch may have changed them after the masks were built.

use std::hash::{Hash, Hasher};

use rayon::prelude::*;
use rustc_hash::FxHasher;
use smallvec::SmallVec;

use super::DexFile;
use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::Result;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::walk_instructions;
use crate::read::header::u32_at;
use crate::types::{FieldIdx, MethodIdx, StringIdx};

/// What a search looks for, hashed the way method masks are built.
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
        let h = hasher.finish();
        Self(1 << (h & 63) | 1 << ((h >> 6) & 63))
    }
}

/// The references a scan requires: every key in `all`, and at least one of
/// `any` when it is not empty.
#[derive(Debug, Clone, Default)]
pub struct RefQuery {
    all: u64,
    any: SmallVec<[u64; 4]>,
}

impl RefQuery {
    pub fn all_of(keys: impl IntoIterator<Item = RefKey>) -> Self {
        Self {
            all: keys.into_iter().fold(0, |m, k| m | k.0),
            any: SmallVec::new(),
        }
    }

    pub fn any_of(keys: impl IntoIterator<Item = RefKey>) -> Self {
        Self {
            all: 0,
            any: keys.into_iter().map(|k| k.0).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.all == 0 && self.any.is_empty()
    }

    pub(crate) fn admits(&self, mask: u64) -> bool {
        mask & self.all == self.all && (self.any.is_empty() || self.any.iter().any(|&m| m & !mask == 0))
    }
}

/// Masks for every method of every class that was still in the file when
/// the filter was built, in scan order.
#[derive(Debug, Clone)]
pub(crate) struct RefFilter {
    class_start: Vec<u32>,
    masks: Vec<u64>,
}

impl RefFilter {
    pub(crate) fn build(dex: &DexFile) -> Result<Self> {
        let class_count = dex.classes.len();
        let mut class_start = Vec::with_capacity(class_count + 1);
        let mut total = 0u32;
        for class_idx in 0..class_count {
            class_start.push(total);
            if let Some(offset) = dex.raw_class_data_offset(class_idx) {
                let buf = dex.raw_bytes(offset)?;
                let opts = &dex.parse_options;
                let mut pos = offset as usize;
                for _ in 0..2 {
                    pos += read_uleb128_with_opts(buf, pos, opts)?.1;
                }
                let (direct, n) = read_uleb128_with_opts(buf, pos, opts)?;
                pos += n;
                let (virtual_, _) = read_uleb128_with_opts(buf, pos, opts)?;
                total += direct + virtual_;
            }
        }
        class_start.push(total);

        let mut masks = vec![0u64; total as usize];
        let chunks: Vec<(usize, &mut [u64])> = {
            let mut rest: &mut [u64] = &mut masks;
            let mut out = Vec::with_capacity(class_count);
            for class_idx in 0..class_count {
                let len = (class_start[class_idx + 1] - class_start[class_idx]) as usize;
                let (head, tail) = rest.split_at_mut(len);
                rest = tail;
                out.push((class_idx, head));
            }
            out
        };
        chunks.into_par_iter().try_for_each(|(class_idx, slots)| -> Result<()> {
            if slots.is_empty() {
                return Ok(());
            }
            let offset = dex.raw_class_data_offset(class_idx).unwrap();
            let buf = dex.raw_bytes(offset)?;
            let skeleton = read_class_skeleton_at(buf, offset as usize, &dex.parse_options)?;
            let headers = skeleton.direct_methods.iter().chain(&skeleton.virtual_methods);
            for (slot, header) in slots.iter_mut().zip(headers) {
                if header.code_off != 0 {
                    *slot = method_mask(buf, header.code_off)?;
                }
            }
            Ok(())
        })?;

        Ok(Self { class_start, masks })
    }

    /// The masks of a class's methods, direct then virtual.
    pub(crate) fn class(&self, class_idx: usize) -> &[u64] {
        let start = self.class_start[class_idx] as usize;
        let end = self.class_start[class_idx + 1] as usize;
        &self.masks[start..end]
    }

    pub(crate) fn heap_bytes(&self) -> u64 {
        (self.masks.len() * 8 + self.class_start.len() * 4) as u64
    }
}

fn method_mask(buf: &[u8], code_off: u32) -> Result<u64> {
    let base = code_off as usize;
    let insns_size = u32_at(buf, base + 12)? as usize;
    let mut mask = 0u64;
    walk_instructions(buf, base + 16, insns_size, |insn| {
        if let Some(m) = insn.method_ref(buf) {
            mask |= RefKey::method(m).0;
        } else if let Some(f) = insn.field_ref(buf) {
            mask |= RefKey::field(f).0;
        } else if let Some(s) = insn.string_ref(buf) {
            mask |= RefKey::string(s).0;
        } else if let Some(l) = insn.literal(buf) {
            mask |= RefKey::literal(l).0;
        }
        true
    })?;
    Ok(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_admit_supersets_only() {
        let a = RefKey::method(MethodIdx(1));
        let b = RefKey::string(StringIdx(7));
        let c = RefKey::literal(-3);
        let mask = a.0 | b.0;
        assert!(RefQuery::all_of([a, b]).admits(mask));
        assert!(RefQuery::any_of([c, b]).admits(mask));
        assert!(!RefQuery::any_of([c]).admits(mask) || c.0 & mask == c.0);
        assert!(RefQuery::default().admits(0));
        assert!(!RefQuery::all_of([a]).admits(0));
    }
}
