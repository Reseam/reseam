// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    EntryValue, ResType, ResValue, ResourceAddress, ResourceRef, ResourceTable, Result,
    first_found, invalid,
};

impl ResourceTable {
    pub fn find_entries_by_string(&self, string_index: u32) -> Result<Vec<ResourceRef>> {
        let mut refs = Vec::new();
        for package in &self.packages {
            for res_type in &package.types {
                for i in 0..res_type.len() {
                    let Some((key, Some(value))) = res_type.entry_head(i)? else {
                        continue;
                    };
                    if value.kind == ResValue::STRING && value.data == string_index {
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
        let ResourceAddress {
            package_id,
            type_id,
            entry_index,
        } = ResourceAddress::decode(res_id);
        for value in self.values(res_id) {
            value?;
        }
        for res_type in self
            .packages
            .iter_mut()
            .filter(|package| package.id == package_id)
            .flat_map(|package| {
                let local = package.local_type_id(type_id);
                package
                    .types
                    .iter_mut()
                    .filter(move |t| Some(t.id) == local)
            })
        {
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

    pub fn values(&self, res_id: u32) -> impl Iterator<Item = Result<(&ResType, ResValue)>> {
        let ResourceAddress {
            package_id,
            type_id,
            entry_index,
        } = ResourceAddress::decode(res_id);
        self.packages
            .iter()
            .filter(move |package| package.id == package_id)
            .flat_map(move |package| {
                package
                    .types
                    .iter()
                    .filter(move |t| Some(t.id) == package.local_type_id(type_id))
            })
            .filter_map(move |res_type| match res_type.entry_head(entry_index) {
                Ok(Some((_, Some(value)))) => Some(Ok((res_type, value))),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
    }

    pub(crate) fn contains_resource_id(&self, res_id: u32) -> Result<bool> {
        let ResourceAddress {
            package_id,
            type_id,
            entry_index,
        } = ResourceAddress::decode(res_id);
        let entries = self
            .packages
            .iter()
            .filter(|package| package.id == package_id)
            .flat_map(|package| {
                package
                    .types
                    .iter()
                    .filter(move |t| Some(t.id) == package.local_type_id(type_id))
            })
            .map(|res_type| res_type.entry(entry_index).map(|entry| entry.map(|_| ())));
        Ok(first_found(entries)?.is_some())
    }
}
