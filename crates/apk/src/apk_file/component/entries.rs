// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Write};
use std::sync::Arc;

use reseam_storage::Bytes;

use super::{ApkComponent, EntryEdit, Resources, map_staged};
use crate::axml::AxmlDocument;
use crate::entry::{EntryName, MANIFEST_ENTRY, RESOURCES_ENTRY, dex_ordinal, is_native_library};
use crate::error::{Result, invalid};
use crate::resources::ResourceTable;
use crate::zip::Compression;
use crate::zip::reader::{self, Archive};

impl ApkComponent {
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

    pub(in crate::apk_file) fn stage(
        &mut self,
        path: &str,
        file: File,
        manifest: Option<AxmlDocument>,
        compression: Compression,
    ) -> Result<()> {
        let resources = if path == RESOURCES_ENTRY {
            Some(ResourceTable::parse(Bytes::from_mmap(Arc::new(
                map_staged(&file)?,
            )))?)
        } else {
            None
        };
        if let Some(manifest) = manifest {
            self.keep_manifest();
            self.manifest = manifest;
            self.manifest_revision += 1;
        }
        if let Some(resources) = resources {
            self.keep_resources();
            self.resources = Resources::Loaded(Box::new(resources));
        }
        let compression = if is_native_library(path) {
            Compression::Stored
        } else {
            compression
        };
        self.set_edit(path.into(), EntryEdit::Staged { file, compression });
        Ok(())
    }

    pub(in crate::apk_file) fn delete(&mut self, path: &str) -> Result<()> {
        if path == MANIFEST_ENTRY {
            return Err(invalid(
                "apk entry",
                "AndroidManifest.xml is required and cannot be deleted",
            ));
        }
        if path == RESOURCES_ENTRY {
            self.keep_resources();
            self.resources = Resources::Absent;
        }
        self.set_edit(path.into(), EntryEdit::Deleted);
        Ok(())
    }

    pub(in crate::apk_file) fn add_dex_entry(&mut self, name: &str) {
        self.set_edit(name.into(), EntryEdit::Dex);
    }

    /// How the DEX entry `name` is stored: as the archive holds it, or for a
    /// new entry, as the component's first DEX is held.
    pub(in crate::apk_file) fn dex_compression(&self, name: &str) -> Result<Compression> {
        let source = if reader::contains(&self.archive, name) {
            Some(name.to_owned())
        } else {
            reader::dex_entry_names(&self.archive).into_iter().next()
        };
        let stored = match source {
            Some(source) => reader::is_stored(&self.archive, &source)?,
            None => false,
        };
        Ok(if stored {
            Compression::Stored
        } else {
            Compression::Deflated
        })
    }

    pub(crate) fn archive(&self) -> &Archive {
        &self.archive
    }

    pub(in crate::apk_file) fn edits(&self) -> &BTreeMap<EntryName, EntryEdit> {
        &self.edits
    }

    /// The DEX entry names the archive or the session already claims.
    pub(in crate::apk_file) fn reserved_dex_names(&self) -> impl Iterator<Item = &str> {
        self.archive
            .file_names()
            .chain(self.edits.keys().map(EntryName::as_str))
            .filter(|name| dex_ordinal(name).is_some())
    }

    pub(in crate::apk_file) fn copy_entry(
        &mut self,
        name: &str,
        output: &mut impl Write,
    ) -> Result<bool> {
        if !self.contains(name) {
            return Ok(false);
        }
        match self.edits.get(name) {
            Some(EntryEdit::Staged { file, .. }) => reseam_storage::copy_file(file, output)?,
            Some(EntryEdit::Manifest) => output.write_all(&self.manifest.serialize()?)?,
            Some(EntryEdit::Resources) => {
                reseam_storage::copy_file(&self.resources_file()?, output)?;
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

    pub(in crate::apk_file) fn map_entry(
        &mut self,
        name: &str,
    ) -> Result<Option<reseam_storage::MappedFile>> {
        match self.edits.get(name) {
            Some(EntryEdit::Staged { file, .. }) => Ok(Some(map_staged(file)?)),
            Some(EntryEdit::Manifest | EntryEdit::Resources | EntryEdit::Dex) => {
                Ok(Some(reseam_storage::spool_mapped(|mut out| {
                    self.copy_entry(name, &mut out).map(drop)
                })?))
            }
            None if reader::contains(&self.archive, name) => {
                Ok(Some(reader::map_entry(&mut self.archive, name)?))
            }
            Some(EntryEdit::Deleted) | None => Ok(None),
        }
    }
}
