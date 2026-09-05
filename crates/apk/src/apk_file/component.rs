// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_dex::file::DexBytes;

use crate::axml::AxmlDocument;
use crate::entry::{is_native_library, MANIFEST_ENTRY, RESOURCES_ENTRY};
use crate::error::Result;
use crate::resources::ResourceTable;
use crate::zip::reader::{self, Archive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Deflated,
    Stored,
}

impl Compression {
    pub(crate) fn method(self) -> zip::CompressionMethod {
        match self {
            Self::Deflated => zip::CompressionMethod::Deflated,
            Self::Stored => zip::CompressionMethod::Stored,
        }
    }
}

pub struct ApkComponent {
    name: String,
    path: PathBuf,
    archive: Archive,
    manifest: AxmlDocument,
    manifest_dirty: bool,
    resources: Resources,
    resources_dirty: bool,
    injected: HashMap<String, (Vec<u8>, Compression)>,
    deleted: HashSet<String>,
    original_dex_names: Vec<String>,
}

enum Resources {
    Absent,
    Deferred,
    Loaded(ResourceTable),
}

impl ApkComponent {
    /// `name` defaults to the manifest's split name, then the file stem.
    pub(crate) fn open(path: &Path, name: Option<String>, defer_resources: bool) -> Result<Self> {
        let mut archive = reader::open_archive(path)?;
        let manifest = AxmlDocument::parse(&reader::read_entry(&mut archive, MANIFEST_ENTRY)?)?;
        let name = name
            .or_else(|| manifest.split_name().map(Cow::into_owned))
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });
        let resources = if !reader::contains(&archive, RESOURCES_ENTRY) {
            Resources::Absent
        } else if defer_resources {
            Resources::Deferred
        } else {
            Resources::Loaded(load_resources(&mut archive)?)
        };
        let original_dex_names = reader::dex_entry_names(&archive);
        Ok(Self {
            name,
            path: path.to_path_buf(),
            archive,
            manifest,
            manifest_dirty: false,
            resources,
            resources_dirty: false,
            injected: HashMap::new(),
            deleted: HashSet::new(),
            original_dex_names,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        self.manifest_dirty = true;
        &mut self.manifest
    }

    pub fn has_resources(&self) -> bool {
        !matches!(self.resources, Resources::Absent)
    }

    /// The resource table, parsed on first access; `None` when the component
    /// has no `resources.arsc`.
    pub fn resources(&mut self) -> Result<Option<&ResourceTable>> {
        self.load_resources()?;
        Ok(match &self.resources {
            Resources::Loaded(table) => Some(table),
            _ => None,
        })
    }

    pub fn resources_mut(&mut self) -> Result<Option<&mut ResourceTable>> {
        self.load_resources()?;
        let Resources::Loaded(table) = &mut self.resources else {
            return Ok(None);
        };
        self.resources_dirty = true;
        Ok(Some(table))
    }

    fn load_resources(&mut self) -> Result<()> {
        if matches!(self.resources, Resources::Deferred) {
            self.resources = Resources::Loaded(load_resources(&mut self.archive)?);
        }
        Ok(())
    }

    /// Entries as they will be written: the archive's minus deletions, plus
    /// injected files.
    pub fn entry_names(&self) -> Vec<String> {
        let mut names: Vec<String> = reader::entry_names(&self.archive)
            .into_iter()
            .filter(|name| !self.deleted.contains(name))
            .collect();
        names.extend(
            self.injected
                .keys()
                .filter(|name| !reader::contains(&self.archive, name.as_str()))
                .cloned(),
        );
        names
    }

    pub fn contains(&self, name: &str) -> bool {
        !self.deleted.contains(name)
            && (self.injected.contains_key(name) || reader::contains(&self.archive, name))
    }

    /// The entry's current bytes: injected data, a dirty manifest or resource
    /// table serialized, or the archive's copy.
    pub fn read_entry(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        if self.deleted.contains(name) {
            return Ok(None);
        }
        if let Some((data, _)) = self.injected.get(name) {
            return Ok(Some(data.clone()));
        }
        if let Some(bytes) = self.manifest_bytes()? {
            if name == MANIFEST_ENTRY {
                return Ok(Some(bytes));
            }
        }
        if name == RESOURCES_ENTRY && self.resources_dirty {
            if let Resources::Loaded(table) = &self.resources {
                return table.serialize().map(Some);
            }
        }
        if !reader::contains(&self.archive, name) {
            return Ok(None);
        }
        reader::read_entry(&mut self.archive, name).map(Some)
    }

    /// Native libraries are always stored, since the platform maps them.
    pub fn inject_file(&mut self, path: &str, data: Vec<u8>, compression: Compression) {
        let compression = if is_native_library(path) {
            Compression::Stored
        } else {
            compression
        };
        self.deleted.remove(path);
        self.injected.insert(path.into(), (data, compression));
    }

    pub fn delete_file(&mut self, path: &str) {
        self.injected.remove(path);
        self.deleted.insert(path.into());
    }

    pub(crate) fn archive(&self) -> &Archive {
        &self.archive
    }

    pub(crate) fn manifest_bytes(&self) -> Result<Option<Vec<u8>>> {
        self.manifest_dirty
            .then(|| self.manifest.serialize())
            .transpose()
    }

    pub(crate) fn resources_file(&self) -> Result<Option<File>> {
        match &self.resources {
            Resources::Loaded(table) if self.resources_dirty => table.serialize_spooled().map(Some),
            _ => Ok(None),
        }
    }

    pub(crate) fn injected(&self) -> &HashMap<String, (Vec<u8>, Compression)> {
        &self.injected
    }

    pub(crate) fn deleted(&self) -> &HashSet<String> {
        &self.deleted
    }

    pub(crate) fn original_dex_names(&self) -> &[String] {
        &self.original_dex_names
    }
}

fn load_resources(archive: &mut Archive) -> Result<ResourceTable> {
    let mapped = reader::map_entry(archive, RESOURCES_ENTRY)?;
    ResourceTable::parse(DexBytes::from_mmap(Arc::new(mapped)))
}
