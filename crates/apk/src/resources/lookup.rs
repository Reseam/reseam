// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;
use std::fmt;

use super::config::{config_for_qualifiers, same_config};
use super::id::{EntryLocation, FRAMEWORK_PACKAGE, ResourceAddress};
use super::{ResEntry, ResPackage, ResType, ResourceTable};
use crate::axml::android_res_name;
use crate::error::{Result, invalid};
use crate::res_value::ResValue;
use crate::string_pool::StringPool;

/// A resource's name as resource XML refers to it: `@string/app_name`, or
/// `@android:string/ok` for one of the framework's.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceName {
    /// `None` for the app's own resources.
    pub package: Option<String>,
    pub type_name: String,
    pub entry: String,
}

impl ResourceName {
    /// The reference resource XML writes: `@` for a resource, `?` for a theme
    /// attribute, as in `?android:attr/textColorPrimary`.
    pub fn reference(&self, sigil: char) -> String {
        match &self.package {
            Some(package) => format!("{sigil}{package}:{}/{}", self.type_name, self.entry),
            None => format!("{sigil}{}/{}", self.type_name, self.entry),
        }
    }
}

impl fmt::Display for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reference('@'))
    }
}

/// The first value found. Errors only surface when no item finds one, so a
/// malformed candidate does not hide a valid match elsewhere.
pub(crate) fn first_found<T>(
    items: impl IntoIterator<Item = Result<Option<T>>>,
) -> Result<Option<T>> {
    let mut first_error = None;
    for item in items {
        match item {
            Ok(Some(value)) => return Ok(Some(value)),
            Err(error) => {
                first_error.get_or_insert(error);
            }
            Ok(None) => {}
        }
    }
    first_error.map_or(Ok(None), Err)
}

pub(crate) fn resource_name_of(
    table: Option<&ResourceTable>,
    id: u32,
) -> Result<Option<ResourceName>> {
    match table {
        _ if ResourceAddress::decode(id).package_id == FRAMEWORK_PACKAGE => {
            Ok(android_res_name(id))
        }
        Some(table) => table.resource_name(id),
        None => Ok(None),
    }
}

pub(crate) fn reference_text(
    table: Option<&ResourceTable>,
    sigil: char,
    id: u32,
) -> Result<String> {
    Ok(resource_name_of(table, id)?.map_or_else(
        || format!("{sigil}0x{id:08x}"),
        |name| name.reference(sigil),
    ))
}

impl ResourceTable {
    pub fn reference_name(&self, res_id: u32) -> Result<Option<ResourceName>> {
        resource_name_of(Some(self), res_id)
    }

    /// Find a resource ID by type and name.
    ///
    /// Returns `None` when the name is absent. A malformed possible match is
    /// reported as an error; an entry with a different readable key is skipped.
    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Result<Option<u32>> {
        Ok(self
            .find_entry(type_name, entry_name)?
            .map(EntryLocation::res_id))
    }

    /// Read a simple value by type and name.
    ///
    /// Returns `None` for an absent or complex entry. A malformed possible
    /// match is reported as an error.
    pub fn resource_value(&self, type_name: &str, entry_name: &str) -> Result<Option<ResValue>> {
        Ok(self
            .find_entry(type_name, entry_name)?
            .and_then(|entry| entry.value))
    }

    /// The simple value of `res_id` in each configuration that defines one.
    pub fn values(&self, res_id: u32) -> impl Iterator<Item = Result<(&ResType, ResValue)>> {
        let entry_index = ResourceAddress::decode(res_id).entry_index;
        self.configs(res_id).filter_map(move |(_, res_type)| {
            match res_type.entry_head(entry_index) {
                Ok(Some((_, Some(value)))) => Some(Ok((res_type, value))),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            }
        })
    }

    pub(crate) fn contains_resource_id(&self, res_id: u32) -> Result<bool> {
        let entry_index = ResourceAddress::decode(res_id).entry_index;
        let entries = self
            .configs(res_id)
            .map(|(_, res_type)| res_type.entry(entry_index).map(|entry| entry.map(|_| ())));
        Ok(first_found(entries)?.is_some())
    }

    /// Every configuration chunk of the type `res_id` belongs to, with its package.
    pub(super) fn configs(&self, res_id: u32) -> impl Iterator<Item = (&ResPackage, &ResType)> {
        let ResourceAddress {
            package_id,
            type_id,
            ..
        } = ResourceAddress::decode(res_id);
        self.packages
            .iter()
            .filter(move |package| package.id == package_id)
            .flat_map(move |package| {
                let local = package.local_type_id(type_id);
                package
                    .types
                    .iter()
                    .filter(move |res_type| Some(res_type.id) == local)
                    .map(move |res_type| (package, res_type))
            })
    }

