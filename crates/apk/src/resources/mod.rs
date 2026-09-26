// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `resources.arsc` as a view over its bytes. String pools, type specs and
//! type chunks are read in place; only entries a patch adds or changes are
//! owned, and serialization copies every untouched chunk verbatim.

mod complex;
mod config;
mod entry;
mod package;
mod res_type;
mod type_spec;

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};

use reseam_dex::file::DexBytes;

use crate::axml;
use crate::buf::{read_u16_le, require_len, write_u32};
use crate::chunk::{self, write_header};
use crate::error::{invalid, Result};
use crate::string_pool::{StringPool, CHUNK_STRING_POOL};
use crate::value::ResValue;

pub use config::config_for_qualifiers;
pub use entry::{EntryValue, MapEntry, ResEntry};
pub use package::ResPackage;
pub use res_type::ResType;
pub use type_spec::TypeSpec;

const RES_TABLE_TYPE: u16 = 0x0002;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
const RES_TABLE_TYPE_TYPE: u16 = 0x0201;
const TABLE_HEADER_LEN: usize = 12;
const MAX_TYPE_ENTRIES: usize = 1_000_000;

#[derive(Debug, Clone)]
pub struct ResourceTable {
    pub global_strings: StringPool,
    pub packages: Vec<ResPackage>,
}

/// `ResTable_map::ATTR_TYPE`, the item of an `attr` bag that holds its format mask.
const ATTR_TYPE: u32 = 0x0100_0000;
const ATTR_FORMAT_ENUM: u32 = 1 << 16;
const ATTR_FORMAT_FLAGS: u32 = 1 << 17;

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

/// Where an entry lives and, for a simple entry, its value.
#[derive(Debug, Clone, Copy)]
struct EntryLocation {
    package_id: u32,
    type_id: u8,
    entry_index: usize,
    value: Option<ResValue>,
}

impl EntryLocation {
    fn res_id(self) -> u32 {
        res_id(self.package_id, self.type_id, self.entry_index)
    }
}

pub fn res_id(package_id: u32, type_id: u8, entry_index: usize) -> u32 {
    (package_id << 24) | ((type_id as u32) << 16) | (entry_index as u32)
}

fn split_res_id(res_id: u32) -> (u32, u8, usize) {
    (
        (res_id >> 24) & 0xFF,
        ((res_id >> 16) & 0xFF) as u8,
        (res_id & 0xFFFF) as usize,
    )
}

impl ResourceTable {
    pub fn parse(data: DexBytes) -> Result<Self> {
        let buf = data.as_bytes();
        require_len(buf, 0, TABLE_HEADER_LEN, "resource table")?;
        let kind = read_u16_le(buf, 0, "resource table")?;
        if kind != RES_TABLE_TYPE {
            return Err(invalid(
                "resource table",
                format!("expected 0x0002, got 0x{kind:04x}"),
            ));
        }
        let header_size = read_u16_le(buf, 2, "resource table")? as usize;

        let mut global_strings = None;
        let mut packages = Vec::new();
        for chunk in chunk::chunks(buf, header_size..buf.len(), "resource chunk")? {
            match chunk.kind {
                CHUNK_STRING_POOL if global_strings.is_none() => {
                    global_strings = Some(StringPool::parse(&data, chunk.range)?);
                }
                RES_TABLE_PACKAGE_TYPE => {
                    packages.push(ResPackage::parse(&data, chunk.range, chunk.header_size)?)
                }
                _ => {}
            }
        }
        Ok(Self {
            global_strings: global_strings.unwrap_or_else(|| StringPool::new(Vec::new(), true)),
            packages,
        })
    }

