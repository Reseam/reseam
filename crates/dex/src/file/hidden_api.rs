// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::{read_u32, require_len};
use std::borrow::Cow;
use std::collections::BTreeMap;

use super::ClassTable;
use crate::DexBytes;
use crate::error::{Result, invalid};
use crate::model::TypeIdx;
use crate::model::hidden_api::ClassHiddenApiFlags;
use crate::options::ParseOptions;
use crate::read::hidden_api::{FlagClass, FlagSource, read_class_flags};

/// Hidden-API member flags keyed by their class and member identities.
/// Source records remain file-backed; each lookup decodes at most one class.
#[derive(Debug, Clone, Default)]
pub struct HiddenApiData {
    source: Option<FlagSource>,
    owned: BTreeMap<TypeIdx, ClassHiddenApiFlags>,
}

impl HiddenApiData {
    /// Indexes the item at `offset` by class type and validates every class's flags.
    pub(crate) fn from_raw(
        bytes: DexBytes,
        offset: usize,
        classes: &ClassTable,
        options: ParseOptions,
    ) -> Result<Self> {
        let buf = bytes.as_bytes();
        let size = read_u32(buf, offset, "hidden api")? as usize;
        let table_size = classes
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| invalid("hidden API", "table size overflow"))?;
        if size < table_size {
            return Err(invalid(
                "hidden API",
                "item is smaller than its class offset table",
            ));
        }
        require_len(buf, offset, size, "hidden API")?;
        let mut indexed = Vec::new();
        for i in 0..classes.len() {
            let relative = read_u32(buf, offset + 4 + i * 4, "hidden api")? as usize;
            if relative == 0 {
                continue;
            }
            if relative > size {
                return Err(invalid(
                    "hidden API",
                    "class flag offset is outside item data",
                ));
            }
            let definition = classes
                .raw_def(i)
                .expect("hidden API records are indexed before classes are materialized");
            if definition.class_data_off == 0 {
                continue;
            }
            indexed.push((
                definition.header().class_type,
                FlagClass {
                    data_offset: definition.class_data_off,
                    flags_offset: offset + relative,
                },
            ));
        }
        indexed.sort_unstable_by_key(|&(type_, _)| type_);
        let source = FlagSource {
            bytes,
            end: offset + size,
            options,
            classes: indexed,
        };
        for &(_, class) in &source.classes {
            read_class_flags(&source, class)?;
        }
        Ok(Self {
            source: Some(source),
            owned: BTreeMap::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_flags(flags: BTreeMap<TypeIdx, ClassHiddenApiFlags>) -> Self {
        Self {
            source: None,
            owned: flags,
        }
    }

    /// Reads a class's flags, retaining no decoded source values in the table.
    /// Invalid source encodings return a DEX error.
    pub fn get(&self, class: TypeIdx) -> Result<Option<Cow<'_, ClassHiddenApiFlags>>> {
        if let Some(flags) = self.owned.get(&class) {
            return Ok(Some(Cow::Borrowed(flags)));
        }
        let Some(source) = &self.source else {
            return Ok(None);
        };
        let Ok(index) = source
            .classes
            .binary_search_by_key(&class, |&(type_, _)| type_)
        else {
            return Ok(None);
        };
        read_class_flags(source, source.classes[index].1).map(|flags| Some(Cow::Owned(flags)))
    }
}
