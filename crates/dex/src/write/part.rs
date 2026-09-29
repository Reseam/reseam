// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Splitting a DEX whose id pools outgrew the 16-bit limit. Each part is a
//! run of the file's classes written as its own DEX with only the pool
//! entries those classes reach, straight from the original buffer, so no
//! class is materialized and the [`DexFile`] is left as it is.

use super::refs::{self, RefSink};
use super::MAX_POOL_SIZE;
use crate::error::{invalid, Result};
use crate::file::DexFile;
use crate::types::method_handle::MethodHandleMember;
use crate::types::Pool;

/// Classes of one DEX that are written together, with every pool entry
/// they reach.
#[derive(Debug, Clone)]
pub struct DexPart {
    pub(crate) classes: Vec<usize>,
    pub(crate) pools: PoolSet,
}

impl DexPart {
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }
}

/// Splits `dex` into parts that each fit the id limits, keeping its classes
/// in table order. `None` when the file fits as it is.
pub fn split_to_fit(dex: &DexFile) -> Result<Option<Vec<DexPart>>> {
    if !overflows(|pool| pool_len(dex, pool)) {
        return Ok(None);
    }
    let mut parts = Vec::new();
    let mut packer = Packer::new(dex);
    let mut classes = Vec::new();
    for class_idx in 0..dex.classes.len() {
        packer.add_class(class_idx)?;
        if packer.pools.overflows() {
            if classes.is_empty() {
                return Err(invalid(
                    "class_defs",
                    format!("class {class_idx} alone exceeds the DEX id limits"),
                ));
            }
            packer.undo();
            parts.push(DexPart {
                classes: std::mem::take(&mut classes),
                pools: std::mem::replace(&mut packer.pools, PoolSet::new(dex)),
            });
            packer.add_class(class_idx)?;
        }
        classes.push(class_idx);
    }
    parts.push(DexPart {
        classes,
        pools: packer.pools,
    });
    Ok(Some(parts))
}

pub(crate) fn pool_len(dex: &DexFile, pool: Pool) -> usize {
    match pool {
        Pool::String => dex.strings.len(),
        Pool::Type => dex.types.len(),
        Pool::Proto => dex.prototypes.len(),
        Pool::Field => dex.fields.len(),
        Pool::Method => dex.methods.len(),
        Pool::CallSite => dex.call_sites.len(),
        Pool::MethodHandle => dex.method_handles.len(),
    }
}

/// Whether any pool indexed by 16-bit operands holds more entries than they
/// can address. Strings are exempt: `const-string/jumbo` reaches them all.
pub(crate) fn overflows(len: impl Fn(Pool) -> usize) -> bool {
    Pool::ALL
        .into_iter()
        .any(|pool| pool != Pool::String && len(pool) > MAX_POOL_SIZE)
}

/// A subset of every pool of one DEX.
#[derive(Debug, Clone)]
pub(crate) struct PoolSet {
    bits: [Vec<u64>; 7],
    counts: [usize; 7],
}

impl PoolSet {
    fn new(dex: &DexFile) -> Self {
        Self {
            bits: Pool::ALL.map(|pool| vec![0; pool_len(dex, pool).div_ceil(64)]),
            counts: [0; 7],
        }
    }

    pub(crate) fn len(&self, pool: Pool) -> usize {
        self.counts[pool as usize]
    }

    /// Members of `pool` in ascending index order.
    pub(crate) fn members(&self, pool: Pool) -> impl Iterator<Item = u32> + '_ {
        self.bits[pool as usize]
            .iter()
            .enumerate()
            .flat_map(|(word_idx, &word)| {
                let mut word = word;
                std::iter::from_fn(move || {
                    (word != 0).then(|| {
                        let bit = word.trailing_zeros();
                        word &= word - 1;
                        (word_idx * 64) as u32 + bit
                    })
                })
            })
    }

    fn overflows(&self) -> bool {
        overflows(|pool| self.len(pool))
    }

    /// Adds `idx`, returning whether it was new.
    fn insert(&mut self, pool: Pool, idx: u32) -> bool {
        let (word, bit) = (idx as usize / 64, 1u64 << (idx % 64));
        let slot = &mut self.bits[pool as usize][word];
        let new = *slot & bit == 0;
        *slot |= bit;
        self.counts[pool as usize] += usize::from(new);
        new
    }

    fn remove(&mut self, pool: Pool, idx: u32) {
        self.bits[pool as usize][idx as usize / 64] &= !(1u64 << (idx % 64));
        self.counts[pool as usize] -= 1;
    }
}

/// Grows a [`PoolSet`] one class at a time, closing it over the entries
/// each new entry references and remembering what the last class added.
struct Packer<'a> {
    dex: &'a DexFile,
    pools: PoolSet,
    added: Vec<(Pool, u32)>,
}

impl<'a> Packer<'a> {
    fn new(dex: &'a DexFile) -> Self {
        Self {
            dex,
            pools: PoolSet::new(dex),
            added: Vec::new(),
        }
    }

    fn add_class(&mut self, class_idx: usize) -> Result<()> {
        self.added.clear();
        let dex = self.dex;
        match (
            dex.classes.resident(class_idx),
            dex.classes.raw_def(class_idx),
        ) {
            (Some(class), _) => refs::class(self, class),
            (None, Some(raw)) => {
                let buf = dex
                    .raw_buffer()
                    .expect("file classes exist only while the buffer is retained");
                refs::raw_class(self, buf, &raw, &dex.parse_options)?;
            }
            (None, None) => unreachable!("every slot is resident or raw"),
        }
        Ok(())
    }

    fn undo(&mut self) {
        for (pool, idx) in self.added.drain(..) {
            self.pools.remove(pool, idx);
        }
    }
}

impl RefSink for Packer<'_> {
    fn add(&mut self, pool: Pool, idx: u32) {
        if !self.pools.insert(pool, idx) {
            return;
        }
        self.added.push((pool, idx));
        let dex = self.dex;
        match pool {
            Pool::String => {}
            Pool::Type => self.add(Pool::String, dex.types.get(idx as usize).0),
            Pool::Proto => {
                let proto = dex.prototypes.get(idx as usize);
                self.add(Pool::String, proto.shorty.0);
                self.add(Pool::Type, proto.return_type.0);
                for param in &proto.parameters {
                    self.add(Pool::Type, param.0);
                }
            }
            Pool::Field => {
                let field = dex.fields.get(idx as usize);
                self.add(Pool::Type, field.class.0);
                self.add(Pool::Type, field.type_.0);
                self.add(Pool::String, field.name.0);
            }
            Pool::Method => {
                let method = dex.methods.get(idx as usize);
                self.add(Pool::Type, method.class.0);
                self.add(Pool::Proto, method.proto.0 as u32);
                self.add(Pool::String, method.name.0);
            }
            Pool::CallSite => refs::call_site(self, &dex.call_sites[idx as usize]),
            Pool::MethodHandle => match dex.method_handles[idx as usize].member {
                MethodHandleMember::Field(field) => self.add(Pool::Field, field.0),
                MethodHandleMember::Method(method) => self.add(Pool::Method, method.0),
            },
        }
    }
}
