// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;
use std::collections::BTreeMap;

use super::DexBytes;
use crate::error::Result;
use crate::types::TypeIdx;
use crate::types::header::ParseOptions;
use crate::types::hidden_api::ClassHiddenApiFlags;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlagClass {
    pub data_offset: u32,
    pub flags_offset: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct FlagSource {
    pub bytes: DexBytes,
    pub end: usize,
    pub options: ParseOptions,
    pub classes: Vec<(TypeIdx, FlagClass)>,
}

/// Hidden-API member flags keyed by their class and member identities.
/// Source records remain file-backed; each lookup decodes at most one class.
#[derive(Debug, Clone, Default)]
pub struct HiddenApiData {
    source: Option<FlagSource>,
    owned: BTreeMap<TypeIdx, ClassHiddenApiFlags>,
}

impl HiddenApiData {
    pub(crate) fn from_source(mut source: FlagSource) -> Self {
        source.classes.sort_unstable_by_key(|&(type_, _)| type_);
        Self {
            source: Some(source),
            owned: BTreeMap::new(),
        }
    }

    pub fn from_flags(flags: BTreeMap<TypeIdx, ClassHiddenApiFlags>) -> Self {
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
        crate::read::hidden_api::read_class_flags(source, source.classes[index].1)
            .map(|flags| Some(Cow::Owned(flags)))
    }

    /// Replaces a class's flags. Unedited source classes remain deferred.
    pub fn set(&mut self, class: TypeIdx, flags: ClassHiddenApiFlags) {
        self.owned.insert(class, flags);
    }
}
