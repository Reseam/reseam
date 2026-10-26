// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::id::{EntryLocation, ResourceAddress};
use super::{ResEntry, ResPackage, ResType, ResourceTable};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;

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

impl ResourceTable {
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

    pub(super) fn entry_name(&self, res_id: u32) -> Result<Option<Cow<'_, str>>> {
        match self.default_entry(res_id)? {
            Some((package, entry)) => package.key_strings.get(entry.key),
            None => Ok(None),
        }
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
    let Some(type_index) = package.type_strings.find(type_name)? else {
        return Ok(None);
    };
    let Some(key) = package.key_strings.find(entry_name)? else {
        return Ok(None);
    };
    let local_type = u8::try_from(type_index + 1)
        .map_err(|_| invalid("resource type", "type ID exceeds 255"))?;
    let type_id = package.resource_type_id(local_type)?;
    first_found(
        package
            .types
            .iter()
            .filter(|t| t.id == local_type)
            .flat_map(|res_type| {
                (0..res_type.len()).map(move |i| {
                    if res_type.entry_key(i)? != Some(key) {
                        return Ok(None);
                    }
                    package.resource_id(local_type, i)?;
                    res_type.entry_head(i).map(|head| {
                        head.map(|(_, value)| EntryLocation {
                            package_id: package.id,
                            type_id,
                            entry_index: i,
                            value,
                        })
                    })
                })
            }),
    )
}
