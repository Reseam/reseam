// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_dex::types::header::Loading;
use reseam_storage::Bytes;

use crate::axml::AxmlDocument;
use crate::entry::{EntryName, MANIFEST_ENTRY, RESOURCES_ENTRY, is_native_library};
use crate::error::{Result, invalid};
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
    resources: Resources,
    edits: BTreeMap<EntryName, EntryEdit>,
}

pub(super) enum EntryEdit {
    Staged {
        file: File,
        compression: Compression,
    },
    Manifest,
    Resources,
    Dex,
    Deleted,
}

enum Resources {
    Absent,
    Deferred,
    Loaded(Box<ResourceTable>),
}

impl ApkComponent {
    pub(crate) fn open(path: &Path, loading: Loading) -> Result<Self> {
        let mut archive = reader::open_archive(path)?;
        let manifest =
            (|| AxmlDocument::parse(&reader::read_entry(&mut archive, MANIFEST_ENTRY)?))()
                .map_err(|error| error.in_entry(MANIFEST_ENTRY).in_file(path))?;
        let name = manifest
            .split_name()
            .map_or_else(|| "base".into(), Cow::into_owned);
        let resources = if reader::contains(&archive, RESOURCES_ENTRY) {
            Resources::Deferred
        } else {
            Resources::Absent
        };
        let mut component = Self {
            name,
            path: path.to_path_buf(),
            archive,
            manifest,
            resources,
            edits: BTreeMap::new(),
        };
        if loading == Loading::Eager {
            component.resources()?;
        }
        Ok(component)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Maps the original APK, including its signing block. Staged edits are
    /// available through the session's entry APIs instead.
    pub fn source(&self) -> Result<memmap2::Mmap> {
        reader::map_file(&self.archive)
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        self.edits
            .insert(MANIFEST_ENTRY.into(), EntryEdit::Manifest);
        &mut self.manifest
    }

    pub fn has_resources(&self) -> bool {
        !matches!(self.resources, Resources::Absent)
    }

    /// Parses the current resource entry on first access. A deleted or absent
    /// table returns `None`; an unreadable table returns an error.
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
        self.edits
            .insert(RESOURCES_ENTRY.into(), EntryEdit::Resources);
        Ok(Some(table))
    }

    fn load_resources(&mut self) -> Result<()> {
        if matches!(self.resources, Resources::Deferred) {
            let mapped = reader::map_entry(&mut self.archive, RESOURCES_ENTRY)?;
            self.resources = Resources::Loaded(Box::new(ResourceTable::parse(Bytes::from_mmap(
                Arc::new(mapped),
            ))?));
        }
        Ok(())
    }

    /// Current entry names, in archive order followed by new entries in name
    /// order. DEX names allocated by the session are included.
    pub fn entry_names(&self) -> Vec<String> {
        reader::entry_names(&self.archive)
            .into_iter()
            .filter(|name| self.contains(name))
            .chain(
                self.edits
                    .iter()
                    .filter(|(name, edit)| {
                        !matches!(edit, EntryEdit::Deleted)
                            && !reader::contains(&self.archive, name.as_str())
                    })
                    .map(|(name, _)| name.to_string()),
            )
            .collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        match self.edits.get(name) {
            Some(EntryEdit::Deleted) => false,
            Some(_) => true,
            None => reader::contains(&self.archive, name),
        }
    }

    pub(super) fn stage(
        &mut self,
        path: &str,
        file: File,
        manifest: Option<AxmlDocument>,
        compression: Compression,
    ) -> Result<()> {
        let resources = if path == RESOURCES_ENTRY {
            Some(ResourceTable::parse(Bytes::from_mmap(Arc::new(
                reader::map_spooled(&file)?,
            )))?)
        } else {
            None
        };
        if let Some(manifest) = manifest {
            self.manifest = manifest;
        }
        if let Some(resources) = resources {
            self.resources = Resources::Loaded(Box::new(resources));
        }
        let compression = if is_native_library(path) {
            Compression::Stored
        } else {
            compression
        };
        self.edits
            .insert(path.into(), EntryEdit::Staged { file, compression });
        Ok(())
    }

    pub(super) fn delete(&mut self, path: &str) -> Result<()> {
        if path == MANIFEST_ENTRY {
            return Err(invalid(
                "apk entry",
                "AndroidManifest.xml is required and cannot be deleted",
            ));
        }
        if path == RESOURCES_ENTRY {
            self.resources = Resources::Absent;
        }
        self.edits.insert(path.into(), EntryEdit::Deleted);
        Ok(())
    }

    pub(super) fn add_dex_entry(&mut self, name: &str) {
        self.edits.insert(name.into(), EntryEdit::Dex);
    }

    pub(crate) fn archive(&self) -> &Archive {
        &self.archive
    }

    pub(super) fn edits(&self) -> &BTreeMap<EntryName, EntryEdit> {
        &self.edits
    }

    pub(super) fn reserved_names(&self) -> impl Iterator<Item = &str> {
        self.archive
            .file_names()
            .chain(self.edits.keys().map(EntryName::as_str))
    }

    pub(super) fn copy_entry(&mut self, name: &str, output: &mut impl Write) -> Result<bool> {
        if !self.contains(name) {
            return Ok(false);
        }
        match self.edits.get(name) {
            Some(EntryEdit::Staged { file, .. }) => {
                reader::copy_spooled(file, output)?;
            }
            Some(EntryEdit::Manifest) => output.write_all(&self.manifest.serialize()?)?,
            Some(EntryEdit::Resources) => {
                let Resources::Loaded(table) = &self.resources else {
                    return Err(invalid("resources", "edited resource table is not loaded"));
                };
                reader::copy_spooled(&table.serialize_spooled()?, output)?;
            }
            Some(EntryEdit::Dex) => {
                return Err(invalid("apk entry", "DEX entries are owned by the session"));
            }
            _ => {
                io::copy(&mut self.archive.by_name(name)?, output)?;
            }
        }
        Ok(true)
    }

    pub(super) fn resources_file(&self) -> Result<File> {
        let Resources::Loaded(table) = &self.resources else {
            return Err(invalid("resources", "edited resource table is not loaded"));
        };
        table.serialize_spooled()
    }
}
