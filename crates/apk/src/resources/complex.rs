// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{EntryValue, MapEntry, ResourceScope, ResourceTable};
use crate::error::{Result, invalid};
use crate::value::ResValue;

const ARRAY_FIRST_NAME: u32 = 0x0100_0001;

impl ResourceScope<'_> {
    /// Adds or replaces `items` in every configuration of `style/name`. A style
    /// the table does not have is created in the default configuration, which
    /// needs `parent`; given for a style that exists, `parent` replaces its own.
    ///
    /// Item names resolve as `<style>` item names do, `android:name` through the
    /// framework table and an unprefixed name through the app's `attr` entries,
    /// and values as attribute values do.
    pub fn set_style_items(
        &mut self,
        name: &str,
        parent: Option<&str>,
        items: &[(String, String)],
    ) -> Result<u32> {
        let names = items
            .iter()
            .map(|(item, _)| {
                self.table().attr_id(item)?.ok_or_else(|| {
                    invalid(
                        "style item",
                        format!(
                            "style/{name}: no attr resource is named {item}, and an item the framework cannot resolve is ignored"
                        ),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let values = items
            .iter()
            .zip(&names)
            .map(|((_, value), &attr)| self.parse_value(value, Some(attr)))
            .collect::<Result<Vec<_>>>()?;
        let parent = parent.map(|parent| self.reference(parent)).transpose()?;

        let added: Vec<MapEntry> = names
            .iter()
            .zip(&values)
            .map(|(&name, &value)| MapEntry { name, value })
            .collect();
        match self
            .table_mut()
            .edit_complex_entries("style", name, |style_parent, entries| {
                if let Some(parent) = parent {
                    *style_parent = parent;
                }
                merge(entries, &added);
            })? {
            Some((id, edited)) if edited > 0 => Ok(id),
            Some(_) => Err(invalid(
                "style",
                format!("{name} is in the table but is not a style"),
            )),
            None => {
                let parent = parent.ok_or_else(|| {
                    invalid(
                        "style",
                        format!("the table has no style/{name}; pass a parent to create it"),
                    )
                })?;
                let mut entries = Vec::new();
                merge(&mut entries, &added);
                self.table_mut()
                    .set_entry_in("style", name, EntryValue::Complex { parent, entries }, "")?
                    .ok_or_else(|| invalid("style", format!("could not add style/{name}")))
            }
        }
    }

    /// Replaces the elements of `array/name` in every configuration that
    /// defines it. The count may change: element names are the positions aapt
    /// writes, so they are renumbered from the start.
    pub fn set_array(&mut self, name: &str, values: &[String]) -> Result<u32> {
        let values = values
            .iter()
            .map(|value| self.parse_value(value, None))
            .collect::<Result<Vec<_>>>()?;
        self.table_mut().set_array_values(name, &values)
    }

    fn reference(&mut self, text: &str) -> Result<u32> {
        match self.parse_value(text, None)? {
            value if value.kind == ResValue::REFERENCE => Ok(value.data),
            _ => Err(invalid(
                "resource reference",
                format!("{text} does not name a resource"),
            )),
        }
    }
}

impl ResourceTable {
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
        for value in values.iter().filter(|value| value.kind == ResValue::STRING) {
            self.get_string(value.data)?
                .ok_or_else(|| invalid("array", format!("invalid string index {}", value.data)))?;
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
}

fn merge(entries: &mut Vec<MapEntry>, added: &[MapEntry]) {
    for item in added {
        match entries.iter_mut().find(|entry| entry.name == item.name) {
            Some(existing) => existing.value = item.value,
            None => entries.push(item.clone()),
        }
    }
    entries.sort_by_key(|entry| entry.name);
}
