// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::id::ResourceAddress;
use super::{EntryValue, ResourceTable};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;

/// A string-typed entry and the key it is filed under.
#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub res_id: u32,
    pub key_name: String,
}

impl ResourceTable {
    /// Read `string/name` from the global pool.
    ///
    /// Returns `None` for an absent or non-string value. Malformed entries
    /// and invalid string pool indices are errors.
    pub fn string_value(&self, name: &str) -> Result<Option<Cow<'_, str>>> {
        match self.find_resource_id("string", name)? {
            Some(res_id) => self.string_value_by_id(res_id),
            None => Ok(None),
        }
    }

    /// [`Self::string_value`] by resource id.
    pub fn string_value_by_id(&self, res_id: u32) -> Result<Option<Cow<'_, str>>> {
        match self.pool_index(res_id)? {
            Some(index) => self.get_string(index),
            None => Ok(None),
        }
    }

    /// Rewrite the global pool value referenced by `string/name`.
    ///
    /// Returns `false` when the resource is absent or has no string value.
    /// Malformed entries and invalid pool indices are errors. Other resources
    /// sharing the same pool index also see the new text.
    pub fn set_string_value(&mut self, name: &str, value: &str) -> Result<bool> {
        match self.find_resource_id("string", name)? {
            Some(res_id) => self.set_string_value_by_id(res_id, value),
            None => Ok(false),
        }
    }

    /// [`Self::set_string_value`] by resource id.
    pub fn set_string_value_by_id(&mut self, res_id: u32, value: &str) -> Result<bool> {
        let Some(index) = self.pool_index(res_id)? else {
            return Ok(false);
        };
        self.set_string(index, value.to_string())?;
        Ok(true)
    }

    pub fn find_entries_by_string(&self, string_index: u32) -> Result<Vec<ResourceRef>> {
        let mut refs = Vec::new();
        for package in &self.packages {
            for res_type in &package.types {
                for i in 0..res_type.len() {
                    let Some((key, Some(value))) = res_type.entry_head(i)? else {
                        continue;
                    };
                    if value.string_index() == Some(string_index) {
                        refs.push(ResourceRef {
                            res_id: package.resource_id(res_type.id, i)?,
                            key_name: package
                                .key_strings
                                .get(key)?
                                .ok_or_else(|| {
                                    invalid("resource key", format!("invalid index {key}"))
                                })?
                                .into_owned(),
                        });
                    }
                }
            }
        }
        Ok(refs)
    }

    pub fn replace_entry_string(&mut self, res_id: u32, string_index: u32) -> Result<()> {
        self.get_string(string_index)?
            .ok_or_else(|| invalid("resource string", format!("invalid index {string_index}")))?;
        for value in self.values(res_id) {
            value?;
        }
        let entry_index = ResourceAddress::decode(res_id).entry_index;
        for res_type in self.configs_mut(res_id) {
            let Some(mut entry) = res_type.entry(entry_index)? else {
                continue;
            };
            if let EntryValue::Simple(value) = &mut entry.value
                && value.kind == ResValue::STRING
            {
                value.data = string_index;
                res_type.set(entry_index, Some(entry))?;
            }
        }
        Ok(())
    }

    /// The path each configuration of `type_name/entry_name` points at, the
    /// default configuration first. A file resource is a string entry holding
    /// the path aapt packed the file to.
    pub fn file_paths(&self, type_name: &str, entry_name: &str) -> Result<Vec<String>> {
        let location = self.find_entry(type_name, entry_name)?.ok_or_else(|| {
            invalid(
                "resource entry",
                format!("the table has no {type_name}/{entry_name}"),
            )
        })?;
        let mut paths: Vec<(bool, String)> = Vec::new();
        for item in self.values(location.res_id()) {
            let (res_type, value) = item?;
            let path = value
                .string_index()
                .map(|index| self.get_string(index))
                .transpose()?
                .flatten()
                .ok_or_else(|| {
                    invalid(
                        "resource entry",
                        format!(
                            "{type_name}/{entry_name} is not file-backed: its value is of kind {:#04x}, not a string",
                            value.kind
                        ),
                    )
                })?;
            paths.push((res_type.is_default_config(), path.into_owned()));
        }
        if paths.is_empty() {
            return Err(invalid(
                "resource entry",
                format!("{type_name}/{entry_name} is not file-backed: it is a complex entry"),
            ));
        }
        paths.sort_by_key(|(is_default, _)| !is_default);
        Ok(paths.into_iter().map(|(_, path)| path).collect())
    }

    /// A value as the text that parses back to it, or `None` for a kind that
    /// has no such form. Strings read this table's global pool.
    pub fn value_text(&self, value: ResValue) -> Result<Option<String>> {
        let Some(index) = value.string_index() else {
            return Ok(value.to_text());
        };
        self.get_string(index)?
            .map(|text| Some(text.into_owned()))
            .ok_or_else(|| invalid("resource string", format!("invalid index {index}")))
    }

    fn pool_index(&self, res_id: u32) -> Result<Option<u32>> {
        let Some((_, value)) = self.values(res_id).next().transpose()? else {
            return Ok(None);
        };
        let Some(index) = value.string_index() else {
            return Ok(None);
        };
        if self.get_string(index)?.is_none() {
            return Err(invalid(
                "res string",
                format!("resource {res_id:#010x} has invalid pool index {index}"),
            ));
        }
        Ok(Some(index))
    }
}
