// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Write;

use reseam_journal::Savepoint;
use reseam_storage::Bytes;

use crate::closure::Closures;
use crate::code::{Code, HookKind};
use crate::edit::{Edits, ModuleId};
use crate::error::Result;
use crate::hook::Hooker;
use crate::link::{Import, Linker};
use crate::metro::MetroModules;
use crate::{FunctionId, FunctionIndex, HermesFile, HermesImage};

/// Edits an app's Hermes file: links modules into it and hooks its functions. The file stays in
/// its source storage, which must not change while the editor lives; original function and
/// string identities stay stable.
pub struct Editor {
    image: HermesImage,
    index: FunctionIndex,
    analysis: Analysis,
    edits: Edits,
}

/// What hooks and links learn about the original file, kept across them.
#[derive(Default)]
struct Analysis {
    closures: Closures,
    metro: MetroModules,
}

impl Editor {
    /// Validates `source` and indexes its functions, decoding each once.
    pub fn new(source: Bytes) -> Result<Self> {
        let image = HermesImage::parse(source)?;
        let file = image.file();
        let index = FunctionIndex::build(&file)?;
        let edits = Edits::new(&file);
        Ok(Self {
            image,
            index,
            analysis: Analysis::default(),
            edits,
        })
    }

    pub fn version(&self) -> u32 {
        self.image.file().version()
    }

    /// Returns exactly one original app function matching every constraint. Matching is exact
    /// and case-sensitive; parameter counts exclude `this`. Zero or several matches fail with
    /// the query and count.
    pub fn find_function(
        &self,
        name: Option<&str>,
        strings: &[&str],
        parameters: Option<u32>,
    ) -> Result<FunctionId> {
        self.index
            .find_function(&self.image.file(), name, strings, parameters)
    }

    /// Links a compiled JavaScript module and returns its identity. The module evaluates to
    /// `{ imports, link }` and keeps its declarations in its own scope, normally an IIFE; see
    /// [`imports`](crate::imports). `imports` resolves each declared specifier exactly once.
    /// Before the app's global code runs, `link` is called with an object mapping each specifier
    /// to its Metro module, and returns the exports object; the exports are the keys every
    /// return path of `link` declares. `CommonJS` segments, global declarations and static
    /// builtins the app does not assume fail. Module debug information is dropped.
    pub fn link(&mut self, module: &HermesFile<'_>, imports: &[Import]) -> Result<ModuleId> {
        let file = self.image.file();
        let (index, metro) = (&self.index, &mut self.analysis.metro);
        self.edits.transaction(|edits| {
            Linker {
                file,
                index,
                metro,
                edits,
            }
            .link(module, imports)
        })
    }

    /// Adds `code` to `function` as a new layer; callers are untouched. [`HookKind::Before`] and
    /// [`HookKind::After`] run around the function as it was, which later layers then run around
    /// in turn; [`HookKind::Replace`] runs instead of it, and earlier layers no longer run. The
    /// receiver and the linked modules' exports keep the function's original captured
    /// environment.
    ///
    /// Generators, async functions, constructors, functions using `new.target` or direct
    /// `eval`, unknown exports, malformed code and unprovable environment chains fail. A replaced
    /// function whose closures an earlier hook needs fails too. Hooked functions cannot be used
    /// as constructors afterwards.
    pub fn hook(&mut self, function: FunctionId, kind: HookKind, code: &Code) -> Result<()> {
        let file = self.image.file();
        let (index, closures) = (&self.index, &mut self.analysis.closures);
        self.edits.transaction(|edits| {
            Hooker {
                file,
                index,
                closures,
                edits,
            }
            .hook(function, kind, code)
        })
    }

    /// Whether writing would change the file.
    pub fn is_edited(&self) -> bool {
        !self.edits.is_empty()
    }

    /// Streams the edited file. Untouched bodies and sections are copied from the source;
    /// function headers and debug information are relocated and the SHA-1 footer recomputed.
    /// Without edits the source is written unchanged. `output` is flushed.
    pub fn write(&self, mut output: impl Write) -> Result<()> {
        crate::write::write(&self.image.file(), &self.edits, &mut output)?;
        output.flush()?;
        Ok(())
    }

    /// Starts keeping what each edit replaces, so [`Self::rollback_to`] can return to this point.
    pub fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.edits.open_savepoint(savepoint);
    }

    /// Keeps the edits made since `savepoint` as part of the savepoint around it.
    pub fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.edits.release_savepoint(savepoint);
    }

    /// Undoes every edit made since `savepoint` was opened and closes it, with every savepoint
    /// opened after it.
    pub fn rollback_to(&mut self, savepoint: Savepoint) {
        self.edits.rollback_to(savepoint);
    }
}
