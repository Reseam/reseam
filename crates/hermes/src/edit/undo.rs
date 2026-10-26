// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EditedFunction, Edits, LinkNames, strings};
use crate::FunctionId;
use crate::closure::RootAttachment;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Part {
    Marks,
    Function(FunctionId),
    Root(FunctionId),
    Relocated(FunctionId),
    Discarded(FunctionId),
}

pub(super) enum Undo {
    Marks(Marks),
    Function(FunctionId, Option<EditedFunction>),
    Root(FunctionId, Option<RootAttachment>),
    Relocated(FunctionId, Option<FunctionId>),
    Discarded(FunctionId, bool),
}

/// Where the append-only edits ended, and the values edits change, at one point.
#[derive(Clone, Copy)]
pub(super) struct Marks {
    additions: [usize; 15],
    appended: usize,
    strings: strings::Marks,
    modules: usize,
    bootstrap: Option<FunctionId>,
    string_switches: u32,
    link_names: Option<LinkNames>,
}

impl Edits {
    /// Starts keeping what each edit replaces, so [`Self::rollback_to`] can
    /// return to this point.
    pub(crate) fn open_savepoint(&mut self, savepoint: reseam_journal::Savepoint) {
        self.journal.open(savepoint);
    }

    /// Keeps the edits made since `savepoint` as part of the savepoint around it.
    pub(crate) fn release_savepoint(&mut self, savepoint: reseam_journal::Savepoint) {
        self.journal.release(savepoint);
    }

    /// Undoes every edit made since `savepoint` was opened and closes it, with
    /// every savepoint opened after it.
    pub(crate) fn rollback_to(&mut self, savepoint: reseam_journal::Savepoint) {
        for undo in self.journal.rollback(savepoint) {
            match undo {
                Undo::Marks(marks) => self.restore(marks),
                Undo::Function(id, Some(body)) => {
                    self.functions.insert(id, body);
                }
                Undo::Function(id, None) => {
                    self.functions.remove(&id);
                }
                Undo::Root(id, Some(root)) => {
                    self.roots.insert(id, root);
                }
                Undo::Root(id, None) => {
                    self.roots.remove(&id);
                }
                Undo::Relocated(id, Some(original)) => {
                    self.relocated.insert(id, original);
                }
                Undo::Relocated(id, None) => {
                    self.relocated.remove(&id);
                }
                Undo::Discarded(id, true) => {
                    self.discarded.insert(id);
                }
                Undo::Discarded(id, false) => {
                    self.discarded.remove(&id);
                }
            }
        }
    }

    pub(super) fn marks(&self) -> Marks {
        Marks {
            additions: self.additions.each_ref().map(Vec::len),
            appended: self.appended.len(),
            strings: self.strings.marks(),
            modules: self.modules.len(),
            bootstrap: self.bootstrap,
            string_switches: self.string_switches,
            link_names: self.link_names,
        }
    }

    pub(super) fn restore(&mut self, marks: Marks) {
        for (bytes, length) in self.additions.iter_mut().zip(marks.additions) {
            bytes.truncate(length);
        }
        self.appended.truncate(marks.appended);
        self.strings.truncate(marks.strings);
        self.modules.truncate(marks.modules);
        self.bootstrap = marks.bootstrap;
        self.string_switches = marks.string_switches;
        self.link_names = marks.link_names;
    }

    /// Gives app function `id` a new body.
    pub(crate) fn replace(&mut self, id: FunctionId, body: EditedFunction) {
        let previous = self.functions.insert(id, body);
        self.journal
            .record(Part::Function(id), || Undo::Function(id, previous));
    }

    pub(crate) fn relocate(&mut self, id: FunctionId, original: FunctionId) {
        let previous = self.relocated.insert(id, original);
        self.journal
            .record(Part::Relocated(id), || Undo::Relocated(id, previous));
    }

    pub(crate) fn discard(&mut self, id: FunctionId) {
        let discarded = !self.discarded.insert(id);
        self.journal
            .record(Part::Discarded(id), || Undo::Discarded(id, discarded));
    }

    pub(crate) fn attach_root(&mut self, id: FunctionId, root: RootAttachment) {
        self.journal.record(Part::Root(id), || {
            Undo::Root(id, self.roots.get(&id).cloned())
        });
        self.roots
            .entry(id)
            .and_modify(|existing| existing.sites.extend(&root.sites))
            .or_insert(root);
    }
}
