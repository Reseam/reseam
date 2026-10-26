// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use reseam_journal::Savepoint;

use super::PatchContext;
use super::hermes::ModuleSource;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Part {
    Merge(usize),
    Warned(String),
    Module(ModuleSource),
}

pub(super) enum Undo {
    /// Extension `extension` was linked into the app as DEX `dex`.
    Merged {
        extension: usize,
        dex: usize,
    },
    Warned(String),
    Module(ModuleSource),
}

impl PatchContext<'_> {
    /// Starts keeping what each change to the app and the run replaces, so
    /// [`Self::rollback_to`] can return to this point.
    pub fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
        self.apk.open_savepoint(savepoint);
        if let Some(session) = &mut self.hermes {
            session.editor.open_savepoint(savepoint);
        }
    }

    /// Keeps the changes made since `savepoint` as part of the savepoint around it.
    pub fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
        self.apk.release_savepoint(savepoint);
        if let Some(session) = &mut self.hermes {
            session.editor.release_savepoint(savepoint);
        }
    }

    /// Undoes every change made since `savepoint` was opened and closes it,
    /// with every savepoint opened after it. Extensions linked since are
    /// available to link again.
    pub fn rollback_to(&mut self, savepoint: Savepoint) {
        let mut removed: HashMap<usize, _> = self.apk.rollback_to(savepoint).into_iter().collect();
        if let Some(session) = &mut self.hermes {
            session.editor.rollback_to(savepoint);
        }
        for undo in self.journal.rollback(savepoint) {
            match undo {
                Undo::Merged { extension, dex } => self.extensions.restore(
                    extension,
                    removed
                        .remove(&dex)
                        .expect("undoing a link removes the extension's DEX"),
                ),
                Undo::Warned(class) => self.extensions.forget_warning(&class),
                Undo::Module(source) => self
                    .hermes
                    .as_mut()
                    .expect("a linked module belongs to the open session")
                    .forget_module(&source),
            }
        }
        self.skeleton = None;
        self.method = None;
    }
}
