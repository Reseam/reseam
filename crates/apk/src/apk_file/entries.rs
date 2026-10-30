// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::component::map_staged;
use super::{ApkFile, ComponentIndex, EntryReader, dex_set};
use crate::entry::dex_ordinal;
use crate::error::{ApkError, Result, invalid};
use crate::zip::Compression;
use std::io::Read;

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
        let file = reseam_storage::spool(|out| out.write_all(&data))?;
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
            .then(|| crate::AxmlDocument::parse(&map_staged(&file)?))
            .transpose()
            .map_err(|error| error.in_entry(name))?;
        let dex = dex_ordinal(name)
            .map(|_| dex_set::parse_entry(map_staged(&file)?, self.options))
            .transpose()
            .map_err(|error| error.in_entry(name))?;
        self.components[component]
            .stage(name, file, manifest, compression)
            .map_err(|error| error.in_entry(name))?;
        if let Some(members) = dex {
            self.replace_dex_entry(ComponentIndex(component), name, members);
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
        if self
            .components
            .get(component)
            .is_some_and(|entry| entry.contains(name))
            && let Some(spooled) = self
                .rewritten_dex_entry(ComponentIndex(component), name)
                .map_err(|error| error.in_entry(name))?
        {
            // SAFETY: the serialized DEX is a temporary file nothing else holds.
            return Ok(Some(unsafe {
                reseam_storage::map_file(&spooled.into_file())?
            }));
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
        self.remove_dex_entry(ComponentIndex(component), name);
        Ok(())
    }

    /// Opens the current entry, including parsed edits and DEX mutations,
    /// without loading it. Returns `None` when absent. Errors retain the entry
    /// name, except read errors, which the caller sees as they come. Edited DEX
    /// containers retain every member in physical-entry order.
    pub fn open_component_entry(
        &mut self,
        component: usize,
        name: &str,
    ) -> Result<Option<EntryReader>> {
        self.open_current(component, name)
            .map_err(|error| error.in_entry(name))
    }

    fn open_current(&mut self, component: usize, name: &str) -> Result<Option<EntryReader>> {
        let entry = self
            .components
            .get(component)
            .ok_or_else(|| invalid("apk", format!("no component at index {component}")))?;
        if !entry.contains(name) {
            return Ok(None);
        }
        if let Some(spooled) = self.rewritten_dex_entry(ComponentIndex(component), name)? {
            return Ok(Some(EntryReader::file(spooled.into_file())?));
        }
        self.components[component].open_entry(name)
    }

    /// Allocates the current, uncompressed entry bytes. Use
    /// `open_component_entry` for large payloads. Returns `None` when this
    /// component has no such entry.
    pub fn read_component_entry(
        &mut self,
        component: usize,
        name: &str,
    ) -> Result<Option<Vec<u8>>> {
        let Some(mut entry) = self.open_component_entry(component, name)? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| ApkError::from(error).in_entry(name))?;
        Ok(Some(bytes))
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
}
