// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::BufWriter;
use std::sync::Arc;

use reseam_apk::{Compression, NativeLibrary};
use reseam_elf::{Branch, Editor, ElfError, Function, Pattern, Query};
use reseam_model::Abi;
use reseam_storage::Bytes;
use tracing::debug;

use super::PatchContext;
use crate::error::{PatcherError, Result};

/// The ABIs whose native code can be edited.
const EDITABLE: [Abi; 2] = [Abi::Arm64V8a, Abi::ArmeabiV7a];

/// The `lib/<abi>/<name>` entry in one component.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LibraryKey {
    pub(crate) component: usize,
    pub(crate) entry: String,
    pub(crate) abi: Abi,
}

impl From<NativeLibrary> for LibraryKey {
    fn from(copy: NativeLibrary) -> Self {
        Self {
            component: copy.component,
            entry: copy.entry,
            abi: copy.abi,
        }
    }
}

impl LibraryKey {
    fn error(&self, source: ElfError) -> PatcherError {
        PatcherError::Elf {
            entry: self.entry.clone(),
            source: Box::new(source),
        }
    }
}

/// A function lookup in the copies of a native library, as a patch states it.
#[derive(Default)]
pub(crate) struct NativeQuery<'a> {
    pub(crate) library: &'a str,
    /// Narrows the lookup to the copy for this ABI.
    pub(crate) abi: Option<Abi>,
    pub(crate) symbol: Option<&'a str>,
    pub(crate) strings: &'a [&'a str],
    /// Byte patterns, each constraining only the copy for its ABI, whose code it is.
    pub(crate) patterns: &'a [(Abi, Pattern)],
    pub(crate) nth: Option<usize>,
}

fn patterns_for(patterns: &[(Abi, Pattern)], abi: Abi) -> impl Iterator<Item = &Pattern> {
    patterns
        .iter()
        .filter(move |(pattern_abi, _)| *pattern_abi == abi)
        .map(|(_, pattern)| pattern)
}

