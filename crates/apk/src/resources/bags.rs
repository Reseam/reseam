// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    ATTR_FORMAT_ENUM, ATTR_FORMAT_FLAGS, ATTR_TYPE, AttrSymbol, Cow, EntryValue, MapEntry,
    ResEntry, ResPackage, ResourceAddress, ResourceTable, Result, first_found,
};

impl ResourceTable {
    /// The map of the complex entry `type_name/entry_name` in the default
    /// configuration.
    pub fn complex_entries(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Result<Option<Vec<MapEntry>>> {
        let Some(location) = self.find_entry(type_name, entry_name)? else {
            return Ok(None);
        };
        let Some((_, entry)) = self.default_entry(location.res_id())? else {
            return Ok(None);
        };
        Ok(match entry.value {
            EntryValue::Complex { entries, .. } => Some(entries),
            EntryValue::Simple(_) => None,
        })
    }

    /// The value the app's `attr` `attr_id` gives the enum or flag name
    /// `symbol`; `None` when its format takes neither or it has no such name.
    pub fn attr_symbol(&self, attr_id: u32, symbol: &str) -> Result<Option<AttrSymbol>> {
        let Some((_, entry)) = self.default_entry(attr_id)? else {
            return Ok(None);
        };
        let EntryValue::Complex { entries, .. } = entry.value else {
            return Ok(None);
        };
        let Some(format) = entries
            .iter()
            .find(|item| item.name == ATTR_TYPE)
            .map(|item| item.value.data)
        else {
            return Ok(None);
        };
        let flags = format & ATTR_FORMAT_FLAGS != 0;
        if !flags && format & ATTR_FORMAT_ENUM == 0 {
            return Ok(None);
        }
        for item in entries {
            if self.entry_name(item.name)?.as_deref() == Some(symbol) {
                return Ok(Some(AttrSymbol {
                    value: item.value.data,
                    flags,
                }));
            }
        }
        Ok(None)
    }

    pub(super) fn entry_name(&self, res_id: u32) -> Result<Option<Cow<'_, str>>> {
        match self.default_entry(res_id)? {
            Some((package, entry)) => package.key_strings.get(entry.key),
            None => Ok(None),
        }
    }

    pub(super) fn default_entry(&self, res_id: u32) -> Result<Option<(&ResPackage, ResEntry)>> {
        let ResourceAddress {
            package_id,
            type_id,
            entry_index,
        } = ResourceAddress::decode(res_id);
        first_found(
            self.packages
                .iter()
                .filter(|package| package.id == package_id)
                .flat_map(|package| {
                    package
                        .types
                        .iter()
                        .filter(move |res_type| {
                            Some(res_type.id) == package.local_type_id(type_id)
                                && res_type.is_default_config()
                        })
                        .map(move |res_type| {
                            res_type
                                .entry(entry_index)
                                .map(|entry| entry.map(|entry| (package, entry)))
                        })
                }),
        )
    }

    pub(crate) fn edit_complex_entries(
        &mut self,
        type_name: &str,
        entry_name: &str,
        mut edit: impl FnMut(&mut u32, &mut Vec<MapEntry>),
    ) -> Result<Option<(u32, usize)>> {
        let Some(location) = self.find_entry(type_name, entry_name)? else {
            return Ok(None);
        };
        for res_type in self
            .packages
            .iter()
            .filter(|package| package.id == location.package_id)
            .flat_map(|package| {
                package
                    .types
                    .iter()
                    .filter(move |t| Some(t.id) == package.local_type_id(location.type_id))
            })
        {
            res_type.entry_head(location.entry_index)?;
        }
        let mut edited = 0;
        for res_type in self
            .packages
            .iter_mut()
            .filter(|package| package.id == location.package_id)
            .flat_map(|package| {
                let local = package.local_type_id(location.type_id);
                package
                    .types
                    .iter_mut()
                    .filter(move |t| Some(t.id) == local)
            })
        {
            let Some(mut entry) = res_type.entry(location.entry_index)? else {
                continue;
            };
            let EntryValue::Complex { parent, entries } = &mut entry.value else {
                continue;
            };
            edit(parent, entries);
            res_type.set(location.entry_index, Some(entry))?;
            edited += 1;
        }
        Ok(Some((location.res_id(), edited)))
    }
}
