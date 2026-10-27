// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::component::map_staged;
use super::{ApkFile, ComponentIndex, dex_set};
use crate::entry::dex_ordinal;
use crate::error::{Result, invalid};
use crate::zip::Compression;
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
        if let Some(spooled) = self.rewritten_dex_entry(ComponentIndex(component), name)? {
            std::io::copy(&mut spooled.reader(), output)?;
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
}