    pub fn get_string(&self, index: u32) -> Option<Cow<'_, str>> {
        self.global_strings.get(index)
    }

    pub fn set_string(&mut self, index: u32, value: String) {
        self.global_strings.set(index, value);
    }

    /// Adds a string to the global pool and returns its index. Strings added
    /// earlier in this run are reused; the file's own strings are not
    /// searched, since that would index every translation in the table.
    pub fn add_global_string(&mut self, value: &str) -> u32 {
        self.global_strings.intern_added(value)
    }

    pub fn find_entries_by_string(&self, string_index: u32) -> Vec<ResourceRef> {
        let mut refs = Vec::new();
        for package in &self.packages {
            for res_type in &package.types {
                for i in 0..res_type.len() {
                    let Some((key, Some(value))) = res_type.entry_head(i) else {
                        continue;
                    };
                    if value.kind == ResValue::STRING && value.data == string_index {
                        refs.push(ResourceRef {
                            res_id: res_id(package.id, res_type.id, i),
                            key_name: package
                                .key_strings
                                .get(key)
                                .map(Cow::into_owned)
                                .unwrap_or_default(),
                        });
                    }
                }
            }
        }
        refs
    }

    pub fn replace_entry_string(&mut self, res_id: u32, string_index: u32) {
        let (package_id, type_id, entry_index) = split_res_id(res_id);
        for res_type in self
            .packages
            .iter_mut()
            .filter(|package| package.id == package_id)
            .flat_map(|package| package.types.iter_mut())
            .filter(|res_type| res_type.id == type_id)
        {
            let Some(mut entry) = res_type.entry(entry_index) else {
                continue;
            };
            if let EntryValue::Simple(value) = &mut entry.value {
                if value.kind == ResValue::STRING {
                    value.data = string_index;
                    res_type.set(entry_index, Some(entry));
                }
            }
        }
    }

    /// The simple value of `res_id` in each configuration that defines it.
    pub fn values(&self, res_id: u32) -> impl Iterator<Item = (&ResType, ResValue)> {
        let (package_id, type_id, entry_index) = split_res_id(res_id);
        self.packages
            .iter()
            .filter(move |package| package.id == package_id)
            .flat_map(|package| &package.types)
            .filter(move |res_type| res_type.id == type_id)
            .filter_map(move |res_type| Some((res_type, res_type.entry_head(entry_index)?.1?)))
    }

    pub(crate) fn contains_resource_id(&self, res_id: u32) -> Result<bool> {
        let (package_id, type_id, entry_index) = split_res_id(res_id);
        let mut first_error = None;
        for res_type in self
            .packages
            .iter()
            .filter(|package| package.id == package_id)
            .flat_map(|package| &package.types)
            .filter(|res_type| res_type.id == type_id)
        {
            match res_type.entry_checked(entry_index) {
                Ok(Some(_)) => return Ok(true),
                Ok(None) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(false), Err)
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
        package.name = name.to_string();
        Ok(())
    }

    /// Adds or replaces the default-configuration entry `type_name/entry_name`
    /// in the first package and returns its id.
    pub fn add_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: ResValue,
    ) -> Option<u32> {
        self.set_entry(type_name, entry_name, EntryValue::Simple(value))
    }

    /// Add or replace a simple entry in the default configuration.
    ///
    /// Returns its resource ID, or `None` when the package cannot provide the
    /// type. Fails if an existing entry that may have the same name is malformed.
    pub fn add_resource_checked(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: ResValue,
    ) -> Result<Option<u32>> {
        self.set_entry_in(type_name, entry_name, EntryValue::Simple(value), "")
    }

    /// Writes `value` as the default-configuration entry `type_name/entry_name`
    /// of the first package, keeping the index and flags an existing entry of
    /// that name already has in any configuration.
    pub(crate) fn set_entry(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: EntryValue,
    ) -> Option<u32> {
        self.set_entry_in(type_name, entry_name, value, "")
            .ok()
            .flatten()
    }

    /// [`set_entry`](Self::set_entry) in the configuration `qualifiers` names,
    /// creating that configuration's chunk when the type has none.
    fn set_entry_in(
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
        let Some(type_id) = package.ensure_type(type_name) else {
            return Ok(None);
        };
        let existing_key = package.key_strings.find(entry_name);
        let entry_index = existing_key
            .map(|key| package.entry_index(type_id, key))
            .transpose()?
            .flatten()
            .unwrap_or_else(|| package.entry_count(type_id));
        let key = existing_key.unwrap_or_else(|| package.key_strings.intern(entry_name));
        let chunk = package.config_type(type_id, config);
        let flags = chunk
            .entry_checked(entry_index)?
            .map_or(0, |current| current.flags);
        chunk.set(entry_index, Some(ResEntry { flags, key, value }));
        package.grow_type(type_id, entry_index + 1);
        Ok(Some(res_id(package.id, type_id, entry_index)))
    }

    pub fn add_string_resource(&mut self, name: &str, value: &str) -> Option<u32> {
        let index = self.add_global_string(value);
        self.add_resource("string", name, ResValue::string(index))
    }

    /// Add or replace a default-configuration string resource.
    ///
    /// Returns its ID, or `None` when the package cannot provide the type.
    /// Fails if an existing entry that may have this name is malformed.
    pub fn add_string_resource_checked(&mut self, name: &str, value: &str) -> Result<Option<u32>> {
        let index = self.add_global_string(value);
        self.add_resource_checked("string", name, ResValue::string(index))
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

    pub fn ensure_id(&mut self, name: &str) -> Option<u32> {
        self.find_resource_id("id", name)
            .or_else(|| self.add_resource("id", name, ResValue::id_entry()))
    }

    /// Find or create an `id/name` entry in the default configuration.
    ///
    /// Returns its ID, or `None` when it cannot be created. A malformed
    /// possible match is reported as an error instead of creating a duplicate.
    pub fn ensure_id_checked(&mut self, name: &str) -> Result<Option<u32>> {
        match self.find_resource_id_checked("id", name)? {
            Some(id) => Ok(Some(id)),
            None => self.add_resource_checked("id", name, ResValue::id_entry()),
        }
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.find_entry(type_name, entry_name)
            .map(EntryLocation::res_id)
    }

    /// Find a resource ID by type and name.
    ///
    /// Returns `None` when the name is absent. A malformed possible match is
    /// reported as an error; an entry with a different readable key is skipped.
    pub fn find_resource_id_checked(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<u32>> {
        Ok(self
            .find_entry_checked(type_name, entry_name)?
            .map(EntryLocation::res_id))
    }

    /// The value of a simple entry; `None` for a missing or complex entry.
    pub fn resource_value(&self, type_name: &str, entry_name: &str) -> Option<ResValue> {
        self.find_entry(type_name, entry_name)?.value
    }

    /// Read a simple value by type and name.
    ///
    /// Returns `None` for an absent or complex entry. A malformed possible
    /// match is reported as an error.
    pub fn resource_value_checked(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<ResValue>> {
        Ok(self
            .find_entry_checked(type_name, entry_name)?
            .and_then(|entry| entry.value))
    }

    pub fn string_value(&self, name: &str) -> Option<Cow<'_, str>> {
        let value = self.resource_value("string", name)?;
        self.get_string(value.string_index()?)
    }

    /// Read `string/name` from the global pool.
    ///
    /// Returns `None` for an absent or non-string value. Malformed entries
    /// and invalid string pool indices are errors.
    pub fn string_value_checked(&self, name: &str) -> Result<Option<Cow<'_, str>>> {
        let Some(value) = self.resource_value_checked("string", name)? else {
            return Ok(None);
        };
        let Some(index) = value.string_index() else {
            return Ok(None);
        };
        self.get_string(index).map(Some).ok_or_else(|| {
            invalid(
                "res string",
                format!("string/{name} has invalid pool index {index}"),
            )
        })
    }

    pub fn set_string_value(&mut self, name: &str, value: &str) -> bool {
        self.set_string_value_checked(name, value).unwrap_or(false)
    }

    /// Rewrite the global pool value referenced by `string/name`.
    ///
    /// Returns `false` when the resource is absent or has no string value.
    /// Malformed entries and invalid pool indices are errors. Other resources
    /// sharing the same pool index also see the new text.
    pub fn set_string_value_checked(&mut self, name: &str, value: &str) -> Result<bool> {
        let index = self
            .resource_value_checked("string", name)?
            .and_then(ResValue::string_index);
        match index {
            Some(index) => {
                if self.get_string(index).is_none() {
                    return Err(invalid(
                        "res string",
                        format!("string/{name} has invalid pool index {index}"),
                    ));
                }
                self.set_string(index, value.to_string());
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// The path each configuration of `type_name/entry_name` points at, the
    /// default configuration first. A file resource is a string entry holding
    /// the path aapt packed the file to.
    pub fn file_paths(&self, type_name: &str, entry_name: &str) -> Result<Vec<String>> {
        let location = self.find_entry(type_name, entry_name).ok_or_else(|| {
            invalid(
                "resource entry",
                format!("the table has no {type_name}/{entry_name}"),
            )
        })?;
        let mut paths: Vec<(bool, String)> = Vec::new();
        for (res_type, value) in self.values(location.res_id()) {
            let path = value
                .string_index()
                .and_then(|index| self.get_string(index))
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

    /// The map of the complex entry `type_name/entry_name` in the default
    /// configuration.
    pub fn complex_entries(&self, type_name: &str, entry_name: &str) -> Option<Vec<MapEntry>> {
        let location = self.find_entry(type_name, entry_name)?;
        match self.default_entry(location.res_id())?.1.value {
            EntryValue::Complex { entries, .. } => Some(entries),
            EntryValue::Simple(_) => None,
        }
    }

    /// The value the app's `attr` `attr_id` gives the enum or flag name
    /// `symbol`; `None` when its format takes neither or it has no such name.
    pub fn attr_symbol(&self, attr_id: u32, symbol: &str) -> Option<AttrSymbol> {
        let EntryValue::Complex { entries, .. } = self.default_entry(attr_id)?.1.value else {
            return None;
        };
        let format = entries
            .iter()
            .find(|item| item.name == ATTR_TYPE)?
            .value
            .data;
        let flags = format & ATTR_FORMAT_FLAGS != 0;
        if !flags && format & ATTR_FORMAT_ENUM == 0 {
            return None;
        }
        entries
            .iter()
            .find(|item| self.entry_name(item.name).as_deref() == Some(symbol))
            .map(|item| AttrSymbol {
                value: item.value.data,
                flags,
            })
    }

    fn entry_name(&self, res_id: u32) -> Option<Cow<'_, str>> {
        let (package, entry) = self.default_entry(res_id)?;
        package.key_strings.get(entry.key)
    }

    fn default_entry(&self, res_id: u32) -> Option<(&ResPackage, ResEntry)> {
        let (package_id, type_id, entry_index) = split_res_id(res_id);
        self.packages
            .iter()
            .filter(|package| package.id == package_id)
            .find_map(|package| {
                package
                    .types
                    .iter()
                    .filter(|res_type| res_type.id == type_id && res_type.is_default_config())
                    .find_map(|res_type| res_type.entry(entry_index))
                    .map(|entry| (package, entry))
            })
    }

    /// Runs `edit` on the complex value of `type_name/entry_name` in every
    /// configuration that defines it, so a `values-night` variant cannot mask
    /// the change, and returns the entry's id with how many it edited.
    pub(crate) fn edit_complex_entries(
        &mut self,
        type_name: &str,
        entry_name: &str,
        mut edit: impl FnMut(&mut u32, &mut Vec<MapEntry>),
    ) -> Option<(u32, usize)> {
        let location = self.find_entry(type_name, entry_name)?;
        let mut edited = 0;
        for res_type in self
            .packages
            .iter_mut()
            .filter(|package| package.id == location.package_id)
            .flat_map(|package| package.types.iter_mut())
            .filter(|res_type| res_type.id == location.type_id)
        {
            let Some(mut entry) = res_type.entry(location.entry_index) else {
                continue;
            };
            let EntryValue::Complex { parent, entries } = &mut entry.value else {
                continue;
            };
            edit(parent, entries);
            res_type.set(location.entry_index, Some(entry));
            edited += 1;
        }
        Some((location.res_id(), edited))
    }

    /// The `attr` id a bag item name refers to: `android:name` reads the
    /// framework table, an unprefixed name the app's own `attr` entries, which
    /// is what a `<style>` item name means in resource XML.
    pub fn attr_id(&self, name: &str) -> Option<u32> {
        match name.strip_prefix("android:") {
            Some(local) => crate::axml::android_attr_res_id(local),
            None => self.find_resource_id("attr", name),
        }
    }

    /// A value as the text that parses back to it, or `None` for a kind that
    /// has no such form.
    pub fn value_text(&self, value: ResValue) -> Option<String> {
        Some(match value.kind {
            ResValue::STRING => self.get_string(value.data)?.into_owned(),
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
            _ => return None,
        })
    }

    /// Reads `text` the way a value of `attr` is read, interning plain text
    /// into the global pool the way a resource entry holds it.
    pub(crate) fn parse_value(&mut self, text: &str, attr: Option<u32>) -> Result<ResValue> {
        Ok(match axml::parse_attribute_value(text, attr, Some(self))? {
            axml::AttributeValue::Value(value) => value,
            axml::AttributeValue::Text => ResValue::string(self.add_global_string(text)),
        })
    }

    fn find_entry(&self, type_name: &str, entry_name: &str) -> Option<EntryLocation> {
        self.find_entry_checked(type_name, entry_name)
            .ok()
            .flatten()
    }

    fn find_entry_checked(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<EntryLocation>> {
        let mut first_error = None;
        for package in &self.packages {
            let Some(type_id) = package
                .type_strings
                .find(type_name)
                .and_then(|index| u8::try_from(index + 1).ok())
            else {
                continue;
            };
            let Some(key) = package.key_strings.find(entry_name) else {
                continue;
            };
            for res_type in package
                .types
                .iter()
                .filter(|res_type| res_type.id == type_id)
            {
                for i in 0..res_type.len() {
                    match res_type.entry_key_checked(i) {
                        Ok(Some(entry_key)) if entry_key == key => {
                            match res_type.entry_head_checked(i) {
                                Ok(Some((_, value))) => {
                                    return Ok(Some(EntryLocation {
                                        package_id: package.id,
                                        type_id,
                                        entry_index: i,
                                        value,
                                    }));
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    first_error.get_or_insert(error);
                                }
                            }
                        }
                        Err(error) => {
                            first_error.get_or_insert(error);
                        }
                        _ => {}
                    }
                }
            }
        }
        first_error.map_or(Ok(None), Err)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.write_to(&mut out)?;
        Ok(out)
    }

    /// Writes the table into an unlinked temp file, so a table of any size
    /// costs no heap beyond the entries a patch added or changed.
    pub(crate) fn serialize_spooled(&self) -> Result<File> {
        let mut file = tempfile::tempfile()?;
        let mut out = BufWriter::with_capacity(1 << 20, &mut file);
        self.write_to(&mut out)?;
        out.flush()?;
        drop(out);
        Ok(file)
    }

    pub fn write_to(&self, out: &mut dyn Write) -> Result<()> {
        let global = self.global_strings.plan();
        let packages = self
            .packages
            .iter()
            .map(ResPackage::plan)
            .collect::<Result<Vec<_>>>()?;
        let total = TABLE_HEADER_LEN + global.size + packages.iter().map(|p| p.size).sum::<usize>();
        let mut head = Vec::with_capacity(TABLE_HEADER_LEN);
        write_header(&mut head, RES_TABLE_TYPE, TABLE_HEADER_LEN as u16, total);
        write_u32(&mut head, self.packages.len() as u32);
        out.write_all(&head)?;
        global.write(out)?;
        for package in &packages {
            package.write(out)?;
        }
        Ok(())
    }
}
