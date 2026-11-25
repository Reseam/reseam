// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::{read_u16, slice, u32_at};
use reseam_storage::Bytes;

use crate::class::Class;
use crate::error::{ElfError, Result, invalid};

const MAGIC: &[u8] = b"\x7fELF";
const CLASS_32: u8 = 1;
const CLASS_64: u8 = 2;
const LITTLE_ENDIAN: u8 = 1;
const TYPE_SHARED: u16 = 3;
const MACHINE_ARM: u16 = 40;
const MACHINE_AARCH64: u16 = 183;
const SEGMENT_LOAD: u32 = 1;
const SECTION_NO_BITS: u32 = 8;
const FLAG_ALLOC: u64 = 0x2;
const FLAG_EXECUTE: u64 = 0x4;

/// The architectures whose libraries can be edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Machine {
    /// 64-bit AArch64.
    Aarch64,
    /// 32-bit ARM, with A32 and Thumb code.
    Arm,
}

/// A little-endian AArch64 or 32-bit ARM shared library, with its loaded segments and sections
/// read.
pub(crate) struct ElfImage {
    bytes: Bytes,
    pub(crate) class: Class,
    pub(crate) machine: Machine,
    segments: Vec<Segment>,
    sections: Vec<Section>,
}

struct Segment {
    address: u64,
    offset: u64,
    size: u64,
}

pub(crate) struct Section {
    pub(crate) name: String,
    pub(crate) kind: u32,
    flags: u64,
    pub(crate) address: u64,
    offset: u64,
    pub(crate) size: u64,
    pub(crate) link: u32,
}

impl Section {
    /// Code: allocated and executable, with contents in the file.
    pub(crate) fn is_code(&self) -> bool {
        self.flags & (FLAG_ALLOC | FLAG_EXECUTE) == FLAG_ALLOC | FLAG_EXECUTE
            && self.kind != SECTION_NO_BITS
    }

    /// Loaded data that is not code, with contents in the file.
    pub(crate) fn is_data(&self) -> bool {
        self.flags & (FLAG_ALLOC | FLAG_EXECUTE) == FLAG_ALLOC && self.kind != SECTION_NO_BITS
    }
}

impl ElfImage {
    pub(crate) fn parse(bytes: Bytes) -> Result<Self> {
        let file = bytes.as_bytes();
        if !file.starts_with(MAGIC) {
            return Err(ElfError::Magic);
        }
        if file.get(5) != Some(&LITTLE_ENDIAN) {
            return Err(ElfError::Unsupported("not little-endian"));
        }
        if read_u16(file, 16, "ELF header")? != TYPE_SHARED {
            return Err(ElfError::Unsupported("not a shared library"));
        }
        let (class, machine) = match (file.get(4), read_u16(file, 18, "ELF header")?) {
            (Some(&CLASS_64), MACHINE_AARCH64) => (Class::Elf64, Machine::Aarch64),
            (Some(&CLASS_32), MACHINE_ARM) => (Class::Elf32, Machine::Arm),
            _ => return Err(ElfError::Unsupported("not 64-bit AArch64 or 32-bit ARM")),
        };
        let segments = read_segments(file, class)?;
        let sections = read_sections(file, class)?;
        Ok(Self {
            bytes,
            class,
            machine,
            segments,
            sections,
        })
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.bytes.as_bytes()
    }

