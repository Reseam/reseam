// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Result, invalid};
use crate::layout::{FOOTER_SIZE, Field, Section};
use crate::string::{StringId, StringKind};
use crate::{FunctionId, HermesFile};

mod function;
mod strings;
mod undo;

pub(crate) use function::{EditedFunction, FunctionBody, generated_function, original_function};
pub(crate) use strings::AddedString;
use undo::{Part, Undo};

/// Owns edits over a borrowed file. Original function and string identities
/// remain stable. Only additions, changed bodies and indices allocate storage.
/// Indices and closure analysis are built lazily and retained in detached edits.
pub struct Editor<'a> {
    pub(crate) file: HermesFile<'a>,
    pub(crate) edits: Edits,
}

/// Detached edits for a long-lived owner of the input mapping. Resume them only
/// against the exact original file. No source data is copied into this state.
pub struct Edits {
    pub(crate) base_hash: [u8; FOOTER_SIZE],
    pub(crate) additions: [Vec<u8>; 15],
    pub(crate) functions: BTreeMap<FunctionId, EditedFunction>,
    pub(crate) appended: Vec<EditedFunction>,
    pub(crate) strings: Vec<AddedString>,
    pub(crate) string_index: rustc_hash::FxHashMap<(u64, StringKind), Vec<StringId>>,
    original_strings_indexed: bool,
    pub(crate) global: FunctionId,
    pub(crate) modules: Vec<crate::link::LinkedModule>,
    pub(crate) link_names: Option<crate::link::LinkNames>,
    pub(crate) string_switches: u32,
    pub(crate) hook_graph: Option<crate::closure::HookGraph>,
    pub(crate) metro: Option<crate::metro::MetroModules>,
    pub(crate) roots: BTreeMap<FunctionId, crate::closure::RootAttachment>,
    /// Hooked app functions, each with the appended function now holding its original body.
    pub(crate) relocated: BTreeMap<FunctionId, FunctionId>,
    /// App functions replaced before any hook kept their original body.
    pub(crate) discarded: BTreeSet<FunctionId>,
    journal: reseam_journal::Journal<Part, Undo>,
}

impl Edits {
    /// Whether writing would reproduce the original file.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty() && self.appended.is_empty() && self.functions.is_empty()
    }
}

impl<'a> Editor<'a> {
    pub fn new(file: HermesFile<'a>) -> Self {
        let global = file.global_function();
        let string_switches = file.field(Field::StringSwitchCount);
        let base_hash = file.footer();
        Self {
            file,
            edits: Edits {
                base_hash,
                additions: std::array::from_fn(|_| Vec::new()),
                functions: BTreeMap::new(),
                appended: Vec::new(),
                strings: Vec::new(),
                string_index: rustc_hash::FxHashMap::default(),
                original_strings_indexed: false,
                global,
                modules: Vec::new(),
                link_names: None,
                string_switches,
                hook_graph: None,
                metro: None,
                roots: BTreeMap::new(),
                relocated: BTreeMap::new(),
                discarded: BTreeSet::new(),
                journal: reseam_journal::Journal::default(),
            },
        }
    }

    /// Detaches owned edits so a caller can retain them beside its mapping
    /// without building a self-referential object. Use `resume` to continue.
    pub fn into_edits(self) -> Edits {
        self.edits
    }

    /// Restores detached state against the original file. A different footer
    /// is an error; callers must keep the underlying mapping unchanged.
    pub fn resume(file: HermesFile<'a>, edits: Edits) -> Result<Self> {
        if file.footer() != edits.base_hash {
            return Err(invalid(0, "edits belong to another source file"));
        }
        Ok(Self { file, edits })
    }

    // Fallible work may append data and populate immutable analysis caches.
    // Replacements and root attachments are published only after it succeeds.
    pub(crate) fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let marks = self.edits.marks();
        self.edits
            .journal
            .record(Part::Marks, || Undo::Marks(marks));
        let result = operation(self);
        if result.is_err() {
            self.edits.restore(marks);
        }
        result
    }

    pub(crate) fn section_size(&self, section: Section) -> usize {
        self.file.section(section).len() + self.edits.additions[section as usize].len()
    }

    pub(crate) fn section_count(&self, section: Section) -> u32 {
        (self.section_size(section) / section.stride()) as u32
    }

    pub(crate) fn next_function(&self) -> FunctionId {
        FunctionId(self.file.function_count() + self.edits.appended.len() as u32)
    }

    pub(crate) fn append_function(&mut self, function: EditedFunction) -> FunctionId {
        let id = self.next_function();
        self.edits.appended.push(function);
        id
    }
}
