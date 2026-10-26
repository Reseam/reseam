// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use super::DexFile;
use super::classes::Undone;
use crate::options::WriteOptions;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum DexPart {
    Pools,
    WriteOptions,
}

#[derive(Debug, Clone)]
pub(super) enum DexUndo {
    Pools(PoolLens),
    WriteOptions(WriteOptions),
}

/// The append-only pools' lengths, and whether the DEX had changed, at one point.
#[derive(Debug, Clone, Copy)]
pub(super) struct PoolLens {
    strings: usize,
    types: usize,
    prototypes: usize,
    fields: usize,
    methods: usize,
    call_sites: usize,
    method_handles: usize,
    dirty: bool,
}

impl DexFile {
    pub(super) fn pool_lens(&self) -> PoolLens {
        PoolLens {
            strings: self.strings.len(),
            types: self.types.len(),
            prototypes: self.prototypes.len(),
            fields: self.fields.len(),
            methods: self.methods.len(),
            call_sites: self.call_sites.len(),
            method_handles: self.method_handles.len(),
            dirty: self.dirty,
        }
    }

    /// Starts keeping what each change replaces, so [`Self::rollback_to`] can
    /// return to this point.
    pub fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
        self.classes.open_savepoint(savepoint);
    }

    /// Keeps the changes made since `savepoint` as part of the savepoint around it.
    pub fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
        self.classes.release_savepoint(savepoint);
    }

    /// Undoes every change made since `savepoint` was opened and closes it,
    /// with every savepoint opened after it.
    pub fn rollback_to(&mut self, savepoint: Savepoint) {
        if self.classes.rollback_to(savepoint) == Undone::Positions {
            self.ref_index.take();
        }
        for undo in self.journal.rollback(savepoint) {
            match undo {
                DexUndo::Pools(lens) => {
                    self.strings.truncate(lens.strings);
                    self.types.truncate(lens.types);
                    self.prototypes.truncate(lens.prototypes);
                    self.fields.truncate(lens.fields);
                    self.methods.truncate(lens.methods);
                    self.call_sites.truncate(lens.call_sites);
                    self.method_handles.truncate(lens.method_handles);
                    self.dirty = lens.dirty;
                    self.type_lookups.clear();
                }
                DexUndo::WriteOptions(options) => self.write_options = options,
            }
        }
    }
}
