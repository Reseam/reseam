// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use fixedbitset::FixedBitSet;

use crate::error::{Result, invalid};
use crate::file::DexFile;
use crate::model::{PerPool, Pool};
use crate::options::MAX_POOL_SIZE;
use crate::references::{self as refs, Bodies, RefSink};

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
    if !overflows(|pool| dex.pool_len(pool)) {
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

/// The part holding class `class_idx` alone.
pub(crate) fn of_class(dex: &DexFile, class_idx: usize) -> Result<DexPart> {
    let mut packer = Packer::new(dex);
    packer.add_class(class_idx)?;
    Ok(DexPart {
        classes: vec![class_idx],
        pools: packer.pools,
    })
}

pub(crate) fn overflows(len: impl Fn(Pool) -> usize) -> bool {
    Pool::ALL
        .into_iter()
        .any(|pool| pool != Pool::String && len(pool) > MAX_POOL_SIZE)
}

#[derive(Debug, Clone)]
pub(crate) struct PoolSet {
    members: PerPool<FixedBitSet>,
    counts: PerPool<usize>,
}

impl PoolSet {
    fn new(dex: &DexFile) -> Self {
        Self {
            members: PerPool::from_fn(|pool| FixedBitSet::with_capacity(dex.pool_len(pool))),
            counts: PerPool::default(),
        }
    }

    pub(crate) fn len(&self, pool: Pool) -> usize {
        self.counts[pool]
    }

    pub(crate) fn members(&self, pool: Pool) -> impl Iterator<Item = u32> + '_ {
        self.members[pool].ones().map(|index| index as u32)
    }

    pub(crate) fn contains(&self, pool: Pool, index: u32) -> bool {
        self.members[pool].contains(index as usize)
    }

    fn overflows(&self) -> bool {
        overflows(|pool| self.len(pool))
    }

    /// Adds an in-bounds index; whether it was absent.
    fn insert(&mut self, pool: Pool, idx: u32) -> bool {
        let new = !self.members[pool].put(idx as usize);
        self.counts[pool] += usize::from(new);
        new
    }

    fn remove(&mut self, pool: Pool, idx: u32) {
        self.members[pool].set(idx as usize, false);
        self.counts[pool] -= 1;
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
            (Some(class), _) => refs::class(self, class, dex, Bodies::Visit)?,
            (None, Some(raw)) => {
                let buf = dex.raw_buffer().ok_or_else(|| {
                    invalid("DEX source", "deferred classes require the original buffer")
                })?;
                refs::raw_class(
                    self,
                    buf,
                    &raw,
                    dex.parse_options,
                    dex.write_options(),
                    Bodies::Visit,
                )?;
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
        if idx as usize >= self.dex.pool_len(pool) {
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
        if let Err(error) = refs::id_entry(self, self.dex, pool, idx) {
            self.error = Some(error);
        }
    }
}
