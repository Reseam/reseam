// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use crate::axml::reader::AxmlDocument;
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
pub(super) struct ApkComponentSession {
    pub meta: ApkComponent,
    pub manifest: AxmlDocument,
    pub manifest_dirty: bool,
    pub resources: Option<ResourceTable>,
    pub resources_dirty: bool,
    pub entry_names: Vec<ApkEntryPath>,
    pub injected_files: HashMap<ApkEntryPath, (Vec<u8>, zip::CompressionMethod)>,
    pub deleted_files: HashSet<ApkEntryPath>,
}

impl ApkComponentSession {
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
            if let Some(resources) = &self.resources {
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
}
