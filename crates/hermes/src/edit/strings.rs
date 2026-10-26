// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Editor;
use crate::error::{Result, invalid};
use crate::layout::{Field, Section};
use crate::string::{
    Encoding, KindRun, StringEntry, StringId, StringKind, StringValue, fingerprint,
};

pub(crate) struct AddedString {
    pub bytes: Vec<u8>,
    pub encoding: Encoding,
    pub kind: StringKind,
    pub id: StringId,
}

impl AddedString {
    pub(crate) fn value(&self) -> StringValue<'_> {
        StringValue {
            encoding: self.encoding,
            bytes: &self.bytes,
        }
    }
}

impl Editor<'_> {
    /// Reuses a string of the requested kind or appends one. Identifiers carry
    /// the v98 hash, and strings requiring UTF-16 or overflow entries use them.
    /// A string previously used only as a value remains a separate entry when
    /// subsequently interned as an identifier, preserving original IDs.
    /// The original string index is built once; later calls inspect only matching
    /// fingerprints and verify the text, returning the first matching ID.
    pub(crate) fn intern(&mut self, text: &str, kind: StringKind) -> Result<StringId> {
        self.index_original_strings()?;
        let units: Vec<_> = text.encode_utf16().collect();
        let (encoding, bytes): (_, Vec<_>) = if units.iter().any(|&unit| unit > 127) {
            (
                Encoding::Utf16,
                units.iter().flat_map(|u| u.to_le_bytes()).collect(),
            )
        } else {
            (Encoding::Latin1, units.iter().map(|&u| u as u8).collect())
        };
        let key = fingerprint(units.iter().copied());
        for id in self
            .edits
            .string_index
            .get(&(key, kind))
            .into_iter()
            .flatten()
        {
            if id.0 < self.file.string_count() {
                if self.file.string(*id)?.equals(text) {
                    return Ok(*id);
                }
                continue;
            }
            let string = &self.edits.strings[(id.0 - self.file.string_count()) as usize];
            if string.kind == kind && string.value().equals(text) {
                return Ok(string.id);
            }
        }
        self.append_string(bytes, encoding, units.len() as u32, kind)
    }

    fn index_original_strings(&mut self) -> Result<()> {
        if self.edits.original_strings_indexed {
            return Ok(());
        }
        let mut index = rustc_hash::FxHashMap::<_, Vec<_>>::default();
        for (id, kind) in self.file.strings() {
            index
                .entry((self.file.string(id)?.fingerprint(), kind))
                .or_default()
                .push(id);
        }
        for (key, ids) in &self.edits.string_index {
            index.entry(*key).or_default().extend_from_slice(ids);
        }
        self.edits.string_index = index;
        self.edits.original_strings_indexed = true;
        Ok(())
    }

    pub(crate) fn append_string(
        &mut self,
        bytes: Vec<u8>,
        encoding: Encoding,
        length: u32,
        kind: StringKind,
    ) -> Result<StringId> {
        let id = StringId(
            self.file
                .string_count()
                .checked_add(self.edits.strings.len() as u32)
                .ok_or_else(|| invalid(0, "too many strings"))?,
        );
        let base = self.file.field(Field::StringStorageSize) as usize;
        let storage = &mut self.edits.additions[Section::Storage as usize];
        if encoding == Encoding::Utf16 && !(base + storage.len()).is_multiple_of(2) {
            storage.push(0);
        }
        let offset = u32::try_from(base + storage.len())
            .map_err(|_| invalid(0, "string storage exceeds four GiB"))?;
        let entry = if let Some(entry) = StringEntry::inline(offset, length, encoding) {
            entry
        } else {
            let index = self.file.field(Field::OverflowStringCount) as usize
                + self.edits.additions[Section::Overflow as usize].len()
                    / Section::Overflow.stride();
            let index = u32::try_from(index)
                .ok()
                .filter(|&index| index < StringEntry::OFFSET_LIMIT)
                .ok_or_else(|| invalid(0, "overflow string table exceeds 23 bits"))?;
            let table = &mut self.edits.additions[Section::Overflow as usize];
            table.extend_from_slice(&offset.to_le_bytes());
            table.extend_from_slice(&length.to_le_bytes());
            StringEntry::Overflow { index, encoding }
        };
        self.edits.additions[Section::Strings as usize]
            .extend_from_slice(&entry.encode().to_le_bytes());
        self.edits.additions[Section::Kinds as usize]
            .extend_from_slice(&KindRun { kind, count: 1 }.encode().to_le_bytes());
        let added = AddedString {
            bytes,
            encoding,
            kind,
            id,
        };
        if kind == StringKind::Identifier {
            self.edits.additions[Section::Hashes as usize]
                .extend_from_slice(&added.value().identifier_hash().to_le_bytes());
        }
        self.edits.additions[Section::Storage as usize].extend_from_slice(&added.bytes);
        self.edits
            .string_index
            .entry((added.value().fingerprint(), kind))
            .or_default()
            .push(id);
        self.edits.strings.push(added);
        Ok(id)
    }
}
