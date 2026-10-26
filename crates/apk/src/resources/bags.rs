// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EntryValue, MapEntry, ResourceTable};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;

/// The name aapt gives an array's first element; the rest follow in order.
const ARRAY_FIRST_NAME: u32 = 0x0100_0001;

impl ResourceTable {
    /// The map of the complex entry `type_name/entry_name` in the default
    /// configuration.
    pub fn complex_entries(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<Vec<MapEntry>>> {
        let Some(location) = self.find_entry(type_name, entry_name)? else {
            return Ok(None);
        };
        let Some((_, entry)) = self.default_entry(location.res_id())? else {
            return Ok(None);
        };
        Ok(match entry.value {
            EntryValue::Complex { entries, .. } => Some(entries),
            EntryValue::Simple(_) => None,
        })
    }

    /// The elements of `array/name` in the default configuration as text.
    /// Use [`Self::set_string_array`] to write string-array elements without
    /// interpreting numeric text, booleans or references as typed literals.
    pub fn array(&self, name: &str) -> Result<Vec<String>> {
        let entries = self.complex_entries("array", name)?.ok_or_else(|| {
            invalid(
                "array",
                format!("the table has no array/{name} in the default configuration"),
            )
        })?;
        entries
            .iter()
            .map(|entry| {
                self.value_text(entry.value)?.ok_or_else(|| {
                    invalid(
                        "array",
                        format!(
                            "array/{name} holds a value of kind {:#04x}, which has no text form",
                            entry.value.kind
                        ),
                    )
                })
            })
            .collect()
    }

    /// Replaces an array with literal strings in every configuration.
    pub fn set_string_array(&mut self, name: &str, values: &[String]) -> Result<u32> {
        let values: Vec<ResValue> = values
            .iter()
            .map(|value| ResValue::string(self.add_global_string(value)))
            .collect();
        self.set_array_values(name, &values)
    }

    /// Reads array scalars without converting their kinds or pool indices to text.
    /// String values refer to this table's global string pool. An absent array,
    /// a non-bag entry, or malformed entry is an error.
    pub fn array_values(&self, name: &str) -> Result<Vec<ResValue>> {
        let location = self
            .find_entry("array", name)?
            .ok_or_else(|| invalid("array", format!("the table has no array/{name}")))?;
        let (_, entry) = self.default_entry(location.res_id())?.ok_or_else(|| {
            invalid(
                "array",
                format!("array/{name} has no default configuration"),
            )
        })?;
        match entry.value {
            EntryValue::Complex { entries, .. } => {
                Ok(entries.into_iter().map(|entry| entry.value).collect())
            }
            EntryValue::Simple(_) => Err(invalid("array", format!("{name} is not an array"))),
        }
    }

    /// Replaces array scalars in every configuration without interpreting them.
    /// String indices must belong to this table's global pool. The array must
    /// already exist; its resource ID and configuration coverage are retained.
    pub fn set_array_values(&mut self, name: &str, values: &[ResValue]) -> Result<u32> {
        let count = u32::try_from(values.len())
            .map_err(|_| invalid("array", "array exceeds the map name range"))?;
        if ARRAY_FIRST_NAME
            .checked_add(count.saturating_sub(1))
            .is_none()
        {
            return Err(invalid("array", "array index exceeds the map name range"));
        }
        for index in values.iter().filter_map(|value| value.string_index()) {
            self.get_string(index)?
                .ok_or_else(|| invalid("array", format!("invalid string index {index}")))?;
        }
        let entries: Vec<MapEntry> = values
            .iter()
            .enumerate()
            .map(|(i, &value)| MapEntry {
                name: ARRAY_FIRST_NAME + i as u32,
                value,
            })
            .collect();
        match self.edit_complex_entries("array", name, |_, existing| {
            existing.clone_from(&entries);
        })? {
            Some((id, edited)) if edited > 0 => Ok(id),
            Some(_) => Err(invalid(
                "array",
                format!("{name} is in the table but is not an array"),
            )),
            None => Err(invalid("array", format!("the table has no array/{name}"))),
        }
    }

    /// Edits the parent and map of `type_name/entry_name` in every
    /// configuration where it is complex, after checking every configuration
    /// reads. Returns its id and how many configurations were edited.
    pub(super) fn edit_complex_entries(
        &mut self,
        type_name: &str,
        entry_name: &str,
        mut edit: impl FnMut(&mut u32, &mut Vec<MapEntry>),
    ) -> Result<Option<(u32, usize)>> {
        let Some(location) = self.find_entry(type_name, entry_name)? else {
            return Ok(None);
        };
        let res_id = location.res_id();
        for (_, res_type) in self.configs(res_id) {
            res_type.entry_head(location.entry_index)?;
        }
        let mut edited = 0;
        for res_type in self.configs_mut(res_id) {
            let Some(mut entry) = res_type.entry(location.entry_index)? else {
                continue;
            };
            let EntryValue::Complex { parent, entries } = &mut entry.value else {
                continue;
            };
            edit(parent, entries);
            res_type.set(location.entry_index, Some(entry))?;
            edited += 1;
        }
        Ok(Some((res_id, edited)))
    }
}
