// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
        let data = compile_if_xml(self.apk_mut(), component, path, data)?;
        Ok(self
            .apk_mut()
            .inject_file(component, path, data, compression)?)
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
fn compile_if_xml(
    apk: &mut ApkFile,
    component: usize,
    path: &str,
    data: Vec<u8>,
) -> Result<Vec<u8>> {
    if !path.ends_with(".xml") || axml::is_compiled_axml(&data) {
        return Ok(data);
    }
    let must_compile = path == MANIFEST_ENTRY || path.starts_with("res/");
    let Ok(text) = std::str::from_utf8(&data) else {
        return if must_compile {
            Err(PatcherError::InvalidFile(format!(
                "{path}: XML is not UTF-8"
            )))
        } else {
            Ok(data)
        };
    };
    match apk.with_resource_scope(component, |scope| axml::compile_xml(text, scope))? {
        Ok(compiled) => Ok(compiled),
        Err(error) if must_compile => Err(PatcherError::InvalidFile(format!("{path}: {error}"))),
        Err(_) => Ok(data),
    }
}
