// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::{read_u32, read_u64, u32_at, u64_at};

use crate::error::Result;

/// The width of the file's addresses and offsets, which sets where its fields lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    Elf32,
    Elf64,
}

/// Where the fields the crate reads lie, as byte offsets into the ELF header and into each table
/// entry.
pub(crate) struct Layout {
    /// The offsets of the program and section header tables.
    pub(crate) program_headers: usize,
    pub(crate) section_headers: usize,
    /// Each table's entry size, with its entry count in the next two bytes.
    pub(crate) program_header_size: usize,
    pub(crate) section_header_size: usize,
    /// The index of the section that holds section names.
    pub(crate) section_names: usize,
    pub(crate) segment: SegmentFields,
    pub(crate) section: SectionFields,
    pub(crate) symbol: SymbolFields,
}

/// A program header's fields.
pub(crate) struct SegmentFields {
    pub(crate) size: usize,
    pub(crate) offset: usize,
    pub(crate) address: usize,
    pub(crate) file_size: usize,
}

/// A section header's fields, after its name and type, which lead every entry.
pub(crate) struct SectionFields {
    pub(crate) size: usize,
    pub(crate) flags: usize,
    pub(crate) address: usize,
    pub(crate) offset: usize,
    pub(crate) length: usize,
    pub(crate) link: usize,
}

/// A symbol's fields, after its name, which leads every entry.
pub(crate) struct SymbolFields {
    pub(crate) size: usize,
    pub(crate) value: usize,
    pub(crate) length: usize,
    pub(crate) info: usize,
    pub(crate) section: usize,
}

const LAYOUT_32: Layout = Layout {
    program_headers: 28,
    section_headers: 32,
    program_header_size: 42,
    section_header_size: 46,
    section_names: 50,
    segment: SegmentFields {
        size: 32,
        offset: 4,
        address: 8,
        file_size: 16,
    },
    section: SectionFields {
        size: 40,
        flags: 8,
        address: 12,
        offset: 16,
        length: 20,
        link: 24,
    },
    symbol: SymbolFields {
        size: 16,
        value: 4,
        length: 8,
        info: 12,
        section: 14,
    },
};

const LAYOUT_64: Layout = Layout {
    program_headers: 32,
    section_headers: 40,
    program_header_size: 54,
    section_header_size: 58,
    section_names: 62,
    segment: SegmentFields {
        size: 56,
        offset: 8,
        address: 16,
        file_size: 32,
    },
    section: SectionFields {
        size: 64,
        flags: 8,
        address: 16,
        offset: 24,
        length: 32,
        link: 40,
    },
    symbol: SymbolFields {
        size: 24,
        value: 8,
        length: 16,
        info: 4,
        section: 6,
    },
};

impl Class {
    pub(crate) fn layout(self) -> &'static Layout {
        match self {
            Self::Elf32 => &LAYOUT_32,
            Self::Elf64 => &LAYOUT_64,
        }
    }

    /// An address-sized field at `at`, which the caller has bounds-checked.
    pub(crate) fn word_at(self, bytes: &[u8], at: usize) -> u64 {
        match self {
            Self::Elf32 => u64::from(u32_at(bytes, at)),
            Self::Elf64 => u64_at(bytes, at),
        }
    }

    pub(crate) fn read_word(self, bytes: &[u8], at: usize, what: &'static str) -> Result<u64> {
        Ok(match self {
            Self::Elf32 => u64::from(read_u32(bytes, at, what)?),
            Self::Elf64 => read_u64(bytes, at, what)?,
        })
    }
}
