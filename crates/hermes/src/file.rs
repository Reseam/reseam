// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use crate::bytes::{read_u32, slice};
use crate::error::{Result, invalid};
use crate::function::{Function, FunctionHeader, FunctionId, SMALL_HEADER_SIZE};
use crate::layout::{FOOTER_SIZE, Field, OPTIONS_OFFSET, Section};
use crate::string::{KindRun, StringEntry, StringId, StringValue};

/// A validated, borrowed execution-form HBC file. Construction accepts only
/// version 98. Trailing epilogues and all original padding are preserved.
pub struct HermesFile<'a> {
    pub(crate) source: &'a [u8],
    pub(crate) sections: [Range<usize>; 15],
}

/// Owns immutable source storage and its validated layout for repeated borrowed
/// views. Mapped storage remains file-backed and is released with this owner;
/// constructing views neither copies nor reparses the source. As with
/// `HermesFile`, the underlying file must not change while it is retained.
pub struct HermesImage {
    source: reseam_storage::Bytes,
    sections: [Range<usize>; 15],
}

impl HermesImage {
    /// Validates the source once, returning the same format errors as
    /// `HermesFile::parse`. No bytecode or string storage is copied.
    pub fn parse(source: reseam_storage::Bytes) -> Result<Self> {
        let file = HermesFile::parse(&source)?;
        let sections = file.sections;
        Ok(Self { source, sections })
    }

    /// Borrows the validated file for inspection, editing or streamed writing.
    /// The view cannot outlive the owner of its source storage.
    pub fn file(&self) -> HermesFile<'_> {
        HermesFile {
            source: &self.source,
            sections: self.sections.clone(),
        }
    }
}

impl<'a> HermesFile<'a> {
    pub fn version(&self) -> u32 {
        self.field(Field::Version)
    }

    pub(crate) fn field(&self, field: Field) -> u32 {
        read_u32(self.source, field as usize).expect("parsing checked the header size")
    }

    pub(crate) fn options(&self) -> u8 {
        self.source[OPTIONS_OFFSET]
    }

    pub(crate) fn function_count(&self) -> u32 {
        self.field(Field::FunctionCount)
    }

    pub(crate) fn string_count(&self) -> u32 {
        self.field(Field::StringCount)
    }

    pub(crate) fn global_function(&self) -> FunctionId {
        FunctionId(self.field(Field::GlobalFunction))
    }

    /// The SHA-1 footer, which identifies the file's contents.
    pub(crate) fn footer(&self) -> [u8; FOOTER_SIZE] {
        let end = self.field(Field::FileLength) as usize;
        self.source[end - FOOTER_SIZE..end]
            .try_into()
            .expect("parsing checked the footer bounds")
    }

    pub(crate) fn section(&self, section: Section) -> &'a [u8] {
        &self.source[self.sections[section as usize].clone()]
    }

    pub(crate) fn kind_runs(&self) -> impl Iterator<Item = KindRun> + use<'a> {
        self.section(Section::Kinds)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|entry| KindRun::decode(u32::from_le_bytes(*entry)))
    }

    pub(crate) fn function(&self, id: FunctionId) -> Result<Function<'a>> {
        if id.0 >= self.function_count() {
            return Err(invalid(0, "function ID out of range"));
        }
        let header = FunctionHeader::decode(
            self.source,
            self.sections[Section::Functions as usize].start + id.0 as usize * SMALL_HEADER_SIZE,
        )?;
        slice(self.source, header.offset as usize, header.size as usize)?;
        if header.name >= self.string_count() {
            return Err(invalid(
                header.offset as usize,
                "function name ID out of range",
            ));
        }
        Ok(Function {
            header,
            source: self.source,
        })
    }

    pub(crate) fn string(&self, id: StringId) -> Result<StringValue<'a>> {
        if id.0 >= self.string_count() {
            return Err(invalid(0, "string ID out of range"));
        }
        let (offset, length, encoding) =
            match StringEntry::decode(read_u32(self.section(Section::Strings), id.0 as usize * 4)?)
            {
                StringEntry::Inline {
                    offset,
                    length,
                    encoding,
                } => (offset, length, encoding),
                StringEntry::Overflow { index, encoding } => {
                    let overflow = self.section(Section::Overflow);
                    let position = index as usize * Section::Overflow.stride();
                    (
                        read_u32(overflow, position)?,
                        read_u32(overflow, position + 4)?,
                        encoding,
                    )
                }
            };
        let bytes = slice(
            self.section(Section::Storage),
            offset as usize,
            (length as usize)
                .checked_mul(encoding.unit_size())
                .ok_or_else(|| invalid(offset as usize, "string length overflow"))?,
        )?;
        Ok(StringValue { encoding, bytes })
    }
}
