// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use reseam_apk::axml::{self, AxmlDocument};
use reseam_apk::entry::MANIFEST_ENTRY;
use reseam_apk::{ApkComponent, Compression};

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
        let component = self.component_mut(component)?;
        let data = compile_if_xml(component, path, data)?;
        if path == MANIFEST_ENTRY {
            *component.manifest_mut() = AxmlDocument::parse(&data)?;
        } else {
            component.inject_file(path, data, compression);
        }
        Ok(())
    }

    pub fn delete_file(&mut self, component: usize, path: &str) -> Result<()> {
        self.component_mut(component)?.delete_file(path);
        Ok(())
    }

    pub fn read_file(&mut self, component: usize, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.component_mut(component)?.read_entry(path)?)
    }

    /// Copies `<bundle_dir>/resources/<res_type>/<file>` into `res/<res_type>/<file>`.
    pub fn copy_resource_group(
        &mut self,
        bundle_dir: &Path,
        res_type: &str,
        files: &[&str],
    ) -> Result<usize> {
        for file in files {
            let source = bundle_dir.join("resources").join(res_type).join(file);
            let data = std::fs::read(&source).map_err(|e| {
                PatcherError::Bundle(format!("read resource file {}: {e}", source.display()))
            })?;
            self.inject_file(
                0,
                &format!("res/{res_type}/{file}"),
                data,
                Compression::Deflated,
            )?;
        }
        Ok(files.len())
    }

    pub fn component_mut(&mut self, index: usize) -> Result<&mut ApkComponent> {
        self.apk
            .component_mut(index)
            .ok_or_else(|| PatcherError::NotFound(format!("component index {index}")))
    }
}

fn compile_if_xml(component: &mut ApkComponent, path: &str, data: Vec<u8>) -> Result<Vec<u8>> {
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
    match axml::compile_xml(text, component.resources_mut()?) {
        Ok(compiled) => Ok(compiled),
        Err(error) if must_compile => Err(PatcherError::InvalidFile(format!("{path}: {error}"))),
        Err(_) => Ok(data),
    }
}
