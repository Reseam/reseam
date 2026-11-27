// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::id::ResourceAddress;
use super::{EntryValue, ResEntry, ResourceTable, config_for_qualifiers, package};
use crate::error::{ApkError, Result, invalid};
use crate::res_value::ResValue;

impl ResourceTable {
    /// Renames the first package. `Resources.getIdentifier(name, type, context.getPackageName())`
    /// matches the table's package name, so an app installed under a new package name finds
    /// none of its own resources by name until the table is renamed with it.
    pub fn set_package_name(&mut self, name: &str) -> Result<()> {
        if name.encode_utf16().count() >= package::NAME_UNITS {
            return Err(invalid(
                "resource package",
                format!("{name} is longer than a package header holds"),
            ));
        }
        let package = self
            .packages
            .first_mut()
            .ok_or_else(|| invalid("resource package", "the table has no package"))?;
        package.set_name(name);
        Ok(())
    }

    /// Add or replace a simple entry in the default configuration.
    ///
    /// Returns its resource ID, or `None` when the package cannot provide the
    /// type. Fails if an existing entry that may have the same name is malformed.
    pub fn add_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: ResValue,
    ) -> Result<Option<u32>> {
        self.add_resource_in(type_name, entry_name, value, "")
    }

    /// [`Self::add_resource`] in the configuration `qualifiers` names (`""`, `night`, `v31`).
    pub fn add_resource_in(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: ResValue,
        qualifiers: &str,
    ) -> Result<Option<u32>> {
        self.set_entry_in(type_name, entry_name, EntryValue::Simple(value), qualifiers)
    }

    /// Registers `apk_path` as the file behind `type_name/entry_name` in the
    /// configuration `qualifiers` names (`""`, `xxhdpi`, `night-v26`). aapt
    /// writes a file resource as a string entry holding the path the file was
    /// packed to, and the loader follows that path.
    pub fn add_file_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
        apk_path: &str,
        qualifiers: &str,
    ) -> Result<u32> {
        let index = self.add_global_string(apk_path);
        let value = EntryValue::Simple(ResValue::string(index));
        self.add_entry_in(type_name, entry_name, value, qualifiers)
    }

    /// Find or create an `id/name` entry in the default configuration.
    ///
    /// Returns its ID, or `None` when it cannot be created. A malformed
    /// possible match is reported as an error instead of creating a duplicate.
    pub fn ensure_id(&mut self, name: &str) -> Result<Option<u32>> {
        match self.find_resource_id("id", name)? {
            Some(id) => Ok(Some(id)),
            None => self.add_resource("id", name, ResValue::id_entry()),
        }
    }

    /// [`Self::add_resource_in`] at a fixed `id`, for code that refers to the resource by number.
    /// Fails when `id` is not a `type_name` id of the package, another entry holds it, or the
    /// name already has another id.
    pub fn add_resource_at(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: ResValue,
        qualifiers: &str,
        id: u32,
    ) -> Result<u32> {
        self.put_entry(
            type_name,
            entry_name,
            EntryValue::Simple(value),
            qualifiers,
            Some(id),
        )?
        .ok_or_else(|| refused(type_name, entry_name))
    }

    pub(super) fn add_entry_in(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: EntryValue,
        qualifiers: &str,
    ) -> Result<u32> {
        self.set_entry_in(type_name, entry_name, value, qualifiers)?
            .ok_or_else(|| refused(type_name, entry_name))
    }

    pub(super) fn set_entry_in(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: EntryValue,
        qualifiers: &str,
    ) -> Result<Option<u32>> {
        self.put_entry(type_name, entry_name, value, qualifiers, None)
    }

    fn put_entry(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: EntryValue,
        qualifiers: &str,
        id: Option<u32>,
    ) -> Result<Option<u32>> {
        let Some(package) = self.packages.first_mut() else {
            return Ok(None);
        };
        let config = config_for_qualifiers(qualifiers, package.config_len())?;
        let Some(type_id) = package.ensure_type(type_name)? else {
            return Ok(None);
        };
        let existing_key = package.key_strings.find(entry_name)?;
        let existing_index = existing_key
            .map(|key| package.entry_index(type_id, key))
            .transpose()?
            .flatten();
        let entry_index = match id {
            None => existing_index.unwrap_or_else(|| package.entry_count(type_id)),
            Some(id) => {
                let index = ResourceAddress::decode(id).entry_index;
                let named = |reason: &str| {
                    invalid(
                        "resource id",
                        format!("{type_name}/{entry_name} at {id:#010x}: {reason}"),
                    )
                };
                if package.resource_id(type_id, index)? != id {
                    return Err(named("not an id of this package and type"));
                }
                match existing_index {
                    Some(existing) if existing != index => {
                        return Err(named("the name has another id"));
                    }
                    None if package.entry_key(type_id, index)?.is_some() => {
                        return Err(named("another entry has the id"));
                    }
                    _ => index,
                }
            }
        };
        package.resource_id(type_id, entry_index)?;
        let key = match existing_key {
            Some(key) => key,
            None => package.key_strings.intern(entry_name)?,
        };
        let chunk = package.config_type(type_id, config);
        let flags = chunk.entry(entry_index)?.map_or(0, |current| current.flags);
        chunk.set(entry_index, Some(ResEntry { flags, key, value }))?;
        package.grow_type(type_id, entry_index + 1);
        Ok(Some(package.resource_id(type_id, entry_index)?))
    }
}

fn refused(type_name: &str, entry_name: &str) -> ApkError {
    invalid(
        "resource table",
        format!("{type_name}/{entry_name}: the table refused the entry"),
    )
}
