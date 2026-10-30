// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;

use reseam_apk::axml;
use reseam_apk::entry::MANIFEST_ENTRY;
use reseam_apk::{ApkComponent, ApkFile, Compression};

use super::PatchContext;
use crate::error::{PatcherError, Result};

impl PatchContext<'_> {
    /// Adds or replaces an entry in `component`. Plain-text XML under `res/`
    /// or at the manifest path is compiled first; a compiled manifest replaces
    /// the component's parsed manifest.
    pub fn inject_file(
        &mut self,
        component: usize,
        path: &str,
        data: Vec<u8>,
        compression: Compression,
    ) -> Result<()> {
        let data = compiled_xml(self.apk_mut(), component, path, &data)?.unwrap_or(data);
        Ok(self
            .apk_mut()
            .inject_file(component, path, data, compression)?)
    }

    /// [`Self::inject_file`] for an entry already written to `file`, which the
    /// APK takes over without loading it, unless it is XML to compile. Nothing
    /// may write to `file` afterwards.
    pub fn inject_spooled(
        &mut self,
        component: usize,
        path: &str,
        file: File,
        compression: Compression,
    ) -> Result<()> {
        if names_xml(path) {
            // SAFETY: the caller hands over a complete file nothing writes to anymore.
            let data = unsafe { reseam_storage::map_file(&file)? };
            if let Some(compiled) = compiled_xml(self.apk_mut(), component, path, &data)? {
                return Ok(self
                    .apk_mut()
                    .inject_file(component, path, compiled, compression)?);
            }
        }
        Ok(self
            .apk_mut()
            .inject_file_spooled(component, path, file, compression)?)
    }

    pub fn delete_file(&mut self, component: usize, path: &str) -> Result<()> {
        Ok(self.apk_mut().delete_file(component, path)?)
    }

    pub fn read_file(&mut self, component: usize, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.apk_mut().read_component_entry(component, path)?)
    }

    pub fn component_mut(&mut self, index: usize) -> Result<&mut ApkComponent> {
        self.apk
            .component_mut(index)
            .ok_or_else(|| PatcherError::NotFound(format!("component index {index}")))
    }
}

#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "Android resource entry suffixes are case-sensitive"
)]
fn names_xml(path: &str) -> bool {
    path.ends_with(".xml")
}

/// `data` compiled, when `path` names plain-text XML; `None` when it stays as
/// it is. XML that must compile and does not is an error.
fn compiled_xml(
    apk: &mut ApkFile,
    component: usize,
    path: &str,
    data: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !names_xml(path) || axml::is_compiled_axml(data) {
        return Ok(None);
    }
    let must_compile = path == MANIFEST_ENTRY || path.starts_with("res/");
    let Ok(text) = std::str::from_utf8(data) else {
        return if must_compile {
            Err(PatcherError::InvalidFile(format!(
                "{path}: XML is not UTF-8"
            )))
        } else {
            Ok(None)
        };
    };
    match apk.with_resource_scope(component, |scope| axml::compile_xml(text, scope))? {
        Ok(compiled) => Ok(Some(compiled)),
        Err(error) if must_compile => Err(PatcherError::InvalidFile(format!("{path}: {error}"))),
        Err(_) => Ok(None),
    }
}
