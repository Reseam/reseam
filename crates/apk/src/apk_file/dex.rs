// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{DexFile, Spooled};

use super::dex_set::DexMember;
use super::dex_set::{DexIndex, DexOrigin, DexSet, DexSource};
use super::undo::{Part, Undo};
use super::{ApkFile, ComponentIndex};
use crate::entry::{EntryName, next_free_dex_name};
use crate::error::Result;

impl ApkFile {
    /// DEX slots in load order, including empty tombstones left by deletion.
    pub fn dex(&self) -> &DexSet {
        &self.dex
    }

    /// One DEX without resolving deferred class data, for whole-DEX
    /// operations such as interning or adding classes.
    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.dex.live_mut(index)
    }

    pub fn resolve_dex_class_mut(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&mut DexFile>> {
        let Some(dex) = self
            .dex
            .live_mut(index)
            .filter(|dex| class_idx < dex.classes().len())
        else {
            return Ok(None);
        };
        dex.class_mut(class_idx)?;
        Ok(Some(dex))
    }

    /// Adds a DEX to the base component under the next unused classes name.
    /// Names stay reserved after deletion, and DEX indices remain stable.
    /// Serialization assigns additions after any overflow parts of earlier DEX
    /// files, so their output names can differ from their session entry names.
    pub fn add_dex(&mut self, dex: DexFile) {
        let mut used = self.components[0]
            .reserved_dex_names()
            .map(EntryName::from)
            .collect();
        let name = next_free_dex_name(&mut used);
        self.components[0].add_dex_entry(name.as_str());
        self.push_dex(
            dex,
            DexOrigin {
                component: ComponentIndex(0),
                name,
                kind: DexSource::Added,
            },
        );
    }

    pub fn is_added_dex(&self, index: usize) -> bool {
        index < self.dex.len() && self.dex.origin(DexIndex(index)).kind == DexSource::Added
    }

    /// Puts the members parsed from a replacement of entry `name` into the
    /// slots of the members it replaces, adding or emptying slots when the
    /// member count changed.
    pub(super) fn replace_dex_entry(
        &mut self,
        component: ComponentIndex,
        name: &str,
        members: Vec<DexFile>,
    ) {
        let previous: Vec<_> = self.dex_entries(component, name).collect();
        let count = members.len();
        for (ordinal, dex) in members.into_iter().enumerate() {
            match previous.get(ordinal) {
                Some(&index) => self.replace_dex(index, dex, DexSource::Archive),
                None => self.push_dex(
                    dex,
                    DexOrigin {
                        component,
                        name: name.into(),
                        kind: DexSource::Archive,
                    },
                ),
            }
        }
        for index in previous.into_iter().skip(count) {
            self.remove_dex_slot(index);
        }
    }

    pub(super) fn remove_dex_entry(&mut self, component: ComponentIndex, name: &str) {
        let indices: Vec<_> = self.dex_entries(component, name).collect();
        for index in indices {
            self.remove_dex_slot(index);
        }
    }

    /// The serialized entry `name` when any DEX parsed from it changed.
    /// Edited v041 containers keep every member in physical-entry order.
    pub(super) fn rewritten_dex_entry(
        &self,
        component: ComponentIndex,
        name: &str,
    ) -> Result<Option<Spooled>> {
        let members: Vec<_> = self
            .dex_entries(component, name)
            .filter(|&index| self.dex.origin(index).kind != DexSource::Removed)
            .map(|dex_index| DexMember {
                dex_index,
                part: None,
            })
            .collect();
        if !members
            .iter()
            .any(|member| self.dex.is_rewritten(member.dex_index))
        {
            return Ok(None);
        }
        Ok(Some(self.dex.serialize(&members)?.0))
    }

    fn dex_entries<'a>(
        &'a self,
        component: ComponentIndex,
        name: &'a str,
    ) -> impl Iterator<Item = DexIndex> + 'a {
        self.dex
            .slots()
            .filter(move |(_, _, origin)| {
                origin.component == component && origin.name.as_str() == name
            })
            .map(|(index, _, _)| index)
    }

    fn remove_dex_slot(&mut self, index: DexIndex) {
        let header = self.dex.dex(index).header().clone();
        self.replace_dex(index, DexFile::new(header), DexSource::Removed);
    }

    /// Adds `dex` under every open savepoint, so undoing any of them returns it as it is now.
    fn push_dex(&mut self, mut dex: DexFile, origin: DexOrigin) {
        let len = self.dex.len();
        self.journal.record(Part::DexCount, || Undo::DexCount(len));
        self.journal
            .savepoints()
            .for_each(|savepoint| dex.open_savepoint(savepoint));
        self.dex.push(dex, origin);
    }

    /// Puts `dex` in slot `index`; the DEX it replaces moves into the journal.
    fn replace_dex(&mut self, index: DexIndex, mut dex: DexFile, kind: DexSource) {
        self.journal
            .savepoints()
            .for_each(|savepoint| dex.open_savepoint(savepoint));
        let (previous, previous_kind) = self.dex.replace(index, dex, kind);
        self.journal.record(Part::Slot(index), || Undo::Slot {
            index,
            dex: Box::new(previous),
            kind: previous_kind,
        });
    }
}
