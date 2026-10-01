// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    Bytes, Cow, EntryValue, Range, ResEntry, ResPackage, ResValue, Result, StringPool,
    config_for_qualifiers, invalid, package,
};
pub(super) const RES_TABLE_TYPE: u16 = 0x0002;
pub(super) const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
pub(super) const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
pub(super) const RES_TABLE_TYPE_TYPE: u16 = 0x0201;
pub(super) const TABLE_HEADER_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct ResourceTable {
    pub(super) global_strings: StringPool,
    pub(super) packages: Vec<ResPackage>,
    pub(super) data: Bytes,
    pub(super) header: Range<usize>,
    pub(super) suffix: Range<usize>,
    pub(super) chunks: Vec<TableChunk>,
}

#[derive(Debug, Clone)]
pub(super) enum TableChunk {
    Strings,
    Package(usize),
    Raw(Range<usize>),
}

pub(super) const ATTR_TYPE: u32 = 0x0100_0000;
pub(super) const ATTR_FORMAT_ENUM: u32 = 1 << 16;
pub(super) const ATTR_FORMAT_FLAGS: u32 = 1 << 17;

/// The value an `attr` gives one of its enum or flag names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrSymbol {
    pub value: u32,
    pub flags: bool,
}

/// A string-typed entry and the key it is filed under.
#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub res_id: u32,
    pub key_name: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EntryLocation {
    pub(super) package_id: u32,
    pub(super) type_id: u8,
    pub(super) entry_index: usize,
    pub(super) value: Option<ResValue>,
}

impl EntryLocation {
    pub(super) fn res_id(self) -> u32 {
        res_id(self.package_id, self.type_id, self.entry_index)
    }
}

pub fn res_id(package_id: u32, type_id: u8, entry_index: usize) -> u32 {
    (package_id << 24) | (u32::from(type_id) << 16) | (entry_index as u32)
}

#[derive(Clone, Copy)]
pub(super) struct ResourceAddress {
    pub(super) package_id: u32,
    pub(super) type_id: u8,
    pub(super) entry_index: usize,
}

