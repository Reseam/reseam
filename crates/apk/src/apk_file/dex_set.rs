// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use rayon::prelude::*;
use reseam_dex::{
    DexFile, DexPart, MaterializationStats, MemoryBreakdown, ParseOptions, Spooled, WriteLayout,
};
use reseam_storage::{Bytes, MappedFile};

use super::{ApkComponent, ComponentIndex};
use crate::entry::EntryName;
use crate::error::Result;
use crate::zip::reader;

/// Every `classes*.dex` of an APK and its splits, in load order. A deleted
/// entry leaves an empty DEX in its slot, so a slot index stays the identity
/// of its DEX for the whole session.
#[derive(Default)]
pub struct DexSet {
    slots: Vec<Slot>,
}

struct Slot {
    dex: DexFile,
    origin: DexOrigin,
}

/// The entry a slot's DEX came from, or the one it is written to.
pub(super) struct DexOrigin {
    pub(super) component: ComponentIndex,
    pub(super) name: EntryName,
    pub(super) kind: DexSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DexSource {
    Archive,
    Added,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct DexIndex(pub(super) usize);

/// A DEX, or one part of it, that an output entry holds.
pub(super) struct DexMember {
    pub dex_index: DexIndex,
    pub part: Option<DexPart>,
}

impl DexSet {
    /// Every DEX of `components` in load order: components in order, and within
    /// a component its entries by ordinal and each container's members in order.
    pub(super) fn load(components: &[ApkComponent], opts: ParseOptions) -> Result<Self> {
        let mut dex = Self::default();
        for (index, component) in components.iter().enumerate() {
            let archive = component.archive();
            let entries = reader::dex_entry_names(archive)
                .into_par_iter()
                .map(|name| {
                    let members = reader::map_entry(&mut archive.clone(), &name)
                        .and_then(|mapped| parse_entry(mapped, opts))
                        .map_err(|error| error.in_entry(&name))?;
                    Ok((EntryName::from(name), members))
                })
                .collect::<Result<Vec<_>>>()?;
            for (name, members) in entries {
                for file in members {
                    dex.push(
                        file,
                        DexOrigin {
                            component: ComponentIndex(index),
                            name: name.clone(),
                            kind: DexSource::Archive,
                        },
                    );
                }
            }
        }
        Ok(dex)
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&DexFile> {
        self.slots.get(index).map(|slot| &slot.dex)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &DexFile> {
        self.slots.iter().map(|slot| &slot.dex)
    }

    /// `(slot, class index)` of the class with this descriptor.
    pub fn find_class(&self, descriptor: &str) -> Option<(usize, usize)> {
        self.iter()
            .enumerate()
            .find_map(|(index, dex)| Some((index, dex.find_class_index(descriptor)?)))
    }

    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        self.iter().map(DexFile::memory_breakdown).sum()
    }

    pub fn materialization_stats(&self) -> MaterializationStats {
        self.iter().map(DexFile::materialization_stats).sum()
    }

    pub(super) fn dex(&self, index: DexIndex) -> &DexFile {
        &self.slots[index.0].dex
    }

    pub(super) fn origin(&self, index: DexIndex) -> &DexOrigin {
        &self.slots[index.0].origin
    }

    pub(super) fn slots(&self) -> impl Iterator<Item = (DexIndex, &DexFile, &DexOrigin)> {
        self.slots
            .iter()
            .enumerate()
            .map(|(index, slot)| (DexIndex(index), &slot.dex, &slot.origin))
    }

    /// The DEX of a slot whose entry still exists.
    pub(super) fn live_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.slots
            .get_mut(index)
            .filter(|slot| slot.origin.kind != DexSource::Removed)
            .map(|slot| &mut slot.dex)
    }

    pub(super) fn files_mut(&mut self) -> impl Iterator<Item = &mut DexFile> {
        self.slots.iter_mut().map(|slot| &mut slot.dex)
    }

    /// Whether the slot's entry must be serialized again rather than copied.
    pub(super) fn is_rewritten(&self, index: DexIndex) -> bool {
        let slot = &self.slots[index.0];
        match slot.origin.kind {
            DexSource::Archive => slot.dex.is_dirty(),
            DexSource::Added => true,
            DexSource::Removed => false,
        }
    }

    pub(super) fn push(&mut self, dex: DexFile, origin: DexOrigin) {
        self.slots.push(Slot { dex, origin });
    }

    /// Puts `dex` in slot `index`, returning the DEX and source it replaces.
    pub(super) fn replace(
        &mut self,
        index: DexIndex,
        dex: DexFile,
        kind: DexSource,
    ) -> (DexFile, DexSource) {
        let slot = &mut self.slots[index.0];
        (
            std::mem::replace(&mut slot.dex, dex),
            std::mem::replace(&mut slot.origin.kind, kind),
        )
    }

    /// Drops the slots from `len` on, returning each DEX with its index.
    pub(super) fn truncate(&mut self, len: usize) -> impl Iterator<Item = (usize, DexFile)> {
        (len..).zip(self.slots.drain(len..).map(|slot| slot.dex))
    }

    /// Serializes one entry holding `members`: a single DEX or part, or the
    /// members of a v041 container in order.
    pub(super) fn serialize(&self, members: &[DexMember]) -> Result<(Spooled, Vec<WriteLayout>)> {
        let written = match members {
            [member] => {
                let (spooled, layout) =
                    reseam_dex::write_spooled(self.dex(member.dex_index), member.part.as_ref())?;
                (spooled, vec![layout])
            }
            _ => reseam_dex::write_container_spooled(
                members
                    .iter()
                    .map(|member| (self.dex(member.dex_index), member.part.as_ref())),
            )?,
        };
        if let Some(member) = members.first() {
            self.dex(member.dex_index).release_pages();
        }
        Ok(written)
    }
}

/// The DEX files of one entry: one, or the members of a v041 container.
pub(super) fn parse_entry(mapped: MappedFile, opts: ParseOptions) -> Result<Vec<DexFile>> {
    let members = reseam_dex::parse_container_with_bytes(Bytes::from_mmap(Arc::new(mapped)), opts)?;
    if let Some(member) = members.first() {
        member.release_pages();
    }
    Ok(members)
}
