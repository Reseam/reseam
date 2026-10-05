use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use reseam_apk::Compression;
use reseam_hermes::{Editor, Edits, FunctionId, HermesFile, ModuleId};
use reseam_storage::MappedFile;

use super::PatchContext;
use crate::error::{PatcherError, Result};

pub(super) struct HermesSession {
    path: String,
    source: MappedFile,
    edits: Option<Edits>,
    modules: BTreeMap<PathBuf, ModuleId>,
}

impl HermesSession {
    fn edit<T>(&mut self, action: impl FnOnce(&mut Editor<'_>) -> Result<T>) -> Result<T> {
        let file = HermesFile::parse(&self.source)?;
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
    fn hermes_session(&mut self, path: &str) -> Result<&mut HermesSession> {
        if self.hermes.is_none() {
            let source = self
                .apk
                .map_component_entry(0, path)?
                .ok_or_else(|| PatcherError::NotFound(format!("base APK Hermes bundle {path}")))?;
            HermesFile::parse(&source)
                .map_err(|e| PatcherError::InvalidFile(format!("base APK {path}: {e}")))?;
            self.hermes = Some(HermesSession {
                path: path.into(),
                source,
                edits: None,
                modules: BTreeMap::new(),
            });
        }
        let session = self.hermes.as_mut().expect("Hermes opened on first use");
        if session.path != path {
            return Err(PatcherError::InvalidFile(format!(
                "Hermes scope already opened {}, cannot also open {path}",
                session.path
            )));
        }
        Ok(session)
    }

    pub(crate) fn hermes_version(&mut self, path: &str) -> Result<u32> {
        let session = self.hermes_session(path)?;
        Ok(HermesFile::parse(&session.source)?.version())
    }

    pub(crate) fn hermes_find(
        &mut self,
        path: &str,
        name: Option<&str>,
        strings: &[&str],
        parameters: Option<u32>,
    ) -> Result<FunctionId> {
        let session = self.hermes_session(path)?;
        Ok(HermesFile::parse(&session.source)?.find_function(name, strings, parameters)?)
    }

    pub(crate) fn hermes_wrap(
        &mut self,
        path: &str,
        function: FunctionId,
        module: &Path,
        export: &str,
    ) -> Result<()> {
        let session = self.hermes_session(path)?;
        let module_id = if let Some(&id) = session.modules.get(module) {
            id
        } else {
            let file = std::fs::File::open(module).map_err(|e| {
                PatcherError::Bundle(format!("Hermes extension {}: {e}", module.display()))
            })?;
            // SAFETY: bundle payloads are validated immutable snapshots retained for the run.
            let source = unsafe { reseam_storage::map_file(&file)? };
            let module_file = HermesFile::parse(&source)?;
            let id = session.edit(|editor| Ok(editor.link(&module_file)?))?;
            session.modules.insert(module.to_owned(), id);
            id
        };
        session.edit(|editor| Ok(editor.wrap(function, module_id, export)?))
    }

    pub(crate) fn finish_hermes(&mut self) -> Result<()> {
        let Some(session) = &mut self.hermes else {
            return Ok(());
        };
        let mut file = reseam_storage::temporary_file()?;
        session.edit(|editor| {
            let mut output = BufWriter::new(&mut file);
            editor.write(&mut output)?;
            output.flush()?;
            Ok(())
        })?;
        self.apk
            .inject_file_spooled(0, &session.path, file, Compression::Stored)?;
        Ok(())
    }
}