impl ResourceAddress {
    pub(super) fn decode(id: u32) -> Self {
        Self {
            package_id: id >> 24,
            type_id: (id >> 16) as u8,
            entry_index: (id & 0xffff) as usize,
        }
    }
}

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
    /// Creates an owned table. Parsed tables instead retain file-backed ranges
    /// for every chunk and header extension.
    pub fn new(global_strings: StringPool, packages: Vec<ResPackage>) -> Self {
        let chunks = std::iter::once(TableChunk::Strings)
            .chain((0..packages.len()).map(TableChunk::Package))
            .collect();
        Self {
            global_strings,
            packages,
            data: Bytes::default(),
            header: 0..0,
            suffix: 0..0,
            chunks,
        }
    }

    pub fn packages(&self) -> &[ResPackage] {
        &self.packages
    }

    pub fn global_strings(&self) -> &StringPool {
        &self.global_strings
    }

    pub fn get_string(&self, index: u32) -> Result<Option<Cow<'_, str>>> {
        self.global_strings.get(index)
    }

    pub fn set_string(&mut self, index: u32, value: String) -> Result<()> {
        self.global_strings.set(index, value)
    }

    /// Adds a string to the global pool and returns its index. Strings added
    /// earlier in this run are reused; the file's own strings are not
    /// searched, since that would index every translation in the table.
    pub fn add_global_string(&mut self, value: &str) -> u32 {
        self.global_strings.intern_added(value)
    }

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
        self.set_entry_in(type_name, entry_name, EntryValue::Simple(value), "")
    }

    pub(super) fn set_entry_in(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: EntryValue,
        qualifiers: &str,
    ) -> Result<Option<u32>> {
        let Some(package) = self.packages.first_mut() else {
            return Ok(None);
        };
        let config = config_for_qualifiers(qualifiers, package.config_len())?;
        let Some(type_id) = package.ensure_type(type_name)? else {
            return Ok(None);
        };
        let existing_key = package.key_strings.find(entry_name)?;
        let entry_index = existing_key
            .map(|key| package.entry_index(type_id, key))
            .transpose()?
            .flatten()
            .unwrap_or_else(|| package.entry_count(type_id));
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

    /// Add or replace a default-configuration string resource.
    ///
    /// Returns its ID, or `None` when the package cannot provide the type.
    /// Fails if an existing entry that may have this name is malformed.
    pub fn add_string_resource(&mut self, name: &str, value: &str) -> Result<Option<u32>> {
        let index = self.add_global_string(value);
        self.add_resource("string", name, ResValue::string(index))
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
        self.set_entry_in(type_name, entry_name, value, qualifiers)?
            .ok_or_else(|| {
                invalid(
                    "resource table",
                    format!("{type_name}/{entry_name}: the table refused the entry"),
                )
            })
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

    /// Read `string/name` from the global pool.
    ///
    /// Returns `None` for an absent or non-string value. Malformed entries
    /// and invalid string pool indices are errors.
    pub fn string_value(&self, name: &str) -> Result<Option<Cow<'_, str>>> {
        let Some(value) = self.resource_value("string", name)? else {
            return Ok(None);
        };
        let Some(index) = value.string_index() else {
            return Ok(None);
        };
        self.get_string(index)?.map(Some).ok_or_else(|| {
            invalid(
                "res string",
                format!("string/{name} has invalid pool index {index}"),
            )
        })
    }

    /// Rewrite the global pool value referenced by `string/name`.
    ///
    /// Returns `false` when the resource is absent or has no string value.
    /// Malformed entries and invalid pool indices are errors. Other resources
    /// sharing the same pool index also see the new text.
    pub fn set_string_value(&mut self, name: &str, value: &str) -> Result<bool> {
        let index = self
            .resource_value("string", name)?
            .and_then(ResValue::string_index);
        match index {
            Some(index) => {
                if self.get_string(index)?.is_none() {
                    return Err(invalid(
                        "res string",
                        format!("string/{name} has invalid pool index {index}"),
                    ));
                }
                self.set_string(index, value.to_string())?;
                Ok(true)
            }
            None => Ok(false),
        }
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

    /// The `attr` id a bag item name refers to: `android:name` reads the
    /// framework table, an unprefixed name the app's own `attr` entries, which
    /// is what a `<style>` item name means in resource XML.
    pub fn attr_id(&self, name: &str) -> Result<Option<u32>> {
        match name.strip_prefix("android:") {
            Some(local) => Ok(crate::axml::android_attr_res_id(local)),
            None => self.find_resource_id("attr", name),
        }
    }

    /// A value as the text that parses back to it, or `None` for a kind that
    /// has no such form.
    pub fn value_text(&self, value: ResValue) -> Result<Option<String>> {
        Ok(Some(match value.kind {
            ResValue::STRING => self
                .get_string(value.data)?
                .ok_or_else(|| invalid("resource string", format!("invalid index {}", value.data)))?
                .into_owned(),
            ResValue::REFERENCE if value.data == 0 => "@null".to_string(),
            ResValue::REFERENCE => format!("@0x{:08x}", value.data),
            ResValue::ATTRIBUTE => format!("?0x{:08x}", value.data),
            ResValue::INT_BOOLEAN => (value.data != 0).to_string(),
            ResValue::INT_DEC => (value.data as i32).to_string(),
            ResValue::INT_HEX => format!("0x{:x}", value.data),
            ResValue::FLOAT => f32::from_bits(value.data).to_string(),
            ResValue::INT_COLOR_ARGB8..=ResValue::INT_COLOR_RGB4 => {
                format!("#{:08x}", value.data)
            }
            _ => return Ok(None),
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
                .map(|package| Self::find_entry_in(package, type_name, entry_name)),
        )
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
}
