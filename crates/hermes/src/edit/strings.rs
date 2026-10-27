// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use rustc_hash::FxHashMap;

use crate::HermesFile;
use crate::error::{HermesError, Result};
use crate::layout::{Field, Section};
use crate::string::{
    Encoding, KindRun, StringEntry, StringId, StringKind, StringValue, fingerprint,
};

/// The strings added to a file, the table entries, kinds, identifier hashes and storage they
/// append, and an index of every string by text, so interning reuses existing strings.
pub(crate) struct AddedStrings {
    /// The ID of the first added string.
    first: u32,
    storage_base: usize,
    overflow_base: usize,
    strings: Vec<AddedString>,
    index: FxHashMap<(u64, StringKind), Vec<StringId>>,
    /// Whether `index` holds the file's own strings, which it reads on first use.
    original_indexed: bool,
    kinds: Vec<u8>,
    hashes: Vec<u8>,
    entries: Vec<u8>,
    overflow: Vec<u8>,
    storage: Vec<u8>,
}

struct AddedString {
    bytes: Vec<u8>,
    encoding: Encoding,
    kind: StringKind,
}

impl AddedString {
    fn value(&self) -> StringValue<'_> {
        StringValue {
            encoding: self.encoding,
            bytes: &self.bytes,
        }
    }
}

/// How far the added strings reached at one point.
#[derive(Clone, Copy)]
pub(super) struct Marks {
    strings: usize,
    hashes: usize,
    overflow: usize,
    storage: usize,
}

impl AddedStrings {
    pub(super) fn new(file: &HermesFile<'_>) -> Self {
        Self {
            first: file.string_count(),
            storage_base: file.field(Field::StringStorageSize) as usize,
            overflow_base: file.field(Field::OverflowStringCount) as usize,
            strings: Vec::new(),
            index: FxHashMap::default(),
            original_indexed: false,
            kinds: Vec::new(),
            hashes: Vec::new(),
            entries: Vec::new(),
            overflow: Vec::new(),
            storage: Vec::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// The bytes added to `section`, for the sections strings own.
    pub(super) fn section(&self, section: Section) -> Option<&[u8]> {
        match section {
            Section::Kinds => Some(&self.kinds),
            Section::Hashes => Some(&self.hashes),
            Section::Strings => Some(&self.entries),
            Section::Overflow => Some(&self.overflow),
            Section::Storage => Some(&self.storage),
            _ => None,
        }
    }

    /// The string `id`, original or added.
    pub(crate) fn string<'a>(
        &'a self,
        file: &HermesFile<'a>,
        id: StringId,
    ) -> Result<StringValue<'a>> {
        match id.0.checked_sub(self.first) {
            Some(added) => Ok(self.strings[added as usize].value()),
            None => file.string(id),
        }
    }

    /// The first string of `kind` with this text, or a new one. A string used only as a value
    /// stays a separate entry when interned as an identifier, so original IDs keep their kind.
    pub(crate) fn intern(
        &mut self,
        file: &HermesFile<'_>,
        text: &str,
        kind: StringKind,
    ) -> Result<StringId> {
        self.index_original(file)?;
        let key = fingerprint(text.encode_utf16());
        for &id in self.index.get(&(key, kind)).into_iter().flatten() {
            if self.string(file, id)?.equals(text) {
                return Ok(id);
            }
        }
        let (encoding, bytes) = if text.is_ascii() {
            (Encoding::Latin1, text.as_bytes().to_vec())
        } else {
            let units = text.encode_utf16().flat_map(u16::to_le_bytes);
            (Encoding::Utf16, units.collect())
        };
        let value = StringValue {
            encoding,
            bytes: &bytes,
        };
        self.append(value, kind)
    }

    fn index_original(&mut self, file: &HermesFile<'_>) -> Result<()> {
        if self.original_indexed {
            return Ok(());
        }
        let mut index = FxHashMap::<_, Vec<_>>::default();
        for (id, kind) in file.strings() {
            index
                .entry((file.string(id)?.fingerprint(), kind))
                .or_default()
                .push(id);
        }
        for (key, ids) in &self.index {
            index.entry(*key).or_default().extend_from_slice(ids);
        }
        self.index = index;
        self.original_indexed = true;
        Ok(())
    }

    /// Adds a string of `kind`. Identifiers carry their hash; strings whose storage offset or
    /// length exceed the inline entry get an overflow entry.
    pub(crate) fn append(&mut self, value: StringValue<'_>, kind: StringKind) -> Result<StringId> {
        let id = StringId(
            self.first
                .checked_add(self.strings.len() as u32)
                .ok_or(HermesError::Limit("four billion strings"))?,
        );
        if value.encoding == Encoding::Utf16
            && !(self.storage_base + self.storage.len()).is_multiple_of(2)
        {
            self.storage.push(0);
        }
        let offset = u32::try_from(self.storage_base + self.storage.len())
            .map_err(|_| HermesError::Limit("four GiB of string storage"))?;
        let length = (value.bytes.len() / value.encoding.unit_size()) as u32;
        let entry = match StringEntry::inline(offset, length, value.encoding) {
            Some(entry) => entry,
            None => self.overflow_entry(offset, length, value.encoding)?,
        };
        self.entries
            .extend_from_slice(&entry.encode().to_le_bytes());
        self.kinds
            .extend_from_slice(&KindRun { kind, count: 1 }.encode().to_le_bytes());
        if kind == StringKind::Identifier {
            self.hashes
                .extend_from_slice(&value.identifier_hash().to_le_bytes());
        }
        self.storage.extend_from_slice(value.bytes);
        self.index
            .entry((value.fingerprint(), kind))
            .or_default()
            .push(id);
        self.strings.push(AddedString {
            bytes: value.bytes.to_vec(),
            encoding: value.encoding,
            kind,
        });
        Ok(id)
    }

    fn overflow_entry(
        &mut self,
        offset: u32,
        length: u32,
        encoding: Encoding,
    ) -> Result<StringEntry> {
        let index =
            u32::try_from(self.overflow_base + self.overflow.len() / Section::Overflow.stride())
                .ok()
                .filter(|&index| index < StringEntry::OFFSET_LIMIT)
                .ok_or(HermesError::Limit("2^23 overflowing strings"))?;
        self.overflow.extend_from_slice(&offset.to_le_bytes());
        self.overflow.extend_from_slice(&length.to_le_bytes());
        Ok(StringEntry::Overflow { index, encoding })
    }

    pub(super) fn marks(&self) -> Marks {
        Marks {
            strings: self.strings.len(),
            hashes: self.hashes.len(),
            overflow: self.overflow.len(),
            storage: self.storage.len(),
        }
    }

    /// Removes the strings added since `marks`.
    pub(super) fn truncate(&mut self, marks: Marks) {
        for (position, string) in self.strings.iter().enumerate().skip(marks.strings) {
            let id = StringId(self.first + position as u32);
            let key = (string.value().fingerprint(), string.kind);
            let ids = self.index.get_mut(&key).expect("added strings are indexed");
            ids.retain(|&indexed| indexed != id);
            if ids.is_empty() {
                self.index.remove(&key);
            }
        }
        self.strings.truncate(marks.strings);
        self.kinds.truncate(marks.strings * 4);
        self.entries.truncate(marks.strings * 4);
        self.hashes.truncate(marks.hashes);
        self.overflow.truncate(marks.overflow);
        self.storage.truncate(marks.storage);
    }
}
