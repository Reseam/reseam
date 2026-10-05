// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_apk::Compression;
use reseam_hermes::{
    Argument, Constant, Editor, Edits, FunctionId, FunctionIndex, HermesFile, HermesImage, ModuleId,
};
use reseam_storage::Bytes;

use super::PatchContext;
use crate::error::{PatcherError, Result};

/// The React Native bundle in the base APK.
const BUNDLE: &str = "assets/index.android.bundle";

pub(super) struct HermesSession {
    image: HermesImage,
    index: FunctionIndex,
    edits: Option<Edits>,
    modules: BTreeMap<PathBuf, ModuleId>,
}

impl HermesSession {
    fn edit<T>(&mut self, action: impl FnOnce(&mut Editor<'_>) -> Result<T>) -> Result<T> {
        let file = self.image.file();
        let mut editor = if let Some(edits) = self.edits.take() {
            Editor::resume(file, edits)?
        } else {
            Editor::new(file)
        };
        let outcome = action(&mut editor);
        self.edits = Some(editor.into_edits());
        outcome
    }
}

impl PatchContext<'_> {
    fn hermes_session(&mut self) -> Result<&mut HermesSession> {
        if self.hermes.is_none() {
            let source = self.apk.map_component_entry(0, BUNDLE)?.ok_or_else(|| {
                PatcherError::NotFound(format!("base APK Hermes bundle {BUNDLE}"))
            })?;
            let image = HermesImage::parse(Bytes::from_mmap(Arc::new(source)))?;
            let index = FunctionIndex::build(&image.file())?;
            self.hermes = Some(HermesSession {
                image,
                index,
                edits: None,
                modules: BTreeMap::new(),
            });
        }
        Ok(self.hermes.as_mut().expect("Hermes opened on first use"))
    }

    pub(crate) fn hermes_version(&mut self) -> Result<u32> {
        Ok(self.hermes_session()?.image.file().version())
    }

    pub(crate) fn hermes_find(
        &mut self,
        name: Option<&str>,
        strings: &[&str],
        parameters: Option<u32>,
    ) -> Result<FunctionId> {
        let session = self.hermes_session()?;
        Ok(session
            .index
            .find_function(&session.image.file(), name, strings, parameters)?)
    }

    pub(crate) fn hermes_link(&mut self, module: &Path) -> Result<ModuleId> {
        let session = self.hermes_session()?;
        if let Some(&id) = session.modules.get(module) {
            return Ok(id);
        }
        let file = std::fs::File::open(module).map_err(|e| {
            PatcherError::Bundle(format!("Hermes extension {}: {e}", module.display()))
        })?;
        // SAFETY: bundle payloads are validated immutable snapshots retained for the run.
        let source = unsafe { reseam_storage::map_file(&file)? };
        let module_file = HermesFile::parse(&source)?;
        let id = session.edit(|editor| Ok(editor.link(&module_file)?))?;
        session.modules.insert(module.to_owned(), id);
        Ok(id)
    }

    pub(crate) fn hermes_wrap(
        &mut self,
        function: FunctionId,
        module: ModuleId,
        export: &str,
        bound: &[Argument],
    ) -> Result<()> {
        self.hermes_session()?
            .edit(|editor| Ok(editor.wrap(function, module, export, bound)?))
    }

    pub(crate) fn hermes_always_return(
        &mut self,
        function: FunctionId,
        value: &Constant,
    ) -> Result<()> {
        self.hermes_session()?
            .edit(|editor| Ok(editor.always_return(function, value)?))
    }

    /// Writes the bundle back when a patch changed it.
    pub(crate) fn finish_hermes(&mut self) -> Result<()> {
        let Some(session) = &mut self.hermes else {
            return Ok(());
        };
        if session.edits.as_ref().is_none_or(Edits::is_empty) {
            return Ok(());
        }
        let mut file = reseam_storage::temporary_file()?;
        session.edit(|editor| {
            let mut output = BufWriter::new(&mut file);
            editor.write(&mut output)?;
            output.flush()?;
            Ok(())
        })?;
        self.apk
            .inject_file_spooled(0, BUNDLE, file, Compression::Stored)?;
        Ok(())
    }
}
