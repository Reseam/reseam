// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ResourceTable;
use crate::axml;
use crate::error::Result;
use crate::value::ResValue;

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
        let value = match attr {
            Some(attr) => axml::parse_attribute_value(text, attr, Some(self))?,
            None => axml::infer_value(text, Some(self))?,
        };
        Ok(match value {
            axml::AttributeValue::Value(value) => value,
            axml::AttributeValue::Text => ResValue::string(self.table.add_global_string(text)),
        })
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
