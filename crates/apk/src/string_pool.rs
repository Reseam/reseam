// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod reader;
mod writer;
pub(crate) use writer::PoolPlan;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::OnceLock;

use reseam_storage::Bytes;
use rustc_hash::FxHasher;

use crate::error::{Result, invalid};

pub(crate) const CHUNK_STRING_POOL: u16 = 0x0001;
const HEADER_LEN: usize = 28;
const FLAG_UTF8: u32 = 1 << 8;
const UTF8_LENGTH_MASK: usize = 0x7fff;

/// Encoding used for newly added strings. Parsed strings retain their original bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StringEncoding {
    Utf8,
    #[default]
    Utf16,
}

#[derive(Debug, Clone, Default)]
pub struct StringPool {
    data: Bytes,
    chunk: Range<usize>,
    raw_len: usize,
    style_count: usize,
    encoding: StringEncoding,
    offsets_start: usize,
    strings_start: usize,
    styles_start: usize,
    owned: Vec<String>,
    overrides: BTreeMap<u32, String>,
    index: OnceLock<Vec<(u32, u32)>>,
}

impl StringPool {
    pub fn new(strings: Vec<String>, encoding: StringEncoding) -> Self {
        Self {
            encoding,
            owned: strings,
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.raw_len + self.owned.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn encoding(&self) -> StringEncoding {
        self.encoding
    }

    /// Reads a string lazily. An out-of-range index is absent; corrupt encoding
    /// or offsets in a present string are errors. UTF-16 text replaces unpaired
    /// surrogates with U+FFFD; the original encoded bytes remain untouched.
    pub fn get(&self, index: u32) -> Result<Option<Cow<'_, str>>> {
        if let Some(s) = self.overrides.get(&index) {
            return Ok(Some(Cow::Borrowed(s)));
        }
        let i = index as usize;
        if i < self.raw_len {
            return self.raw(i).map(Some);
        }
        Ok(self
            .owned
            .get(i - self.raw_len)
            .map(|s| Cow::Borrowed(s.as_str())))
    }

    pub fn iter(&self) -> impl Iterator<Item = Result<Cow<'_, str>>> {
        (0..self.len() as u32).map(move |i| {
            self.get(i)?
                .ok_or_else(|| invalid("string pool", format!("missing index {i}")))
        })
    }

    /// Replaces an existing string. Invalid indices are errors.
    pub fn set(&mut self, index: u32, value: String) -> Result<()> {
        if index as usize >= self.len() {
            return Err(invalid(
                "string pool",
                format!("index {index} is outside pool"),
            ));
        }
        self.overrides.insert(index, value);
        self.index.take();
        Ok(())
    }

    pub fn push(&mut self, value: &str) -> u32 {
        let index = self.len() as u32;
        self.owned.push(value.to_string());
        index
    }

    pub fn find(&self, value: &str) -> Result<Option<u32>> {
        if let Some(i) = self.find_added(value) {
            return Ok(Some(i));
        }
        let index = if let Some(index) = self.index.get() {
            index
        } else {
            let mut entries = (0..self.raw_len as u32)
                .filter(|i| !self.overrides.contains_key(i))
                .map(|i| self.raw(i as usize).map(|s| (hash_str(&s), i)))
                .collect::<Result<Vec<_>>>()?;
            entries.sort_unstable();
            self.index.get_or_init(|| entries)
        };
        let hash = hash_str(value);
        let start = index.partition_point(|&(h, _)| h < hash);
        for &(_, i) in index[start..].iter().take_while(|&&(h, _)| h == hash) {
            if self.get(i)?.as_deref() == Some(value) {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn find_added(&self, value: &str) -> Option<u32> {
        if let Some((&i, _)) = self.overrides.iter().find(|(_, s)| s.as_str() == value) {
            return Some(i);
        }
        self.owned
            .iter()
            .enumerate()
            .find(|(i, s)| {
                !self.overrides.contains_key(&((self.raw_len + i) as u32)) && *s == value
            })
            .map(|(i, _)| i)
            .map(|i| (self.raw_len + i) as u32)
    }

    pub fn intern(&mut self, value: &str) -> Result<u32> {
        Ok(self.find(value)?.unwrap_or_else(|| self.push(value)))
    }

    pub(crate) fn intern_added(&mut self, value: &str) -> u32 {
        self.find_added(value).unwrap_or_else(|| self.push(value))
    }
}

fn hash_str(s: &str) -> u32 {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    hasher.finish() as u32
}
