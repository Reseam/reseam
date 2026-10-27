// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::HermesFile;
use crate::error::{HermesError, Result, invalid};
use crate::layout::Section;
use crate::string::StringId;
use reseam_bytes::u32_at;

/// The value type of a run in a literal buffer (`SerializedLiteralGenerator` tags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Literal {
    Null,
    True,
    False,
    Number,
    LongString,
    ShortString,
    Undefined,
    Integer,
}

impl Literal {
    const MASK: u8 = 0x70;
    /// Set when the run length continues into a second byte.
    pub(crate) const LONG_RUN: u8 = 0x80;
    pub(crate) const LENGTH: u8 = 0x0f;

    pub(crate) fn decode(tag: u8) -> Self {
        match (tag & Self::MASK) >> 4 {
            0 => Self::Null,
            1 => Self::True,
            2 => Self::False,
            3 => Self::Number,
            4 => Self::LongString,
            5 => Self::ShortString,
            6 => Self::Undefined,
            _ => Self::Integer,
        }
    }

    pub(crate) const fn tag(self) -> u8 {
        (self as u8) << 4
    }

    pub(crate) const fn size(self) -> usize {
        match self {
            Self::Null | Self::True | Self::False | Self::Undefined => 0,
            Self::ShortString => 2,
            Self::LongString | Self::Integer => 4,
            Self::Number => 8,
        }
    }
}

/// The longest run a tag and its second length byte can describe.
pub(crate) const MAX_RUN: usize = 0xfff;

/// One literal of a buffer: its kind and its encoded value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LiteralValue<'a> {
    pub(crate) kind: Literal,
    pub(crate) bytes: &'a [u8],
}

impl LiteralValue<'_> {
    /// The string this literal holds, if it is a string.
    pub(crate) fn string(&self) -> Option<StringId> {
        match self.kind {
            Literal::ShortString => Some(u32::from(u16::from_le_bytes(
                self.bytes.try_into().expect("short strings hold two bytes"),
            ))),
            Literal::LongString => Some(u32::from_le_bytes(
                self.bytes.try_into().expect("long strings hold four bytes"),
            )),
            _ => None,
        }
        .map(StringId)
    }
}

/// Reads `count` literals from `offset` the way the runtime does: run by run,
/// stopping inside a run once `count` literals are read.
pub(crate) fn read_literals<'a>(
    file: &HermesFile<'a>,
    buffer: Section,
    offset: usize,
    count: usize,
) -> Result<Vec<LiteralValue<'a>>> {
    let byte = |at: usize| Ok::<_, HermesError>(file.section_bytes(buffer, at, 1)?[0]);
    let mut literals = Vec::with_capacity(count);
    let mut cursor = offset;
    while literals.len() < count {
        let tag = byte(cursor)?;
        cursor += 1;
        let mut length = usize::from(tag & Literal::LENGTH);
        if tag & Literal::LONG_RUN != 0 {
            length = length << 8 | usize::from(byte(cursor)?);
            cursor += 1;
        }
        if length == 0 {
            let start = file.sections[buffer as usize].start;
            return Err(invalid(start + cursor, "empty literal run"));
        }
        let kind = Literal::decode(tag);
        for _ in 0..length.min(count - literals.len()) {
            literals.push(LiteralValue {
                kind,
                bytes: file.section_bytes(buffer, cursor, kind.size())?,
            });
            cursor += kind.size();
        }
    }
    Ok(literals)
}

/// The keys of object shape `shape`, in slot order.
pub(crate) fn shape_keys<'a>(file: &HermesFile<'a>, shape: u32) -> Result<Vec<LiteralValue<'a>>> {
    let stride = Section::Shapes.stride();
    let entry = file.section_bytes(Section::Shapes, shape as usize * stride, stride)?;
    read_literals(
        file,
        Section::Keys,
        u32_at(entry, 0) as usize,
        u32_at(entry, 4) as usize,
    )
}
