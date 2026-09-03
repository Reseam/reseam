// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod component;
mod source;
mod types;
mod write;

use std::borrow::Cow;
use std::num::NonZeroUsize;
use reseam_dex::{DexFile, MultiDexContainer};

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
    /// Threads serializing and deflating dirty DEX files concurrently. Each
    /// holds one DEX's writer state, so this bounds the write-phase memory.
    pub dex_workers: NonZeroUsize,
    /// Deflate level for rewritten DEX entries. Level 3 compresses within two
    /// percent of level 6 in a quarter less time; level 1 is another 2x faster
    /// at about 13 points larger.
    pub dex_compression_level: i64,
}

impl Default for ApkWriteOptions {
    fn default() -> Self {
        Self {
            strip_signatures: true,
            dex_workers: std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
            dex_compression_level: 3,
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
    dex_sessions: Vec<DexSessionEntry>,
    components: Vec<ApkComponentSession>,
    entry_names: Vec<ApkEntryPath>,
}

#[derive(Debug, Clone)]
struct DexEntryOrigin {
    component_index: usize,
    entry_name: ApkEntryPath,
}

/// Where a DEX in the container came from. Whether it needs rewriting is the
/// DEX's own [`reseam_dex::DexFile::is_dirty`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DexEntryState {
    Existing,
    Added,
}

#[derive(Debug, Clone)]
struct DexSessionEntry {
    origin: Option<DexEntryOrigin>,
    state: DexEntryState,
}

impl DexSessionEntry {
    fn existing(component_index: usize, entry_name: ApkEntryPath) -> Self {
        Self {
            origin: Some(DexEntryOrigin {
                component_index,
                entry_name,
            }),
            state: DexEntryState::Existing,
        }
    }

    fn added() -> Self {
        Self {
            origin: None,
            state: DexEntryState::Added,
        }
    }
}

pub struct ApkDexMut<'a> {
    apk: &'a mut ApkFile,
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

    fn any_dex_dirty(&self) -> bool {
        self.dex_sessions
            .iter()
            .any(|entry| entry.state == DexEntryState::Added)
            || self.dex.iter().any(reseam_dex::DexFile::is_dirty)
    }

    /// Get a reference to the unified DEX container.
    pub fn dex(&self) -> &MultiDexContainer {
        &self.dex
    }

    pub fn resolve_dex_class(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&reseam_dex::DexFile>> {
        Ok(self.dex.dex_class_resolved(index, class_idx)?)
    }

    pub fn resolve_dex_class_mut(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&mut reseam_dex::DexFile>> {
        Ok(self.dex.dex_class_resolved_mut(index, class_idx)?)
    }

    /// Mutable access to one DEX without resolving deferred class data. Used
    /// for whole-DEX operations (interning, adding classes) that do not read
    /// existing class data.
    pub fn dex_mut_at(&mut self, index: usize) -> Option<&mut reseam_dex::DexFile> {
        self.dex.dex_mut(index)
    }

    /// Get tracked mutable access to the unified DEX container.
    pub fn dex_mut(&mut self) -> ApkDexMut<'_> {
        ApkDexMut { apk: self }
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.base_component().manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        let component = self.base_component_mut();
        component.manifest_dirty = true;
        &mut component.manifest
    }

    pub fn resources(&mut self) -> Option<&ResourceTable> {
        self.base_component_mut().resources()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        self.base_component_mut().resources_mut()
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

    pub fn component_has_resources(&self, index: usize) -> bool {
        self.component(index)
            .is_some_and(ApkComponentSession::has_resource_entry)
    }

    pub fn component_resources(&mut self, index: usize) -> Option<&ResourceTable> {
        self.component_mut(index)?.resources()
    }

    pub fn component_resources_mut(&mut self, index: usize) -> Option<&mut ResourceTable> {
        self.component_mut(index)?.resources_mut()
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

    pub fn find_resource_component(&mut self, type_name: &str, entry_name: &str) -> Option<usize> {
        for index in 0..self.components.len() {
            let Some(resources) = self
                .component_mut(index)
                .and_then(|component| component.resources())
            else {
                continue;
            };
            if resources.find_resource_id(type_name, entry_name).is_some() {
                return Some(index);
            }
        }
        None
    }

    pub fn find_resource_component_by_id(&mut self, res_id: u32) -> Option<usize> {
        for index in 0..self.components.len() {
            let Some(resources) = self
                .component_mut(index)
                .and_then(|component| component.resources())
            else {
                continue;
            };
            if resources.contains_resource_id(res_id) {
                return Some(index);
            }
        }
        None
    }

    pub fn find_resource_id(&mut self, type_name: &str, entry_name: &str) -> Option<u32> {
        for index in 0..self.components.len() {
            let Some(resources) = self
                .component_mut(index)
                .and_then(|component| component.resources())
            else {
                continue;
            };
            if let Some(res_id) = resources.find_resource_id(type_name, entry_name) {
                return Some(res_id);
            }
        }
        None
    }

    pub fn resource_exists(&mut self, type_name: &str, entry_name: &str) -> bool {
        self.find_resource_component(type_name, entry_name)
            .is_some()
    }

    pub fn get_string_resource_value(&mut self, name: &str) -> Option<String> {
        (0..self.components.len()).find_map(|index| {
            self.component_mut(index)
                .and_then(|component| component.resources())
                .and_then(|resources| resources.get_string_value(name))
                .map(Cow::into_owned)
        })
    }

    pub fn set_string_resource_value(&mut self, name: &str, value: &str) -> bool {
        let component_index = self.find_resource_component("string", name).unwrap_or(0);
        let Some(component) = self.component_mut(component_index) else {
            return false;
        };
        let Some(resources) = component.resources_mut() else {
            return false;
        };
        resources.set_string_value(name, value)
    }

    pub fn component_find_resource_id(
        &mut self,
        component_index: usize,
        type_name: &str,
        entry_name: &str,
    ) -> Option<u32> {
        self.component_mut(component_index)
            .and_then(|component| component.resources())?
            .find_resource_id(type_name, entry_name)
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
        mut compression: zip::CompressionMethod,
    ) {
        if crate::zip::writer::is_native_library_entry(path) {
            compression = zip::CompressionMethod::Stored;
        }
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

impl<'a> ApkDexMut<'a> {
    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.apk.dex.dex_mut(index)
    }

    pub fn add_dex(&mut self, dex: DexFile) {
        self.apk.dex.add_dex(dex);
        self.apk.dex_sessions.push(DexSessionEntry::added());
    }

    pub fn len(&self) -> usize {
        self.apk.dex.len()
    }

    pub fn is_empty(&self) -> bool {
        self.apk.dex.is_empty()
    }
}
