// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use super::{ApkComponent, EntryEdit, Resources};
use crate::axml::AxmlDocument;
use crate::entry::EntryName;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Part {
    Manifest,
    Resources,
    Entry(EntryName),
}

pub(super) enum Undo {
    Manifest {
        document: Box<AxmlDocument>,
        revision: u64,
    },
    Resources(Resources),
    Entry(EntryName, Option<EntryEdit>),
}

impl ApkComponent {
    pub(super) fn keep_manifest(&mut self) {
        self.journal.record(Part::Manifest, || Undo::Manifest {
            document: Box::new(self.manifest.clone()),
            revision: self.manifest_revision,
        });
    }

    pub(super) fn keep_resources(&mut self) {
        self.journal
            .record(Part::Resources, || Undo::Resources(self.resources.clone()));
    }

    /// Replaces the edit of entry `name`. A replaced staged file moves into the
    /// journal, so undoing restores it without copying it.
    pub(super) fn set_edit(&mut self, name: EntryName, edit: EntryEdit) {
        let previous = self.edits.insert(name.clone(), edit);
        self.journal
            .record(Part::Entry(name.clone()), || Undo::Entry(name, previous));
    }

    pub(crate) fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
    }

    pub(crate) fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
    }

    pub(crate) fn rollback_to(&mut self, savepoint: Savepoint) {
        for undo in self.journal.rollback(savepoint) {
            match undo {
                Undo::Manifest { document, revision } => {
                    self.manifest = *document;
                    self.manifest_revision = revision;
                }
                Undo::Resources(resources) => self.resources = resources,
                Undo::Entry(name, Some(edit)) => {
                    self.edits.insert(name, edit);
                }
                Undo::Entry(name, None) => {
                    self.edits.remove(&name);
                }
            }
        }
    }
}