impl PatchContext<'_> {
    /// The function matching `query` in each copy of its library the run edits: the copy for its
    /// ABI, or every copy of an editable ABI the device loads. See [`Editor::find_function`].
    /// Only arm64-v8a and armeabi-v7a code is editable: a run for a device that loads another ABI
    /// fails, and a run for any device warns about the copies of other ABIs it leaves unedited.
    /// The library must have a copy the run can edit; a query narrowed to an ABI without one
    /// matches nothing.
    pub(crate) fn native_find(
        &mut self,
        query: &NativeQuery<'_>,
    ) -> Result<Vec<(LibraryKey, Function)>> {
        let device_abi = self.apk().native_abi();
        if let Some(device_abi) = device_abi.filter(|device_abi| !EDITABLE.contains(device_abi)) {
            return Err(PatcherError::NativeAbi(device_abi));
        }
        let loads = |copy_abi: Abi| device_abi.is_none_or(|device_abi| device_abi == copy_abi);
        let (editable, others): (Vec<NativeLibrary>, Vec<NativeLibrary>) = self
            .apk()
            .native_library_copies(query.library)
            .into_iter()
            .filter(|copy| loads(copy.abi))
            .partition(|copy| EDITABLE.contains(&copy.abi));
        if editable.is_empty() {
            return Err(PatcherError::MissingNativeLibrary {
                library: query.library.to_owned(),
                abis: EDITABLE.into_iter().filter(|&abi| loads(abi)).collect(),
            });
        }
        for copy in others {
            self.scope().log.warn(format!(
                "{} stays unedited: {} native code cannot be edited",
                copy.entry,
                copy.abi.as_str()
            ));
        }
        editable
            .into_iter()
            .filter(|copy| query.abi.is_none_or(|abi| abi == copy.abi))
            .map(|copy| {
                let key = LibraryKey::from(copy);
                let patterns: Vec<Pattern> =
                    patterns_for(query.patterns, key.abi).cloned().collect();
                let function = self
                    .native_editor(&key)?
                    .find_function(&Query {
                        symbol: query.symbol,
                        strings: query.strings,
                        patterns: &patterns,
                        nth: query.nth,
                    })
                    .map_err(|source| key.error(source))?;
                debug!(
                    entry = %key.entry,
                    address = %format_args!("{:#x}", function.address),
                    "native function found"
                );
                Ok((key, function))
            })
            .collect()
    }

    fn native_editor(&mut self, key: &LibraryKey) -> Result<&mut Editor> {
        if !self.native.contains_key(key) {
            let source = self
                .apk
                .map_component_entry(key.component, &key.entry)?
                .ok_or_else(|| PatcherError::NotFound(key.entry.clone()))?;
            let mut editor = Editor::new(Bytes::from_mmap(Arc::new(source)))
                .map_err(|source| key.error(source))?;
            self.journal
                .savepoints()
                .for_each(|savepoint| editor.open_savepoint(savepoint));
            self.native.insert(key.clone(), editor);
        }
        Ok(self
            .native
            .get_mut(key)
            .expect("library opened on first use"))
    }

    /// The conditional branch in each of `functions` that the pattern for its copy's ABI covers.
    /// See [`Editor::find_branch`].
    pub(crate) fn native_find_branch(
        &mut self,
        functions: &[(LibraryKey, Function)],
        patterns: &[(Abi, Pattern)],
    ) -> Result<Vec<(LibraryKey, Branch)>> {
        functions
            .iter()
            .map(|(key, function)| {
                let pattern = patterns_for(patterns, key.abi).next().ok_or_else(|| {
                    PatcherError::MissingBranchPattern {
                        entry: key.entry.clone(),
                        abi: key.abi,
                    }
                })?;
                let branch = self
                    .native_editor(key)?
                    .find_branch(*function, pattern)
                    .map_err(|source| key.error(source))?;
                debug!(
                    entry = %key.entry,
                    address = %format_args!("{:#x}", branch.address),
                    "native branch found"
                );
                Ok((key.clone(), branch))
            })
            .collect()
    }

    /// Makes each of `branches` jump whatever its condition.
    pub(crate) fn native_always_take(&mut self, branches: &[(LibraryKey, Branch)]) -> Result<()> {
        self.edit_each(branches, Editor::always_take)
    }

    /// Makes each of `branches` fall through whatever its condition.
    pub(crate) fn native_never_take(&mut self, branches: &[(LibraryKey, Branch)]) -> Result<()> {
        self.edit_each(branches, Editor::never_take)
    }

    /// Replaces each of `functions` with a return of `value`.
    pub(crate) fn native_always_return(
        &mut self,
        functions: &[(LibraryKey, Function)],
        value: i32,
    ) -> Result<()> {
        self.edit_each(functions, |editor, function| {
            editor.always_return(function, value)
        })
    }

    /// Writes `bytes` at `offset` into each of `functions`. Raw bytes are code of one instruction
    /// set, so the functions must share theirs.
    pub(crate) fn native_replace_bytes(
        &mut self,
        functions: &[(LibraryKey, Function)],
        offset: u64,
        bytes: &[u8],
    ) -> Result<()> {
        if let [(first, function), rest @ ..] = functions
            && let Some((second, _)) = rest
                .iter()
                .find(|(_, other)| other.instruction_set != function.instruction_set)
        {
            return Err(PatcherError::MixedInstructionSets {
                first: first.entry.clone(),
                second: second.entry.clone(),
            });
        }
        self.edit_each(functions, |editor, function| {
            editor.replace_bytes(function, offset, bytes)
        })
    }

    fn edit_each<T: Copy>(
        &mut self,
        targets: &[(LibraryKey, T)],
        edit: impl Fn(&mut Editor, T) -> reseam_elf::Result<()>,
    ) -> Result<()> {
        for (key, target) in targets {
            edit(self.native_editor(key)?, *target).map_err(|source| key.error(source))?;
        }
        Ok(())
    }

    /// How many native code edits the run holds. Every edit raises it.
    pub(crate) fn native_revision(&self) -> usize {
        self.native.values().map(Editor::edits).sum()
    }

    /// Forgets the editor of `entry` before a patch replaces or deletes the file. A library
    /// holding edits fails instead, since writing it back would undo the replacement.
    pub(super) fn release_native_library(&mut self, component: usize, entry: &str) -> Result<()> {
        let Some(key) = self
            .native
            .keys()
            .find(|key| key.component == component && key.entry == entry)
            .cloned()
        else {
            return Ok(());
        };
        if self.native[&key].edits() > 0 {
            return Err(PatcherError::EditedNativeLibrary(key.entry));
        }
        self.native.remove(&key);
        Ok(())
    }

    /// Writes back every native library a patch changed.
    pub(crate) fn finish_native(&mut self) -> Result<()> {
        let changed: Vec<LibraryKey> = self
            .native
            .iter()
            .filter(|(_, editor)| editor.edits() > 0)
            .map(|(key, _)| key.clone())
            .collect();
        for key in changed {
            let mut file = reseam_storage::temporary_file()?;
            self.native[&key]
                .write(BufWriter::new(&mut file))
                .map_err(|source| key.error(source))?;
            self.apk
                .inject_file_spooled(key.component, &key.entry, file, Compression::Stored)?;
        }
        Ok(())
    }
}