    pub(crate) fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub(crate) fn find_section(&self, name: &'static str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }

    pub(crate) fn section(&self, name: &'static str) -> Result<&Section> {
        self.find_section(name)
            .ok_or(ElfError::MissingSection(name))
    }

    pub(crate) fn contents(&self, section: &Section) -> Result<&[u8]> {
        contents(self.bytes(), section.offset, section.size, "section")
    }

    /// The file offset holding the `len` loaded bytes from `address`, which one segment must
    /// store in the file.
    pub(crate) fn offset(&self, address: u64, len: u64) -> Result<usize> {
        self.segments
            .iter()
            .find_map(|segment| {
                let at = address.checked_sub(segment.address)?;
                (at.checked_add(len)? <= segment.size).then(|| (segment.offset + at) as usize)
            })
            .ok_or(ElfError::Unmapped(address))
    }

    /// The loaded bytes at `address`, which one segment must store in the file.
    pub(crate) fn loaded(&self, address: u64, len: usize) -> Result<&[u8]> {
        let at = self.offset(address, len as u64)?;
        Ok(&self.bytes()[at..at + len])
    }
}

fn read_segments(file: &[u8], class: Class) -> Result<Vec<Segment>> {
    let layout = class.layout();
    let fields = &layout.segment;
    let table = table(
        file,
        class,
        layout.program_headers,
        layout.program_header_size,
        fields.size,
        "program header",
    )?;
    table
        .chunks_exact(fields.size)
        .filter(|header| u32_at(header, 0) == SEGMENT_LOAD)
        .map(|header| {
            let segment = Segment {
                offset: class.word_at(header, fields.offset),
                address: class.word_at(header, fields.address),
                size: class.word_at(header, fields.file_size),
            };
            contents(file, segment.offset, segment.size, "loaded segment")?;
            Ok(segment)
        })
        .collect()
}

fn read_sections(file: &[u8], class: Class) -> Result<Vec<Section>> {
    let layout = class.layout();
    let fields = &layout.section;
    let table = table(
        file,
        class,
        layout.section_headers,
        layout.section_header_size,
        fields.size,
        "section header",
    )?;
    let headers: Vec<&[u8]> = table.chunks_exact(fields.size).collect();
    let names = headers
        .get(usize::from(read_u16(
            file,
            layout.section_names,
            "ELF header",
        )?))
        .ok_or_else(|| {
            invalid(
                layout.section_names,
                "section name table index past the section headers",
            )
        })?;
    let names = contents(
        file,
        class.word_at(names, fields.offset),
        class.word_at(names, fields.length),
        "section names",
    )?;
    headers
        .iter()
        .map(|header| {
            Ok(Section {
                name: String::from_utf8_lossy(name(names, u32_at(header, 0) as usize)?)
                    .into_owned(),
                kind: u32_at(header, 4),
                flags: class.word_at(header, fields.flags),
                address: class.word_at(header, fields.address),
                offset: class.word_at(header, fields.offset),
                size: class.word_at(header, fields.length),
                link: u32_at(header, fields.link),
            })
        })
        .collect()
}

/// The header table at the offset stored at `offset`, whose entry size is stored at `entry` and
/// must be `size`, with the entry count stored right after it.
fn table<'a>(
    file: &'a [u8],
    class: Class,
    offset: usize,
    entry: usize,
    size: usize,
    what: &'static str,
) -> Result<&'a [u8]> {
    if usize::from(read_u16(file, entry, "ELF header")?) != size {
        return Err(invalid(entry, "unexpected header table entry size"));
    }
    let start = class.read_word(file, offset, "ELF header")?;
    let count = u64::from(read_u16(file, entry + 2, "ELF header")?);
    contents(file, start, count * size as u64, what)
}

fn contents<'a>(file: &'a [u8], offset: u64, size: u64, what: &'static str) -> Result<&'a [u8]> {
    let (Ok(offset), Ok(size)) = (usize::try_from(offset), usize::try_from(size)) else {
        return Err(invalid(file.len(), "range past the address space"));
    };
    Ok(slice(file, offset, size, what)?)
}

pub(crate) fn name(names: &[u8], offset: usize) -> Result<&[u8]> {
    let tail = names
        .get(offset..)
        .ok_or_else(|| invalid(offset, "name past its string table"))?;
    let end = memchr::memchr(0, tail).ok_or_else(|| invalid(offset, "unterminated name"))?;
    Ok(&tail[..end])
}
