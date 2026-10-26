// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::closure::RootAttachment;
use crate::error::{HermesError, Result};
use crate::layout::{Field, Section};

use crate::{FunctionId, HermesFile};

mod function;
mod module;
mod strings;
mod undo;

pub(crate) use function::{EditedFunction, FunctionBody, generated_function, original_function};
pub use module::ModuleId;
pub(crate) use module::{LinkNames, LinkedImport, LinkedModule};
pub(crate) use strings::AddedStrings;
use undo::{Part, Undo};

/// Changes to one file, kept apart from it. Original function and string identities stay
/// stable; only additions and changed bodies allocate. Every change can be rolled back.
pub(crate) struct Edits {
    strings: AddedStrings,
    /// What is added to the sections strings do not own.
    additions: [Vec<u8>; 15],
    /// Replaced app function bodies.
    functions: BTreeMap<FunctionId, EditedFunction>,
    appended: Vec<EditedFunction>,
    /// The appended function that initializes linked modules and then runs the app's global code.
    bootstrap: Option<FunctionId>,
    modules: Vec<LinkedModule>,
    link_names: Option<LinkNames>,
    string_switches: u32,
    roots: BTreeMap<FunctionId, RootAttachment>,
    /// Hooked app functions, each with the appended function now holding its original body.
    relocated: BTreeMap<FunctionId, FunctionId>,
    /// App functions replaced before any hook kept their original body.
    discarded: BTreeSet<FunctionId>,
    journal: reseam_journal::Journal<Part, Undo>,
}

impl Edits {
    pub(crate) fn new(file: &HermesFile<'_>) -> Self {
        Self {
            strings: AddedStrings::new(file),
            additions: std::array::from_fn(|_| Vec::new()),
            functions: BTreeMap::new(),
            appended: Vec::new(),
            bootstrap: None,
            modules: Vec::new(),
            link_names: None,
            string_switches: file.field(Field::StringSwitchCount),
            roots: BTreeMap::new(),
            relocated: BTreeMap::new(),
            discarded: BTreeSet::new(),
            journal: reseam_journal::Journal::default(),
        }
    }

    /// Whether writing would reproduce the original file.
    pub(crate) fn is_empty(&self) -> bool {
        self.strings.is_empty() && self.appended.is_empty() && self.functions.is_empty()
    }

    // Fallible work may append data. Replacements and root attachments are published only after
    // it succeeds, so undoing the appends undoes a failed operation.
    pub(crate) fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let marks = self.marks();
        self.journal.record(Part::Marks, || Undo::Marks(marks));
        let result = operation(self);
        if result.is_err() {
            self.restore(marks);
        }
        result
    }

    pub(crate) fn section_size(&self, file: &HermesFile<'_>, section: Section) -> usize {
        file.section(section).len() + self.added(section).len()
    }

    pub(crate) fn section_count(&self, file: &HermesFile<'_>, section: Section) -> u32 {
        (self.section_size(file, section) / section.stride()) as u32
    }

    /// The bytes added to the end of `section`.
    pub(crate) fn added(&self, section: Section) -> &[u8] {
        self.strings
            .section(section)
            .unwrap_or(&self.additions[section as usize])
    }

    pub(crate) fn strings(&self) -> &AddedStrings {
        &self.strings
    }

    pub(crate) fn strings_mut(&mut self) -> &mut AddedStrings {
        &mut self.strings
    }

    /// Adds `bytes` to the end of a literal `section`.
    pub(crate) fn append(&mut self, section: Section, bytes: &[u8]) {
        self.additions[section as usize].extend_from_slice(bytes);
    }

    /// How many functions the file has with the appended ones: the ID the next one gets.
    pub(crate) fn function_count(&self, file: &HermesFile<'_>) -> u32 {
        file.function_count() + self.appended.len() as u32
    }

    pub(crate) fn append_function(
        &mut self,
        file: &HermesFile<'_>,
        function: EditedFunction,
    ) -> FunctionId {
        let id = FunctionId(self.function_count(file));
        self.appended.push(function);
        id
    }

    /// The function `id` as edited: an app function's replaced body, or an appended function.
    pub(crate) fn function(
        &self,
        file: &HermesFile<'_>,
        id: FunctionId,
    ) -> Option<&EditedFunction> {
        match id.0.checked_sub(file.function_count()) {
            Some(appended) => self.appended.get(appended as usize),
            None => self.functions.get(&id),
        }
    }

    /// Whether `id`'s original body was replaced without being kept.
    pub(crate) fn is_discarded(&self, id: FunctionId) -> bool {
        self.discarded.contains(&id)
    }

    pub(crate) fn roots(&self) -> &BTreeMap<FunctionId, RootAttachment> {
        &self.roots
    }

    /// The appended function holding `id`'s original body, once a hook keeps it.
    pub(crate) fn relocated(&self, id: FunctionId) -> Option<FunctionId> {
        self.relocated.get(&id).copied()
    }

    /// The function the file runs first.
    pub(crate) fn global(&self, file: &HermesFile<'_>) -> FunctionId {
        self.bootstrap.unwrap_or_else(|| file.global_function())
    }

    pub(crate) fn bootstrap(&self) -> Option<FunctionId> {
        self.bootstrap
    }

    pub(crate) fn set_bootstrap(&mut self, id: FunctionId) {
        self.bootstrap = Some(id);
    }

    pub(crate) fn string_switches(&self) -> u32 {
        self.string_switches
    }

    pub(crate) fn add_string_switches(&mut self, count: u32) -> Result<()> {
        self.string_switches = self
            .string_switches
            .checked_add(count)
            .ok_or(HermesError::Limit("four billion string switches"))?;
        Ok(())
    }

    pub(crate) fn modules(&self) -> &[LinkedModule] {
        &self.modules
    }

    pub(crate) fn link_names(&self) -> Option<LinkNames> {
        self.link_names
    }

    pub(crate) fn add_module(&mut self, module: LinkedModule, names: LinkNames) -> ModuleId {
        self.modules.push(module);
        self.link_names = Some(names);
        ModuleId(self.modules.len() as u32 - 1)
    }
}
