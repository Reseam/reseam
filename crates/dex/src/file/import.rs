// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use super::DexFile;
use crate::error::Result;
use crate::model::{FieldIdx, MethodIdx, Pool, ProtoIdx, StringIdx, TypeIdx};
use crate::options::MAX_POOL_SIZE;
use crate::write::part::{self, PoolSet};
use crate::write::remap::Remap;

/// What became of a class [`DexFile::import_class`] was asked to copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassImport {
    /// The class is in the DEX now, at this index.
    Imported(usize),
    /// Its ids would take a pool past the DEX id limits.
    Full,
    /// One of its strings has an unpaired surrogate, which an added string
    /// cannot spell.
    Unspellable,
}

/// Where the pool entries of one DEX landed in another while its classes were
/// imported, so classes that share entries look each up once. It holds for as
/// long as nothing is undone in the importing DEX.
pub struct ImportedPools {
    placed: Remap,
    /// The entries the import under way placed, which go if it is undone.
    pending: Vec<(Pool, u32)>,
}

impl ImportedPools {
    /// No entry of `source` imported yet.
    pub fn none(source: &DexFile) -> Self {
        Self {
            placed: Remap::unplaced(|pool| source.pool_len(pool)),
            pending: Vec::new(),
        }
    }

    fn place(&mut self, pool: Pool, idx: u32, written: u32) {
        self.placed.place(pool, idx, written);
        self.pending.push((pool, idx));
    }

    /// The entries of `pool` among `pools` that are not placed yet.
    fn missing(&self, pools: &PoolSet, pool: Pool) -> Vec<u32> {
        pools
            .members(pool)
            .filter(|&idx| !self.placed.places(pool, idx))
            .collect()
    }
}

impl DexFile {
    /// Copies class `class_idx` of `source` into this DEX with every pool entry
    /// it reaches. `pools` is what earlier imports from `source` into this DEX
    /// placed. A class that is not imported leaves this DEX as it was.
    pub fn import_class(
        &mut self,
        source: &DexFile,
        class_idx: usize,
        pools: &mut ImportedPools,
    ) -> Result<ClassImport> {
        let savepoint = self
            .journal
            .innermost()
            .map_or(Savepoint::FIRST, Savepoint::next);
        self.open_savepoint(savepoint);
        let import = self
            .copy_class(source, class_idx, pools)
            .map(|import| match import {
                ClassImport::Imported(_) if part::overflows(|pool| self.pool_len(pool)) => {
                    ClassImport::Full
                }
                import => import,
            });
        match import {
            Ok(ClassImport::Imported(_)) => {
                self.release_savepoint(savepoint);
                pools.pending.clear();
            }
            Ok(ClassImport::Full | ClassImport::Unspellable) | Err(_) => {
                self.rollback_to(savepoint);
                for (pool, idx) in pools.pending.drain(..) {
                    pools.placed.unplace(pool, idx);
                }
            }
        }
        import
    }

    /// The fewest entries any id pool can still take before the DEX is full.
    pub fn id_headroom(&self) -> usize {
        Pool::ALL
            .into_iter()
            .filter(|&pool| pool != Pool::String)
            .map(|pool| MAX_POOL_SIZE.saturating_sub(self.pool_len(pool)))
            .min()
            .expect("a DEX has id pools")
    }

    fn copy_class(
        &mut self,
        source: &DexFile,
        class_idx: usize,
        pools: &mut ImportedPools,
    ) -> Result<ClassImport> {
        let reached = part::of_class(source, class_idx)?.pools;
        if !self.intern_pools(source, &reached, pools)? {
            return Ok(ClassImport::Unspellable);
        }
        let mut class = source
            .classes
            .read(class_idx, source.parse_options)?
            .into_owned();
        pools.placed.remap_class(&mut class)?;
        Ok(ClassImport::Imported(self.add_class(class)))
    }

    /// Interns the entries of `source` among `reached` that `pools` has not
    /// placed, each pool after the pools its entries refer to. `false` when a
    /// string has an unpaired surrogate.
    fn intern_pools(
        &mut self,
        source: &DexFile,
        reached: &PoolSet,
        pools: &mut ImportedPools,
    ) -> Result<bool> {
        for idx in pools.missing(reached, Pool::String) {
            let Ok(text) = String::from_utf16(&source.strings.units(StringIdx(idx))) else {
                return Ok(false);
            };
            pools.place(Pool::String, idx, self.intern_string(&text).0);
        }
        for idx in pools.missing(reached, Pool::Type) {
            let written = self.intern_type(&source.type_descriptor(TypeIdx(idx)))?;
            pools.place(Pool::Type, idx, written.0);
        }
        for idx in pools.missing(reached, Pool::Proto) {
            let descriptor = source.proto_descriptor(&source.proto(ProtoIdx(idx)));
            pools.place(Pool::Proto, idx, self.intern_proto(&descriptor)?.0);
        }
        for idx in pools.missing(reached, Pool::Field) {
            let field = source.field_id(FieldIdx(idx));
            let written = self.intern_field(
                &source.type_descriptor(field.class),
                &source.strings.get(field.name),
                &source.type_descriptor(field.type_),
            )?;
            pools.place(Pool::Field, idx, written.0);
        }
        for idx in pools.missing(reached, Pool::Method) {
            let method = source.method_id(MethodIdx(idx));
            let written = self.intern_method(
                &source.type_descriptor(method.class),
                &source.strings.get(method.name),
                &source.proto_descriptor(&source.proto(method.proto)),
            )?;
            pools.place(Pool::Method, idx, written.0);
        }
        for idx in pools.missing(reached, Pool::MethodHandle) {
            let mut handle = *source.method_handles.get(idx as usize)?;
            pools.placed.remap_method_handle(&mut handle)?;
            let written = self.intern_method_handle(handle, None)?;
            pools.place(Pool::MethodHandle, idx, written.0);
        }
        for idx in pools.missing(reached, Pool::CallSite) {
            let mut site = source.call_sites.get(idx as usize)?.into_owned();
            pools.placed.remap_call_site(&mut site)?;
            let written = self.intern_call_site(site, None)?;
            pools.place(Pool::CallSite, idx, written.0);
        }
        Ok(true)
    }
}