    pub(super) fn configs_mut(&mut self, res_id: u32) -> impl Iterator<Item = &mut ResType> {
        let ResourceAddress {
            package_id,
            type_id,
            ..
        } = ResourceAddress::decode(res_id);
        self.packages
            .iter_mut()
            .filter(move |package| package.id == package_id)
            .flat_map(move |package| {
                let local = package.local_type_id(type_id);
                package
                    .types
                    .iter_mut()
                    .filter(move |res_type| Some(res_type.id) == local)
            })
    }

    /// The entry `res_id` in the default configuration, with its package.
    pub(super) fn default_entry(&self, res_id: u32) -> Result<Option<(&ResPackage, ResEntry)>> {
        let entry_index = ResourceAddress::decode(res_id).entry_index;
        first_found(
            self.configs(res_id)
                .filter(|(_, res_type)| res_type.is_default_config())
                .map(|(package, res_type)| {
                    res_type
                        .entry(entry_index)
                        .map(|entry| entry.map(|entry| (package, entry)))
                }),
        )
    }

    pub fn resource_names(&self, type_name: &str) -> Result<Vec<String>> {
        let Some(package) = self.packages.first() else {
            return Ok(Vec::new());
        };
        let Some(type_id) = package.local_type(type_name)? else {
            return Ok(Vec::new());
        };
        (0..package.entry_count(type_id))
            .filter_map(|index| package.entry_key(type_id, index).transpose())
            .map(|key| {
                let key = key?;
                package
                    .key_strings
                    .get(key)?
                    .map(Cow::into_owned)
                    .ok_or_else(|| invalid("resource key", format!("invalid index {key}")))
            })
            .collect()
    }

    pub fn has_entry_in(
        &self,
        type_name: &str,
        entry_name: &str,
        qualifiers: &str,
    ) -> Result<bool> {
        Ok(self.entry_in(type_name, entry_name, qualifiers)?.is_some())
    }

    pub(super) fn entry_in(
        &self,
        type_name: &str,
        entry_name: &str,
        qualifiers: &str,
    ) -> Result<Option<ResEntry>> {
        let Some(location) = self.find_entry(type_name, entry_name)? else {
            return Ok(None);
        };
        for (package, res_type) in self.configs(location.res_id()) {
            let config = config_for_qualifiers(qualifiers, package.config_len())?;
            if same_config(res_type.config(), &config) {
                return res_type.entry(location.entry_index);
            }
        }
        Ok(None)
    }

    /// The type and entry name of `res_id`, from whichever configuration
    /// defines it; `None` when no configuration does.
    pub fn resource_name(&self, res_id: u32) -> Result<Option<ResourceName>> {
        let entry_index = ResourceAddress::decode(res_id).entry_index;
        first_found(self.configs(res_id).map(|(package, res_type)| {
            let Some(key) = res_type.entry_key(entry_index)? else {
                return Ok(None);
            };
            let name = |pool: &StringPool, index: u32, what: &'static str| {
                pool.get(index)?
                    .map(Cow::into_owned)
                    .ok_or_else(|| invalid(what, format!("invalid index {index}")))
            };
            Ok(Some(ResourceName {
                package: None,
                type_name: name(
                    &package.type_strings,
                    u32::from(res_type.id) - 1,
                    "resource type",
                )?,
                entry: name(&package.key_strings, key, "resource key")?,
            }))
        }))
    }

    pub(super) fn find_entry(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<EntryLocation>> {
        first_found(
            self.packages
                .iter()
                .map(|package| find_entry_in(package, type_name, entry_name)),
        )
    }
}

fn find_entry_in(
    package: &ResPackage,
    type_name: &str,
    entry_name: &str,
) -> Result<Option<EntryLocation>> {
    let Some(local_type) = package.local_type(type_name)? else {
        return Ok(None);
    };
    let Some(key) = package.key_strings.find(entry_name)? else {
        return Ok(None);
    };
    let Some(entry_index) = package.entry_index(local_type, key)? else {
        return Ok(None);
    };
    let type_id = package.resource_type_id(local_type)?;
    package.resource_id(local_type, entry_index)?;
    first_found(
        package
            .types
            .iter()
            .filter(|t| t.id == local_type)
            .map(|res_type| {
                if res_type.entry_key(entry_index)? != Some(key) {
                    return Ok(None);
                }
                res_type.entry_head(entry_index).map(|head| {
                    head.map(|(_, value)| EntryLocation {
                        package_id: package.id,
                        type_id,
                        entry_index,
                        value,
                    })
                })
            }),
    )
}
