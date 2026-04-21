// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use reseam_apk::{ApkEntryPath, AxmlDocument};

use super::PatchContext;
use crate::error::{PatcherError, Result as PatcherResult};

impl<'a> PatchContext<'a> {
    pub fn inject_file(&mut self, apk_path: &str, data: Vec<u8>) {
        let Some(data) = self.auto_compile_xml(apk_path, data) else {
            return;
        };
        if apk_path == "AndroidManifest.xml" {
            match AxmlDocument::parse(&data) {
                Ok(document) => {
                    *self.apk.manifest_mut() = document;
                }
                Err(e) => {
                    self.log().warn(format!(
                        "inject_file: failed to parse compiled manifest {apk_path}: {e}"
                    ));
                }
            }
            return;
        }
        self.apk.inject_file(apk_path, data);
    }

    pub fn inject_file_into_component(
        &mut self,
        component_index: usize,
        apk_path: &str,
        data: Vec<u8>,
    ) {
        let Some(data) = self.auto_compile_xml_in_component(component_index, apk_path, data) else {
            return;
        };
        if apk_path == "AndroidManifest.xml" {
            match AxmlDocument::parse(&data) {
                Ok(document) => {
                    if let Some(manifest) = self.apk.component_manifest_mut(component_index) {
                        *manifest = document;
                    }
                }
                Err(e) => {
                    self.log().warn(format!(
                        "inject_file_into_component: failed to parse compiled manifest {apk_path}: {e}"
                    ));
                }
            }
            return;
        }
        self.apk.inject_file_into(component_index, apk_path, data);
    }

    fn auto_compile_xml(&mut self, apk_path: &str, data: Vec<u8>) -> Option<Vec<u8>> {
        if !apk_path.ends_with(".xml") {
            return Some(data);
        }
        if reseam_apk::axml::compiler::is_compiled_axml(&data) {
            return Some(data);
        }
        match std::str::from_utf8(&data) {
            Ok(text) => {
                let result = reseam_apk::axml::compiler::compile_xml_with_resources(
                    text,
                    self.apk.resources_mut(),
                );
                match result {
                    Ok(compiled) => Some(compiled),
                    Err(e) if requires_compiled_xml(apk_path) => {
                        self.log().warn(format!(
                            "inject_file: refused to inject plain XML at {apk_path}: {e}"
                        ));
                        None
                    }
                    Err(_) => Some(data),
                }
            }
            Err(_) if requires_compiled_xml(apk_path) => {
                self.log().warn(format!(
                    "inject_file: refused to inject non-UTF8 XML at {apk_path}"
                ));
                None
            }
            Err(_) => Some(data),
        }
    }

    fn auto_compile_xml_in_component(
        &mut self,
        component_index: usize,
        apk_path: &str,
        data: Vec<u8>,
    ) -> Option<Vec<u8>> {
        if !apk_path.ends_with(".xml") {
            return Some(data);
        }
        if reseam_apk::axml::compiler::is_compiled_axml(&data) {
            return Some(data);
        }
        match std::str::from_utf8(&data) {
            Ok(text) => {
                let result = reseam_apk::axml::compiler::compile_xml_with_resources(
                    text,
                    self.apk.component_resources_mut(component_index),
                );
                match result {
                    Ok(compiled) => Some(compiled),
                    Err(e) if requires_compiled_xml(apk_path) => {
                        self.log().warn(format!(
                            "inject_file_into_component: refused to inject plain XML at {apk_path}: {e}"
                        ));
                        None
                    }
                    Err(_) => Some(data),
                }
            }
            Err(_) if requires_compiled_xml(apk_path) => {
                self.log().warn(format!(
                    "inject_file_into_component: refused to inject non-UTF8 XML at {apk_path}"
                ));
                None
            }
            Err(_) => Some(data),
        }
    }

    pub fn inject_file_stored(&mut self, apk_path: &str, data: Vec<u8>) {
        self.apk.inject_file_stored(apk_path, data);
    }

    pub fn inject_file_stored_into_component(
        &mut self,
        component_index: usize,
        apk_path: &str,
        data: Vec<u8>,
    ) {
        self.apk
            .inject_file_stored_into(component_index, apk_path, data);
    }

    pub fn delete_file(&mut self, apk_path: &str) {
        self.apk.delete_file(apk_path);
    }

    pub fn delete_file_from_component(&mut self, component_index: usize, apk_path: &str) {
        self.apk.delete_file_from(component_index, apk_path);
    }

    pub fn list_files(&self) -> &[ApkEntryPath] {
        self.apk.entry_names()
    }

    pub fn list_files_in_component(&self, component_index: usize) -> Option<&[ApkEntryPath]> {
        self.apk.component_entry_names(component_index)
    }

    pub fn read_file(&self, apk_path: &str) -> Option<Vec<u8>> {
        self.apk.read_entry(apk_path).ok()
    }

    pub fn read_file_from_component(
        &self,
        component_index: usize,
        apk_path: &str,
    ) -> Option<Vec<u8>> {
        self.apk
            .read_entry_from_component(component_index, apk_path)
            .ok()
    }

    pub fn copy_resource_group(
        &mut self,
        bundle_dir: &Path,
        res_type: &str,
        files: &[&str],
    ) -> PatcherResult<usize> {
        let mut count = 0;
        for file_name in files {
            let src = bundle_dir.join("resources").join(res_type).join(file_name);
            if !src.exists() {
                return Err(PatcherError::Bundle {
                    reason: format!("missing resource file {}", src.display()),
                });
            }
            let data = std::fs::read(&src)?;
            let apk_path = format!("res/{res_type}/{file_name}");
            self.inject_file(&apk_path, data);
            count += 1;
        }
        Ok(count)
    }
}

fn requires_compiled_xml(apk_path: &str) -> bool {
    apk_path == "AndroidManifest.xml" || apk_path.starts_with("res/")
}
