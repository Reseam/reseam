// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::{Deref, DerefMut};

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
            self.table.find_resource_id(type_name, entry_name),
            &mut self.splits,
        ) {
            (Some(id), _) => Ok(Some(id)),
            (None, Some(splits)) => splits(type_name, entry_name),
            (None, None) => Ok(None),
        }
    }

    /// Reads `text` the way a value of `attr` is read, interning plain text
    /// into the global pool the way a resource entry holds it.
    pub(crate) fn parse_value(&mut self, text: &str, attr: Option<u32>) -> Result<ResValue> {
        Ok(match axml::parse_attribute_value(text, attr, Some(self))? {
            axml::AttributeValue::Value(value) => value,
            axml::AttributeValue::Text => ResValue::string(self.add_global_string(text)),
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

impl Deref for ResourceScope<'_> {
    type Target = ResourceTable;

    fn deref(&self) -> &ResourceTable {
        self.table
    }
}

impl DerefMut for ResourceScope<'_> {
    fn deref_mut(&mut self) -> &mut ResourceTable {
        self.table
    }
}
