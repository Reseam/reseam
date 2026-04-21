// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod component;
mod source;
mod types;
mod write;

use reseam_dex::MultiDexContainer;

use crate::axml::reader::AxmlDocument;
use crate::error::{invalid, Result};
use crate::resources::ResourceTable;

pub use component::ApkComponent;
use component::ApkComponentSession;
pub use types::{ApkEntryPath, ComponentName};

/// The kind of APK: single or split bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApkKind {
    Single,
    Split,
}

#[derive(Debug, Clone, Copy)]
pub struct ApkWriteOptions {
    pub strip_signatures: bool,
}

impl Default for ApkWriteOptions {
    fn default() -> Self {
        Self {
            strip_signatures: true,
        }
    }
}

/// An opened APK file with parsed DEX and manifest access.
///
/// `ApkFile` remains the stable public facade, but internally it now acts as the
/// mutable APK session for the current refactor phase.
pub struct ApkFile {
    kind: ApkKind,
    dex: MultiDexContainer,
    dex_dirty: bool,
    components: Vec<ApkComponentSession>,
    entry_names: Vec<ApkEntryPath>,
}

impl ApkFile {
    fn base_component(&self) -> &ApkComponentSession {
        &self.components[0]
    }

    fn base_component_mut(&mut self) -> &mut ApkComponentSession {
        &mut self.components[0]
    }

    fn component(&self, index: usize) -> Option<&ApkComponentSession> {
        self.components.get(index)
    }

    fn component_mut(&mut self, index: usize) -> Option<&mut ApkComponentSession> {
        self.components.get_mut(index)
    }

    fn touch_entry_name(&mut self, path: &str) {
        if !self.entry_names.iter().any(|entry| entry.as_str() == path) {
            self.entry_names.push(path.into());
        }
    }

    fn remove_entry_name_if_absent(&mut self, path: &str) {
        let exists = self.components.iter().any(|component| {
            component
                .entry_names
                .iter()
                .any(|entry| entry.as_str() == path)
        });
        if !exists {
            self.entry_names.retain(|entry| entry.as_str() != path);
        }
    }

    /// Get a reference to the unified DEX container.
    pub fn dex(&self) -> &MultiDexContainer {
        &self.dex
    }

    /// Resolve the target DEX's lazy class data, then return it immutably.
    pub fn resolved_dex(&mut self, index: usize) -> Result<Option<&reseam_dex::DexFile>> {
        Ok(self.dex.dex_resolved(index)?)
    }

    /// Resolve the target DEX's lazy class data, then return it mutably.
    pub fn resolved_dex_mut(&mut self, index: usize) -> Result<Option<&mut reseam_dex::DexFile>> {
        self.dex_dirty = true;
        Ok(self.dex.dex_resolved_mut(index)?)
    }

    /// Get a mutable reference to the unified DEX container.
    pub fn dex_mut(&mut self) -> &mut MultiDexContainer {
        self.dex_dirty = true;
        &mut self.dex
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.base_component().manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        let component = self.base_component_mut();
        component.manifest_dirty = true;
        &mut component.manifest
    }

    pub fn resources(&self) -> Option<&ResourceTable> {
        self.base_component().resources.as_ref()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        let component = self.base_component_mut();
        component.resources_dirty = true;
        component.resources.as_mut()
    }

    pub fn component_meta(&self, index: usize) -> Option<&ApkComponent> {
        self.component(index).map(|component| &component.meta)
    }

    pub fn component_manifest(&self, index: usize) -> Option<&AxmlDocument> {
        self.component(index).map(|component| &component.manifest)
    }

    pub fn component_manifest_mut(&mut self, index: usize) -> Option<&mut AxmlDocument> {
        let component = self.component_mut(index)?;
        component.manifest_dirty = true;
        Some(&mut component.manifest)
    }

    pub fn component_resources(&self, index: usize) -> Option<&ResourceTable> {
        self.component(index)
            .and_then(|component| component.resources.as_ref())
    }

    pub fn component_resources_mut(&mut self, index: usize) -> Option<&mut ResourceTable> {
        let component = self.component_mut(index)?;
        component.resources_dirty = true;
        component.resources.as_mut()
    }

    pub fn component_entry_names(&self, index: usize) -> Option<&[ApkEntryPath]> {
        self.component(index)
            .map(|component| component.entry_names.as_slice())
    }

    pub fn component_index_by_name(&self, name: &str) -> Option<usize> {
        self.components
            .iter()
            .position(|component| component.meta.name.as_str() == name)
    }

    pub fn find_resource_component(&self, type_name: &str, entry_name: &str) -> Option<usize> {
        self.components
            .iter()
            .enumerate()
            .find_map(|(index, component)| {
                component
                    .resources
                    .as_ref()
                    .and_then(|resources| resources.find_resource_id(type_name, entry_name))
                    .map(|_| index)
            })
    }

