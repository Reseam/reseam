// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::hash::Hasher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StringId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StringKind {
    String,
    Identifier,
}

/// A string kind table entry: `count` consecutive strings of one kind.
pub(crate) struct KindRun {
    pub kind: StringKind,
    pub count: u32,
}

impl KindRun {
    const IDENTIFIER: u32 = 1 << 31;

    pub(crate) fn decode(entry: u32) -> Self {
        Self {
            kind: if entry & Self::IDENTIFIER == 0 {
                StringKind::String
            } else {
                StringKind::Identifier
            },
            count: entry & !Self::IDENTIFIER,
        }
    }

    pub(crate) fn encode(&self) -> u32 {
        self.count
            | match self.kind {
                StringKind::String => 0,
                StringKind::Identifier => Self::IDENTIFIER,
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    Latin1,
    Utf16,
}

impl Encoding {
    pub(crate) const fn unit_size(self) -> usize {
        match self {
            Self::Latin1 => 1,
            Self::Utf16 => 2,
        }
    }
}

/// A string table entry. Strings whose storage offset or length exceed the inline bit fields
/// refer to an `(offset, length)` pair in the overflow table.
pub(crate) enum StringEntry {
    Inline {
        offset: u32,
        length: u32,
        encoding: Encoding,
    },
    Overflow {
        index: u32,
        encoding: Encoding,
    },
}

impl StringEntry {
    pub(crate) const OFFSET_LIMIT: u32 = 1 << 23;
    const OVERFLOWED: u32 = 255;

    pub(crate) fn decode(entry: u32) -> Self {
        let encoding = if entry & 1 == 0 {
            Encoding::Latin1
        } else {
            Encoding::Utf16
        };
        let offset = (entry >> 1) & (Self::OFFSET_LIMIT - 1);
        let length = entry >> 24;
        if length == Self::OVERFLOWED {
            Self::Overflow {
                index: offset,
                encoding,
            }
        } else {
            Self::Inline {
                offset,
                length,
                encoding,
            }
        }
    }

    /// The inline entry when both fields fit, else `None`.
    pub(crate) fn inline(offset: u32, length: u32, encoding: Encoding) -> Option<Self> {
        (offset < Self::OFFSET_LIMIT && length < Self::OVERFLOWED).then_some(Self::Inline {
            offset,
            length,
            encoding,
        })
    }

    pub(crate) fn encode(&self) -> u32 {
        let (offset, length, encoding) = match *self {
            Self::Inline {
                offset,
                length,
                encoding,
            } => (offset, length, encoding),
            Self::Overflow { index, encoding } => (index, Self::OVERFLOWED, encoding),
        };
        offset << 1 | length << 24 | u32::from(encoding == Encoding::Utf16)
    }
}

/// Hashes code units, so equal text has one fingerprint in either encoding.
pub(crate) fn fingerprint(units: impl IntoIterator<Item = u16>) -> u64 {
    let mut hash = rustc_hash::FxHasher::default();
    for unit in units {
        hash.write_u16(unit);
    }
    hash.finish()
}

/// Borrowed string storage. UTF-16 may contain unpaired surrogates, so values
/// are compared by code unit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StringValue<'a> {
    pub encoding: Encoding,
    pub bytes: &'a [u8],
}

impl StringValue<'_> {
    pub(crate) fn units(self) -> impl Iterator<Item = u16> {
        let (latin1, utf16) = match self.encoding {
            Encoding::Latin1 => (self.bytes, &[][..]),
            Encoding::Utf16 => (&[][..], self.bytes.as_chunks::<2>().0),
        };
        latin1
            .iter()
            .map(|&byte| u16::from(byte))
            .chain(utf16.iter().map(|unit| u16::from_le_bytes(*unit)))
    }

    pub(crate) fn fingerprint(self) -> u64 {
        fingerprint(self.units())
    }

    pub(crate) fn equals(self, text: &str) -> bool {
        self.units().eq(text.encode_utf16())
    }

    /// The text, unless the string has an unpaired surrogate.
    pub(crate) fn text(self) -> Option<String> {
        String::from_utf16(&self.units().collect::<Vec<_>>()).ok()
    }

    /// The identifier hash Hermes stores for each identifier.
    pub(crate) fn identifier_hash(self) -> u32 {
        self.units().fold(0_u32, |hash, unit| {
            let hash = hash.wrapping_add(u32::from(unit));
            let hash = hash.wrapping_add(hash << 10);
            hash ^ (hash >> 6)
        })
    }
}
