// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::ResValue;
use reseam_apk::axml::android_res_name;
use reseam_apk::resources::{
    EntryValue, ResEntry, ResourceName, ResourceTable, StringEntries, StringUse, qualifiers,
};
use reseam_model::analysis::{ConfigValue, ResourceId, ResourceView};

use super::Analysis;
use crate::error::{HostError, Result};

impl Analysis {
    /// The resource table of every component, the base first: splits hold
    /// the configurations, such as languages, the base leaves out.
    pub(super) fn tables(&self) -> impl Iterator<Item = &ResourceTable> {
        self.opened
            .apk
            .components()
            .iter()
            .filter_map(|component| component.loaded_resources())
    }

    /// Visits every string of every table's global pool with the entries that
    /// hold it, in each configuration.
    pub(super) fn for_each_string(
        &self,
        mut visit: impl FnMut(&ResourceTable, &str, &[StringUse]) -> Result<()>,
    ) -> Result<()> {
        for (table, entries) in self.tables().zip(self.string_entries()?) {
            for (string, value) in (0u32..).zip(table.global_strings().iter()) {
                let value = value.map_err(|error| self.opened.error(error))?;
                visit(table, &value, entries.get(string))?;
            }
        }
        Ok(())
    }

    /// The string index of each table, built on first use.
    fn string_entries(&self) -> Result<&[StringEntries]> {
        if let Some(entries) = self.string_entries.get() {
            return Ok(entries);
        }
        let entries = self
            .tables()
            .map(ResourceTable::string_entries)
            .collect::<reseam_apk::Result<Vec<_>>>()
            .map_err(|error| self.opened.error(error))?;
        Ok(self.string_entries.get_or_init(|| entries))
    }

    /// `id` with the name the first table that defines it gives it.
    pub(super) fn resource_id(&self, id: u32) -> Result<ResourceId> {
        Ok(ResourceId {
            id,
            name: self.resource_name(id)?.map(|name| name.to_string()),
        })
    }

    /// The framework's name for a framework id; the app's from the first
    /// table that names it otherwise.
    pub(super) fn resource_name(&self, id: u32) -> Result<Option<ResourceName>> {
        if id >> 24 == 0x01 {
            return Ok(android_res_name(id));
        }
        for table in self.tables() {
            if let Some(name) = table
                .resource_name(id)
                .map_err(|error| self.opened.error(error))?
            {
                return Ok(Some(name));
            }
        }
        Ok(None)
    }

    /// The id of a resource named as resource XML names it: `@string/title`.
    pub fn find_resource(&self, name: &str) -> Result<Option<u32>> {
        let Some((type_name, entry)) = name.strip_prefix('@').and_then(|name| name.split_once('/'))
        else {
            return Ok(None);
        };
        for table in self.tables() {
            if let Some(id) = table
                .find_resource_id(type_name, entry)
                .map_err(|error| self.opened.error(error))?
            {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// The resource's value in every configuration of every component.
    pub fn show_resource(&self, id: u32) -> Result<ResourceView> {
        let entry_index = (id & 0xffff) as usize;
        let mut values = Vec::new();
        for table in self.tables() {
            for package in table.packages() {
                for res_type in package.types() {
                    if package.resource_id(res_type.id(), entry_index).ok() != Some(id) {
                        continue;
                    }
                    let entry = res_type
                        .entry(entry_index)
                        .map_err(|error| self.opened.error(error))?;
                    if let Some(entry) = entry {
                        values.push(ConfigValue {
                            qualifiers: qualifiers(res_type.config()),
                            value: self.entry_text(table, &entry)?,
                        });
                    }
                }
            }
        }
        if values.is_empty() {
            return Err(HostError::UnknownResource(id));
        }
        Ok(ResourceView {
            resource: self.resource_id(id)?,
            values,
        })
    }

    /// A simple entry's value, or a style's or array's items as `name = value` lines.
    fn entry_text(&self, table: &ResourceTable, entry: &ResEntry) -> Result<String> {
        match &entry.value {
            EntryValue::Simple(value) => self.value_text(table, *value),
            EntryValue::Complex { parent, entries } => {
                let mut lines = Vec::new();
                if *parent != 0 {
                    lines.push(format!("parent = {}", self.reference('@', *parent)?));
                }
                for item in entries {
                    lines.push(format!(
                        "{} = {}",
                        self.reference('@', item.name)?,
                        self.value_text(table, item.value)?
                    ));
                }
                Ok(lines.join("\n"))
            }
        }
    }

    /// A value as resource XML writes it, references named.
    pub(super) fn value_text(&self, table: &ResourceTable, value: ResValue) -> Result<String> {
        Ok(match value.kind {
            ResValue::REFERENCE if value.data != 0 => self.reference('@', value.data)?,
            ResValue::ATTRIBUTE => self.reference('?', value.data)?,
            _ => table
                .value_text(value)
                .map_err(|error| self.opened.error(error))?
                .unwrap_or_else(|| format!("0x{:08x}", value.data)),
        })
    }

    fn reference(&self, sigil: char, id: u32) -> Result<String> {
        Ok(match self.resource_name(id)? {
            Some(name) => name.reference(sigil),
            None => format!("{sigil}0x{id:08x}"),
        })
    }
}
