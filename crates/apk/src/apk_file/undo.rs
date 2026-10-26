// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::DexFile;
use reseam_journal::Savepoint;

use super::ApkFile;
use super::dex_set::{DexIndex, DexSource};

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Part {
    DexCount,
    Slot(DexIndex),
}

pub(super) enum Undo {
    DexCount(usize),
    Slot {
        index: DexIndex,
        dex: Box<DexFile>,
        kind: DexSource,
    },
}

impl ApkFile {
    /// Starts keeping what each change replaces, across the components and
    /// every DEX, so [`Self::rollback_to`] can return to this point.
    pub fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
        self.dex
            .files_mut()
            .for_each(|dex| dex.open_savepoint(savepoint));
        self.components
            .iter_mut()
            .for_each(|component| component.open_savepoint(savepoint));
    }

    /// Keeps the changes made since `savepoint` as part of the savepoint around it.
    pub fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
        self.dex
            .files_mut()
            .for_each(|dex| dex.release_savepoint(savepoint));
        self.components
            .iter_mut()
            .for_each(|component| component.release_savepoint(savepoint));
    }

    /// Undoes every change made since `savepoint` was opened and closes it,
    /// with every savepoint opened after it. Returns the DEX files added since,
    /// by the index they had, each as it was when added.
    pub fn rollback_to(&mut self, savepoint: Savepoint) -> Vec<(usize, DexFile)> {
        let mut removed = Vec::new();
        for undo in self.journal.rollback(savepoint) {
            match undo {
                Undo::DexCount(len) => removed.extend(self.dex.truncate(len)),
                Undo::Slot { index, dex, kind } => {
                    self.dex.replace(index, *dex, kind);
                }
            }
        }
        self.dex
            .files_mut()
            .for_each(|dex| dex.rollback_to(savepoint));
        // A removed DEX joined every savepoint open when it was added, older
        // ones too, but each change it holds came after: all of them go.
        for (_, dex) in &mut removed {
            dex.rollback_to(Savepoint::FIRST);
        }
        self.components
            .iter_mut()
            .for_each(|component| component.rollback_to(savepoint));
        removed.sort_by_key(|&(index, _)| index);
        removed
    }
}
