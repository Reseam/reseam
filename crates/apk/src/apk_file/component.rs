// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use tracing::warn;

use crate::axml::reader::AxmlDocument;
use crate::error::ApkError;
use crate::resources::ResourceTable;
use crate::zip::reader::ApkReader;
use crate::Result;

use super::{ApkEntryPath, ComponentName};

/// Metadata for a single APK component (base or split).
#[derive(Debug, Clone)]
pub struct ApkComponent {
    /// Human-readable name (e.g. "base", "split_config.arm64_v8a").
    pub name: ComponentName,
    /// Original file path.
    pub path: PathBuf,
    /// DEX entry names originally present in this APK.
    pub original_dex_names: Vec<ApkEntryPath>,
}

#[derive(Debug, Clone)]
pub(super) enum ResourceSession {
    Absent,
    Deferred,
    Loaded(ResourceTable),
    Unavailable,
}

impl ResourceSession {
    pub fn is_present(&self) -> bool {
        !matches!(self, Self::Absent)
    }

    pub fn loaded(&self) -> Option<&ResourceTable> {
        match self {
            Self::Loaded(resources) => Some(resources),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ApkComponentSession {
    pub meta: ApkComponent,
    pub manifest: AxmlDocument,
    pub manifest_dirty: bool,
    pub resources: ResourceSession,
    pub resources_dirty: bool,
    pub entry_names: Vec<ApkEntryPath>,
    pub injected_files: HashMap<ApkEntryPath, (Vec<u8>, zip::CompressionMethod)>,
    pub deleted_files: HashSet<ApkEntryPath>,
}

impl ApkComponentSession {
    pub fn has_resource_entry(&self) -> bool {
        self.resources.is_present()
    }

    pub fn resources_loaded(&self) -> Option<&ResourceTable> {
        self.resources.loaded()
    }

    pub fn resources(&mut self) -> Option<&ResourceTable> {
        self.ensure_resources_loaded();
        self.resources.loaded()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        self.ensure_resources_loaded();
        let ResourceSession::Loaded(resources) = &mut self.resources else {
            return None;
        };
        self.resources_dirty = true;
        Some(resources)
    }

    fn ensure_resources_loaded(&mut self) {
        if !matches!(self.resources, ResourceSession::Deferred) {
            return;
        }

        let file = match File::open(&self.meta.path) {
            Ok(file) => file,
            Err(error) => {
                warn!(
                    component = %self.meta.name,
                    path = %self.meta.path.display(),
                    %error,
                    "failed to open component while loading resources"
                );
                self.resources = ResourceSession::Unavailable;
                return;
            }
        };
        let mut reader = match ApkReader::new(BufReader::new(file)) {
            Ok(reader) => reader,
            Err(error) => {
                warn!(
                    component = %self.meta.name,
                    path = %self.meta.path.display(),
                    %error,
                    "failed to open ZIP while loading resources"
                );
                self.resources = ResourceSession::Unavailable;
                return;
            }
        };

        let arsc_bytes = match reader.read_entry("resources.arsc") {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!(
                    component = %self.meta.name,
                    path = %self.meta.path.display(),
                    %error,
                    "failed to read resources.arsc"
                );
                self.resources = ResourceSession::Unavailable;
                return;
            }
        };

        match ResourceTable::parse(&arsc_bytes) {
            Ok(resources) => {
                self.resources = ResourceSession::Loaded(resources);
            }
            Err(ApkError::Unsupported { feature, detail })
                if feature == "resource string pool styles" =>
            {
                warn!(
                    component = %self.meta.name,
                    feature,
                    detail,
                    "loading APK without mutable resource table support"
                );
                self.resources = ResourceSession::Unavailable;
            }
            Err(error) => {
                warn!(
                    component = %self.meta.name,
                    path = %self.meta.path.display(),
                    %error,
                    "failed to parse resources.arsc"
                );
                self.resources = ResourceSession::Unavailable;
            }
        }
    }

    pub fn read_entry(&self, entry_name: &str) -> Result<Option<Vec<u8>>> {
        if self.deleted_files.contains(entry_name) {
            return Ok(None);
        }
        if let Some((data, _)) = self.injected_files.get(entry_name) {
            return Ok(Some(data.clone()));
        }
        if entry_name == "AndroidManifest.xml" && self.manifest_dirty {
            return self.manifest.serialize().map(Some);
        }
        if entry_name == "resources.arsc" && self.resources_dirty {
            if let ResourceSession::Loaded(resources) = &self.resources {
                return resources.serialize().map(Some);
            }
        }

        let file = File::open(&self.meta.path)?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        if reader.contains(entry_name) {
            return reader.read_entry(entry_name).map(Some);
        }
        Ok(None)
    }

    pub fn inject_file(&mut self, path: &str, data: Vec<u8>, compression: zip::CompressionMethod) {
        self.injected_files.insert(path.into(), (data, compression));
        self.deleted_files.remove(path);
        if !self.entry_names.iter().any(|entry| entry.as_str() == path) {
            self.entry_names.push(path.into());
        }
    }

    pub fn delete_file(&mut self, path: &str) {
        self.deleted_files.insert(path.into());
        self.injected_files.remove(path);
        self.entry_names.retain(|entry| entry.as_str() != path);
    }

    pub fn finalize_write(&mut self, output_path: PathBuf) -> Result<()> {
        let file = File::open(&output_path)?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        let entry_names = reader.entry_names();
        let original_dex_names = reader.dex_entry_names();
        let has_resources = entry_names.iter().any(|entry| entry == "resources.arsc");
        let resources_replaced = self.resources_dirty
            || self.injected_files.contains_key("resources.arsc")
            || self.deleted_files.contains("resources.arsc");
        let resources = std::mem::replace(&mut self.resources, ResourceSession::Absent);

        self.meta.path = output_path;
        self.meta.original_dex_names = original_dex_names
            .into_iter()
            .map(ApkEntryPath::from)
            .collect();
        self.entry_names = entry_names.into_iter().map(ApkEntryPath::from).collect();
        self.manifest_dirty = false;
        self.resources_dirty = false;
        self.injected_files.clear();
        self.deleted_files.clear();
        self.resources = if has_resources {
            match resources {
                ResourceSession::Loaded(resources) => ResourceSession::Loaded(resources),
                ResourceSession::Unavailable if !resources_replaced => ResourceSession::Unavailable,
                _ => ResourceSession::Deferred,
            }
        } else {
            ResourceSession::Absent
        };
        Ok(())
    }
}
