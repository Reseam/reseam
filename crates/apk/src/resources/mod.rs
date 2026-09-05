// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `resources.arsc` as a view over its bytes. String pools, type specs and
//! type chunks are read in place; only entries a patch adds or changes are
//! owned, and serialization copies every untouched chunk verbatim.

mod entry;
mod package;
mod res_type;
mod type_spec;

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};

use reseam_dex::file::DexBytes;

use crate::buf::{read_u16_le, require_len, write_u32};
use crate::chunk::{self, write_header};
use crate::error::{invalid, Result};
use crate::string_pool::{StringPool, CHUNK_STRING_POOL};
use crate::value::ResValue;

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

    /// Adds or replaces the default-configuration entry `type_name/entry_name`
    /// in the first package and returns its id.
    pub(crate) fn contains_resource_id(&self, res_id: u32) -> bool {
        let (package_id, type_id, entry_index) = split_res_id(res_id);
        self.packages
            .iter()
            .filter(|package| package.id == package_id)
            .flat_map(|package| &package.types)
            .any(|res_type| res_type.id == type_id && res_type.entry(entry_index).is_some())
    }

    pub fn add_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
        value: ResValue,
    ) -> Option<u32> {
        let package = self.packages.first_mut()?;
        let type_id = package.ensure_type(type_name)?;
        let key = package.key_strings.intern(entry_name);
        let default_type = package
            .types
            .iter_mut()
            .find(|res_type| res_type.id == type_id && res_type.is_default_config())?;
        let existing = (0..default_type.len())
            .find(|&i| default_type.entry_head(i).is_some_and(|(k, _)| k == key));
        let entry_index = match existing {
            Some(i) => {
                let mut current = default_type.entry(i)?;
                current.value = EntryValue::Simple(value);
                default_type.set(i, Some(current));
                i
            }
            None => default_type.push(Some(ResEntry {
                flags: 0,
                key,
                value: EntryValue::Simple(value),
            })),
        };
        if let Some(spec) = package
            .type_specs
            .iter_mut()
            .find(|spec| spec.id == type_id)
        {
            while spec.len() <= entry_index {
                spec.push(0);
            }
        }
        for res_type in package
            .types
            .iter_mut()
            .filter(|res_type| res_type.id == type_id)
        {
            res_type.pad_to(entry_index + 1);
        }
        Some(res_id(package.id, type_id, entry_index))
    }

    pub fn add_string_resource(&mut self, name: &str, value: &str) -> Option<u32> {
        let index = self.add_global_string(value);
        self.add_resource("string", name, ResValue::string(index))
    }

    pub fn ensure_id(&mut self, name: &str) -> Option<u32> {
        self.find_resource_id("id", name)
            .or_else(|| self.add_resource("id", name, ResValue::reference(0)))
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.find_entry(type_name, entry_name)
            .map(EntryLocation::res_id)
    }

    /// The value of a simple entry; `None` for a missing or complex entry.
    pub fn resource_value(&self, type_name: &str, entry_name: &str) -> Option<ResValue> {
        self.find_entry(type_name, entry_name)?.value
    }

    pub fn string_value(&self, name: &str) -> Option<Cow<'_, str>> {
        let value = self.resource_value("string", name)?;
        self.get_string(value.string_index()?)
    }

    pub fn set_string_value(&mut self, name: &str, value: &str) -> bool {
        match self
            .resource_value("string", name)
            .and_then(ResValue::string_index)
        {
            Some(index) => {
                self.set_string(index, value.to_string());
                true
            }
            None => false,
        }
    }

    fn find_entry(&self, type_name: &str, entry_name: &str) -> Option<EntryLocation> {
        self.packages.iter().find_map(|package| {
            let type_id = u8::try_from(package.type_strings.find(type_name)? + 1).ok()?;
            let key = package.key_strings.find(entry_name)?;
            package
                .types
                .iter()
                .filter(|res_type| res_type.id == type_id)
                .find_map(|res_type| {
                    (0..res_type.len()).find_map(|i| match res_type.entry_head(i) {
                        Some((k, value)) if k == key => Some(EntryLocation {
                            package_id: package.id,
                            type_id,
                            entry_index: i,
                            value,
                        }),
                        _ => None,
                    })
                })
        })
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
