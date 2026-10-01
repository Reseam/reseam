// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::MAX_POOL_SIZE;
use crate::error::{Result, invalid};
use crate::file::DexFile;
use crate::references::{self as refs, RefSink, pool_len};
use crate::types::Pool;
use crate::types::method_handle::MethodHandleMember;

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
            if packer.pools.overflows() {
                return Err(invalid(
                    "class_defs",
                    format!("class {class_idx} alone exceeds the DEX id limits"),
                ));
            }
        }
        classes.push(class_idx);
    }
    parts.push(DexPart {
        classes,
        pools: packer.pools,
    });
    Ok(Some(parts))
}

pub(crate) fn overflows(len: impl Fn(Pool) -> usize) -> bool {
    Pool::ALL
        .into_iter()
        .any(|pool| pool != Pool::String && len(pool) > MAX_POOL_SIZE)
}

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

    pub(crate) fn contains(&self, pool: Pool, index: u32) -> bool {
        self.bits[pool as usize]
            .get(index as usize / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    fn overflows(&self) -> bool {
        overflows(|pool| self.len(pool))
    }

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

struct Packer<'a> {
    dex: &'a DexFile,
    pools: PoolSet,
    added: Vec<(Pool, u32)>,
    error: Option<crate::DexError>,
}

impl<'a> Packer<'a> {
    fn new(dex: &'a DexFile) -> Self {
        Self {
            dex,
            pools: PoolSet::new(dex),
            added: Vec::new(),
            error: None,
        }
    }

    fn add_class(&mut self, class_idx: usize) -> Result<()> {
        self.added.clear();
        let dex = self.dex;
        match (
            dex.classes.resident(class_idx),
            dex.classes.raw_def(class_idx),
        ) {
            (Some(class), _) => refs::class(self, class, dex)?,
            (None, Some(raw)) => {
                let buf = dex.raw_buffer().ok_or_else(|| {
                    invalid("DEX source", "deferred classes require the original buffer")
                })?;
                refs::raw_class(self, buf, &raw, dex.parse_options, dex.write_options())?;
            }
            (None, None) => {
                return Err(invalid(
                    "DEX part",
                    format!("class index {class_idx} has no source"),
                ));
            }
        }
        match self.error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn undo(&mut self) {
        for (pool, idx) in self.added.drain(..) {
            self.pools.remove(pool, idx);
        }
    }
}

impl RefSink for Packer<'_> {
    fn add(&mut self, pool: Pool, idx: u32) {
        if self.error.is_some() {
            return;
        }
        if idx as usize >= pool_len(self.dex, pool) {
            self.error = Some(invalid(
                "pool reference",
                format!("{pool:?} index {idx} is out of bounds"),
            ));
            return;
        }
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
                self.add(Pool::Proto, method.proto.0);
                self.add(Pool::String, method.name.0);
            }
            Pool::CallSite => match dex.call_sites.get(idx as usize) {
                Ok(site) => refs::call_site(self, &site),
                Err(error) => self.error = Some(error),
            },
            Pool::MethodHandle => match dex.method_handles.get(idx as usize) {
                Err(error) => self.error = Some(error),
                Ok(handle) => match handle.member {
                    MethodHandleMember::Field(field) => self.add(Pool::Field, field.0),
                    MethodHandleMember::Method(method) => self.add(Pool::Method, method.0),
                },
            },
        }
    }
}
