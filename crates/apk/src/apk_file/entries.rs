// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{ApkFile, ComponentIndex, Compression, DexOrigin, DexSource};
use crate::entry::dex_ordinal;
use crate::error::{Result, invalid};
use crate::zip::reader;
use std::io::Write;

impl ApkFile {
    /// Adds or replaces a component entry. Format entries are parsed before
    /// committing a replacement; resource entry bodies stay deferred.
    /// A replacement invalidates the previous parsed document. Native libraries
    /// are stored regardless of the requested compression.
    pub fn inject_file(
        &mut self,
        component: usize,
        name: &str,
        data: Vec<u8>,
        compression: Compression,
    ) -> Result<()> {
        let file = reader::spool_file(&mut data.as_slice())?;
        drop(data);
        self.inject_file_spooled(component, name, file, compression)
    }

    /// Takes ownership of a completed file as an entry replacement without
    /// allocating its payload. All format-entry validation is the same as
    /// `inject_file`. The file and any other handles to it must remain immutable
    /// after this call; the session may retain read-only mappings of it.
    pub fn inject_file_spooled(
        &mut self,
        component: usize,
        name: &str,
        file: std::fs::File,
        compression: Compression,
    ) -> Result<()> {
        if component >= self.components.len() {
            return Err(invalid("apk", format!("no component at index {component}")));
        }
        let manifest = (name == crate::entry::MANIFEST_ENTRY)
            .then(|| crate::AxmlDocument::parse(&reader::map_spooled(&file)?))
            .transpose()
            .map_err(|error| error.in_entry(name))?;
        let dex = if dex_ordinal(name).is_some() {
            Some(
                crate::dex::parse_entry(reader::map_spooled(&file)?, self.options)
                    .map_err(|error| error.in_entry(name))?,
            )
        } else {
            None
        };
        self.components[component]
            .stage(name, file, manifest, compression)
            .map_err(|error| error.in_entry(name))?;
        if let Some(members) = dex {
            let previous: Vec<_> = self.dex_entries(component, name).collect();
            let count = members.len();
            for (ordinal, dex) in members.into_iter().enumerate() {
                if let Some(&index) = previous.get(ordinal) {
                    self.dex.dex_files[index] = dex;
                    self.dex_origins[index].kind = DexSource::Archive;
                } else {
                    self.dex.add_dex(dex);
                    self.dex_origins.push(DexOrigin {
                        component: ComponentIndex(component),
                        name: name.into(),
                        kind: DexSource::Archive,
                    });
                }
            }
            for index in previous.into_iter().skip(count) {
                self.remove_dex_slot(index);
            }
        }
        Ok(())
    }

    /// Maps the current entry, returning `None` when absent. Stored entries
    /// in the input APK are mapped directly; compressed or parsed entries are
    /// streamed to disk first. Mappings stay valid across later staged edits.
    pub fn map_component_entry(
        &mut self,
        component: usize,
        name: &str,
    ) -> Result<Option<reseam_storage::MappedFile>> {
        let changed = self.dex_entries(component, name).any(|index| {
            self.dex.dex_files[index].is_dirty() || self.dex_origins[index].kind == DexSource::Added
        });
        if changed {
            let mut file = reseam_storage::temporary_file()?;
            if !self.copy_entry(component, name, &mut file)? {
                return Ok(None);
            }
            return Ok(Some(reader::map_spooled(&file)?));
        }
        self.components
            .get_mut(component)
            .ok_or_else(|| invalid("apk", format!("no component at index {component}")))?
            .map_entry(name)
            .map_err(|error| error.in_entry(name))
    }

    /// Deletes an entry and its parsed state. The required manifest cannot be
    /// deleted. Removed DEX slots become empty tombstones so other indices keep
    /// their identity; mutable access to a tombstone returns `None`.
    pub fn delete_file(&mut self, component: usize, name: &str) -> Result<()> {
        self.components
            .get_mut(component)
            .ok_or_else(|| invalid("apk", format!("no component at index {component}")))?
            .delete(name)?;
        let indices: Vec<_> = self.dex_entries(component, name).collect();
        for index in indices {
            self.remove_dex_slot(index);
        }
        Ok(())
    }

    /// Streams the current entry to `output`, including parsed edits and DEX
    /// mutations. Returns `false` when absent. IO and serialization errors retain
    /// the entry name. This does not allocate an entry-sized buffer.
    /// Edited DEX containers retain every member in physical-entry order.
    pub fn copy_entry(
        &mut self,
        component: usize,
        name: &str,
        output: &mut impl Write,
    ) -> Result<bool> {
        self.copy_current(component, name, output)
            .map_err(|error| error.in_entry(name))
    }

    fn copy_current(
        &mut self,
        component: usize,
        name: &str,
        output: &mut impl Write,
    ) -> Result<bool> {
        let entry = self
            .components
            .get(component)
            .ok_or_else(|| invalid("apk", format!("no component at index {component}")))?;
        if !entry.contains(name) {
            return Ok(false);
        }
        let indices: Vec<_> = self
            .dex_entries(component, name)
            .filter(|&index| self.dex_origins[index].kind != DexSource::Removed)
            .collect();
        let changed = indices.iter().any(|&index| {
            self.dex.dex_files[index].is_dirty() || self.dex_origins[index].kind == DexSource::Added
        });
        if changed {
            let file = match indices.as_slice() {
                [index] => reseam_dex::write_spooled(&self.dex.dex_files[*index], None)?,
                _ => reseam_dex::write_container_spooled(
                    indices
                        .iter()
                        .map(|&index| (&self.dex.dex_files[index], None)),
                )?,
            };
            std::io::copy(&mut file.reader(), output)?;
            return Ok(true);
        }
        self.components[component].copy_entry(name, output)
    }

    /// Allocates the current, uncompressed entry bytes. Use `copy_entry` for
    /// large payloads. Returns `None` when this component has no such entry.
    pub fn read_component_entry(
        &mut self,
        component: usize,
        name: &str,
    ) -> Result<Option<Vec<u8>>> {
        let mut bytes = Vec::new();
        Ok(self
            .copy_entry(component, name, &mut bytes)?
            .then_some(bytes))
    }

    /// Reads the entry from the first component that contains it, base first.
    /// Like `read_component_entry`, this allocates the whole payload.
    pub fn read_entry(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(component) = self
            .components
            .iter()
            .position(|component| component.contains(name))
        else {
            return Ok(None);
        };
        self.read_component_entry(component, name)
    }

    fn dex_entries<'a>(
        &'a self,
        component: usize,
        name: &'a str,
    ) -> impl Iterator<Item = usize> + 'a {
        self.dex_origins
            .iter()
            .enumerate()
            .filter_map(move |(index, origin)| {
                (origin.component == ComponentIndex(component) && origin.name.as_str() == name)
                    .then_some(index)
            })
    }

    fn remove_dex_slot(&mut self, index: usize) {
        let header = self.dex.dex_files[index].header().clone();
        self.dex.dex_files[index] = reseam_dex::DexFile::new(header);
        self.dex_origins[index].kind = DexSource::Removed;
    }
}
