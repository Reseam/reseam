// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
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
        items: &[StyleItem],
    ) -> Result<u32> {
        let edit = self.style_edit(name, parent, items)?;
        let table = self.table_mut();
        match table.edit_complex_entries("style", name, |style_parent, entries| {
            if let Some(parent) = edit.parent {
                *style_parent = parent;
            }
            merge(entries, &edit.items);
        })? {
            Some((id, edited)) if edited > 0 => Ok(id),
            Some(_) => Err(invalid(
                "style",
                format!("{name} is in the table but is not a style"),
            )),
            None => edit.write(table, name, None, ""),
        }
    }

    /// [`Self::set_style_items`] for the one configuration `qualifiers` names,
    /// such as `night`. A configuration without the style starts from the
    /// default configuration's parent and items, since the loader takes a
    /// configuration's style in place of the default one.
    pub fn set_style_items_in(
        &mut self,
        name: &str,
        parent: Option<&str>,
        items: &[StyleItem],
        qualifiers: &str,
    ) -> Result<u32> {
        let edit = self.style_edit(name, parent, items)?;
        let table = self.table_mut();
        let base = match table.complex_entry_in("style", name, qualifiers)? {
            Some(found) => Some(found),
            None => table.complex_entry_in("style", name, "")?,
        };
        edit.write(table, name, base, qualifiers)
    }

    fn style_edit(
        &mut self,
        name: &str,
        parent: Option<&str>,
        items: &[StyleItem],
    ) -> Result<StyleEdit> {
        let attributes = items
            .iter()
            .map(|item| {
                self.table().attr_id(&item.attribute)?.ok_or_else(|| {
                    invalid(
                        "style item",
                        format!(
                            "style/{name}: no attr resource is named {}, and an item the framework cannot resolve is ignored",
                            item.attribute
                        ),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let items = items
            .iter()
            .zip(attributes)
            .map(|(item, attribute)| {
                Ok(MapEntry {
                    name: attribute,
                    value: self.parse_value(&item.value, Some(attribute))?,
                })
            })
            .collect::<Result<_>>()?;
        let parent = parent.map(|parent| self.reference(parent)).transpose()?;
        Ok(StyleEdit { parent, items })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleItem {
    pub attribute: String,
    pub value: String,
}

struct StyleEdit {
    parent: Option<u32>,
    items: Vec<MapEntry>,
}

impl StyleEdit {
    fn write(
        self,
        table: &mut ResourceTable,
        name: &str,
        base: Option<(u32, Vec<MapEntry>)>,
        qualifiers: &str,
    ) -> Result<u32> {
        let (parent, mut entries) = match (base, self.parent) {
            (Some((existing, entries)), parent) => (parent.unwrap_or(existing), entries),
            (None, Some(parent)) => (parent, Vec::new()),
            (None, None) => {
                return Err(invalid(
                    "style",
                    format!("the table has no style/{name}; pass a parent to create it"),
                ));
            }
        };
        merge(&mut entries, &self.items);
        table.add_entry_in(
            "style",
            name,
            EntryValue::Complex { parent, entries },
            qualifiers,
        )
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
