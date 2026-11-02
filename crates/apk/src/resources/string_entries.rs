// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use super::config::qualifiers;
use super::{ResPackage, ResType, ResourceTable};
use crate::error::Result;

/// A simple entry holding a string of the table's global pool.
pub(super) struct StringValue<'a> {
    pub(super) string: u32,
    pub(super) res_id: u32,
    pub(super) key: u32,
    pub(super) package: &'a ResPackage,
    pub(super) res_type: &'a ResType,
}

/// One configuration of an entry that holds a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringUse {
    pub res_id: u32,
    /// The configuration's qualifiers, empty for the default one.
    pub qualifiers: String,
}

/// Every entry that holds each global-pool string, built in one pass over
/// the table so a search for many strings does not walk it once per string.
#[derive(Debug, Clone, Default)]
pub struct StringEntries {
    by_string: HashMap<u32, Vec<StringUse>>,
}

impl StringEntries {
    /// The entries holding global string `string`, each configuration once.
    pub fn get(&self, string: u32) -> &[StringUse] {
        self.by_string.get(&string).map_or(&[], Vec::as_slice)
    }
}

impl ResourceTable {
    /// Indexes the entries that hold each global-pool string.
    pub fn string_entries(&self) -> Result<StringEntries> {
        let mut by_string: HashMap<u32, Vec<StringUse>> = HashMap::new();
        for value in self.string_values() {
            let value = value?;
            by_string.entry(value.string).or_default().push(StringUse {
                res_id: value.res_id,
                qualifiers: qualifiers(value.res_type.config()),
            });
        }
        Ok(StringEntries { by_string })
    }

    /// Each simple entry of every configuration whose value is a global-pool string.
    pub(super) fn string_values(&self) -> impl Iterator<Item = Result<StringValue<'_>>> {
        self.packages.iter().flat_map(|package| {
            package.types.iter().flat_map(move |res_type| {
                (0..res_type.len()).filter_map(move |i| {
                    let (key, value) = match res_type.entry_head(i) {
                        Ok(Some((key, Some(value)))) => (key, value),
                        Ok(_) => return None,
                        Err(error) => return Some(Err(error)),
                    };
                    let string = value.string_index()?;
                    Some(
                        package
                            .resource_id(res_type.id, i)
                            .map(|res_id| StringValue {
                                string,
                                res_id,
                                key,
                                package,
                                res_type,
                            }),
                    )
                })
            })
        })
    }
}
