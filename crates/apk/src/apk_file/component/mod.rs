// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_dex::Loading;
use reseam_storage::{Bytes, MappedFile};

use crate::axml::AxmlDocument;
use crate::entry::{EntryName, MANIFEST_ENTRY, RESOURCES_ENTRY};
use crate::error::{Result, invalid};
use crate::resources::ResourceTable;
use crate::zip::Compression;
use crate::zip::reader::{self, Archive};

mod entries;
mod undo;

/// How an entry differs from the archive the component was opened from.
pub(in crate::apk_file) enum EntryEdit {
    Staged {
        file: File,
        compression: Compression,
    },
    Manifest,
    Resources,
    Dex,
    Deleted,
}

pub struct ApkComponent {
    name: String,
    path: PathBuf,
    archive: Archive,
    manifest: AxmlDocument,
    manifest_revision: u64,
    resources: Resources,
    edits: BTreeMap<EntryName, EntryEdit>,
    journal: reseam_journal::Journal<undo::Part, undo::Undo>,
}

#[derive(Clone)]
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
            manifest_revision: 0,
            resources,
            edits: BTreeMap::new(),
            journal: reseam_journal::Journal::default(),
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
    pub fn source(&self) -> Result<MappedFile> {
        reader::map_file(&self.archive)
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        self.keep_manifest();
        self.set_edit(MANIFEST_ENTRY.into(), EntryEdit::Manifest);
        self.manifest_revision += 1;
        &mut self.manifest
    }

    /// Advances each time the manifest is granted for mutation, so a caller can
    /// tell whether anything edited it between two readings.
    pub fn manifest_revision(&self) -> u64 {
        self.manifest_revision
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
        if !matches!(self.resources, Resources::Loaded(_)) {
            return Ok(None);
        }
        self.keep_resources();
        self.set_edit(RESOURCES_ENTRY.into(), EntryEdit::Resources);
        let Resources::Loaded(table) = &mut self.resources else {
            unreachable!("the table was loaded above");
        };
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

    /// The edited resource table, serialized to a temporary file.
    pub(in crate::apk_file) fn resources_file(&self) -> Result<File> {
        let Resources::Loaded(table) = &self.resources else {
            return Err(invalid("resources", "edited resource table is not loaded"));
        };
        reseam_storage::spool(|out| table.write_to(out))
    }
}

/// Maps a staged entry file. Staging takes a file on the promise that nothing
/// writes to it afterwards, which is what a mapping needs.
pub(in crate::apk_file) fn map_staged(file: &File) -> Result<MappedFile> {
    // SAFETY: staged files are immutable once handed to the session.
    Ok(unsafe { reseam_storage::map_file(file)? })
}
