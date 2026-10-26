// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EntryValue, MapEntry, ResourceTable};
use crate::axml;
use crate::error::{Result, invalid};
use crate::res_value::ResValue;

/// Finds `type/name` in the app's other resource tables.
pub type SplitLookup<'a> = dyn FnMut(&str, &str) -> Result<Option<u32>> + 'a;

/// A table being edited, resolving `@type/name` across the whole app. The
/// platform merges every split's table under one package, so a name this
/// table lacks may be one only a configuration split defines. New entries
/// always land in this table.
pub struct ResourceScope<'a> {
    table: &'a mut ResourceTable,
    splits: Option<&'a mut SplitLookup<'a>>,
}

impl<'a> ResourceScope<'a> {
    pub fn new(table: &'a mut ResourceTable, splits: &'a mut SplitLookup<'a>) -> Self {
        Self {
            table,
            splits: Some(splits),
        }
    }

    pub fn resource_id(&mut self, type_name: &str, entry_name: &str) -> Result<Option<u32>> {
        match (
            self.table.find_resource_id(type_name, entry_name)?,
            &mut self.splits,
        ) {
            (Some(id), _) => Ok(Some(id)),
            (None, Some(splits)) => splits(type_name, entry_name),
            (None, None) => Ok(None),
        }
    }

    /// Finds an existing application-wide ID before creating it in this table.
    /// Other split tables are loaded only when the local lookup misses.
    pub fn ensure_id(&mut self, name: &str) -> Result<Option<u32>> {
        match self.resource_id("id", name)? {
            Some(id) => Ok(Some(id)),
            None => self.table.ensure_id(name),
        }
    }

    /// The edited component's table. Attribute definitions are configuration
    /// independent and deliberately resolve against this table only.
    pub fn table(&self) -> &ResourceTable {
        self.table
    }

    /// Edits entries in this component; application-wide name lookups use
    /// [`Self::resource_id`] and [`Self::ensure_id`].
    pub fn table_mut(&mut self) -> &mut ResourceTable {
        self.table
    }

    pub(crate) fn parse_value(&mut self, text: &str, attr: Option<u32>) -> Result<ResValue> {
        Ok(match axml::parse_value(text, attr, Some(self))? {
            axml::AttributeValue::Value(value) => value,
            axml::AttributeValue::Text => ResValue::string(self.table.add_global_string(text)),
        })
    }
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

impl<'a> From<&'a mut ResourceTable> for ResourceScope<'a> {
    fn from(table: &'a mut ResourceTable) -> Self {
        Self {
            table,
            splits: None,
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
