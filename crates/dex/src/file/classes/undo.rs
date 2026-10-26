// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use super::{ClassSlot, ClassTable};
use crate::model::class::ClassDef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ClassKey {
    Len,
    Slot(u32),
}

#[derive(Debug, Clone)]
pub(super) enum ClassUndo {
    Truncate(usize),
    Restore(u32, Prior),
    Insert(u32, Box<ClassDef>),
}

/// A slot as it was before a change: still a file record, or the class then.
#[derive(Debug, Clone)]
pub(super) enum Prior {
    Raw(u32),
    Resident(Box<ClassDef>),
}

/// What a rollback changed in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Undone {
    Nothing,
    /// Classes went back to earlier contents; every class kept its position.
    Contents,
    /// Classes were added or removed, so positions moved.
    Positions,
}

impl ClassTable {
    pub(crate) fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
    }

    pub(crate) fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
    }

    pub(crate) fn rollback_to(&mut self, savepoint: Savepoint) -> Undone {
        let undos = self.journal.rollback(savepoint);
        if undos.is_empty() {
            return Undone::Nothing;
        }
        let mut undone = Undone::Contents;
        for undo in undos {
            match undo {
                ClassUndo::Truncate(len) => {
                    self.slots.truncate(len);
                    undone = Undone::Positions;
                }
                ClassUndo::Restore(i, Prior::Raw(r)) => self.slots[i as usize] = ClassSlot::Raw(r),
                ClassUndo::Restore(i, Prior::Resident(def)) => {
                    self.slots[i as usize] = ClassSlot::resident(*def);
                }
                ClassUndo::Insert(i, def) => {
                    self.slots.insert(i as usize, ClassSlot::resident(*def));
                    undone = Undone::Positions;
                }
            }
        }
        self.resident = (0..self.slots.len() as u32)
            .filter(|&i| matches!(self.slots[i as usize], ClassSlot::Resident(_)))
            .collect();
        self.by_type.take();
        undone
    }
}