    pub fn find_resource_component_by_id(&self, res_id: u32) -> Option<usize> {
        self.components
            .iter()
            .enumerate()
            .find_map(|(index, component)| {
                component
                    .resources
                    .as_ref()
                    .filter(|resources| resources.contains_resource_id(res_id))
                    .map(|_| index)
            })
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.components.iter().find_map(|component| {
            component
                .resources
                .as_ref()
                .and_then(|resources| resources.find_resource_id(type_name, entry_name))
        })
    }

    pub fn resource_exists(&self, type_name: &str, entry_name: &str) -> bool {
        self.find_resource_component(type_name, entry_name)
            .is_some()
    }

    pub fn get_string_resource_value(&self, name: &str) -> Option<&str> {
        self.components.iter().find_map(|component| {
            component
                .resources
                .as_ref()
                .and_then(|resources| resources.get_string_value(name))
        })
    }

    pub fn set_string_resource_value(&mut self, name: &str, value: &str) -> bool {
        let component_index = self.find_resource_component("string", name).unwrap_or(0);
        let Some(component) = self.component_mut(component_index) else {
            return false;
        };
        let Some(resources) = component.resources.as_mut() else {
            return false;
        };
        component.resources_dirty = true;
        resources.set_string_value(name, value)
    }

    pub fn component_find_resource_id(
        &self,
        component_index: usize,
        type_name: &str,
        entry_name: &str,
    ) -> Option<u32> {
        self.component(component_index)
            .and_then(|component| component.resources.as_ref())
            .and_then(|resources| resources.find_resource_id(type_name, entry_name))
    }

    /// Get the package name from the base manifest.
    pub fn package_name(&self) -> Option<&str> {
        self.base_component().manifest.package_name()
    }

    /// Get the version code from the base manifest.
    pub fn version_code(&self) -> Option<u32> {
        self.base_component().manifest.version_code()
    }

    /// Get the version name from the base manifest.
    pub fn version_name(&self) -> Option<&str> {
        self.base_component().manifest.version_name()
    }

    /// Whether this is a split APK bundle.
    pub fn is_split(&self) -> bool {
        self.kind == ApkKind::Split
    }

    /// Number of APK components (1 for single, 1+N for splits).
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Get the split names (empty for single APKs, excludes base).
    pub fn split_names(&self) -> Vec<&str> {
        self.components
            .iter()
            .skip(1)
            .map(|component| component.meta.name.as_str())
            .collect()
    }

    pub fn entry_names(&self) -> &[ApkEntryPath] {
        &self.entry_names
    }

    pub fn read_entry(&self, entry_name: &str) -> Result<Vec<u8>> {
        for component in &self.components {
            if let Some(data) = component.read_entry(entry_name)? {
                return Ok(data);
            }
        }
        Err(crate::error::ApkError::Invalid {
            section: "apk entry",
            reason: format!("entry not found in any component: {entry_name}"),
        })
    }

    pub fn read_entry_from_component(&self, index: usize, entry_name: &str) -> Result<Vec<u8>> {
        let component = self.component(index).ok_or_else(|| {
            invalid(
                "apk component",
                format!("component index out of range: {index}"),
            )
        })?;
        component.read_entry(entry_name)?.ok_or_else(|| {
            invalid(
                "apk entry",
                format!(
                    "entry not found in component {}: {entry_name}",
                    component.meta.name
                ),
            )
        })
    }

    /// Inject a file into the base component.
    pub fn inject_file(&mut self, path: &str, data: Vec<u8>) {
        self.inject_file_into(0, path, data);
    }

    /// Inject a stored file into the base component.
    pub fn inject_file_stored(&mut self, path: &str, data: Vec<u8>) {
        self.inject_file_stored_into(0, path, data);
    }

    pub fn inject_file_into(&mut self, component_index: usize, path: &str, data: Vec<u8>) {
        self.inject_file_with_method(
            component_index,
            path,
            data,
            zip::CompressionMethod::Deflated,
        );
    }

    pub fn inject_file_stored_into(&mut self, component_index: usize, path: &str, data: Vec<u8>) {
        self.inject_file_with_method(component_index, path, data, zip::CompressionMethod::Stored);
    }

    fn inject_file_with_method(
        &mut self,
        component_index: usize,
        path: &str,
        data: Vec<u8>,
        compression: zip::CompressionMethod,
    ) {
        if let Some(component) = self.component_mut(component_index) {
            component.inject_file(path, data, compression);
            self.touch_entry_name(path);
        }
    }

    /// Delete a file from the base component.
    pub fn delete_file(&mut self, path: &str) {
        self.delete_file_from(0, path);
    }

    pub fn delete_file_from(&mut self, component_index: usize, path: &str) {
        if let Some(component) = self.component_mut(component_index) {
            component.delete_file(path);
            self.remove_entry_name_if_absent(path);
        }
    }
}
