// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::hash::Hasher;
use std::ops::Range;

use crate::error::{Result, invalid};
use crate::opcode::BytecodeVersion;
use crate::parse::{align, read_u32, slice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StringId(pub u32);

pub(crate) const MAGIC: u64 = 0x1f19_03c1_03bc_1fc6;
pub(crate) const HEADER_SIZE: usize = 128;
pub(crate) const FOOTER_SIZE: usize = 20;
pub(crate) const SMALL_HEADER_SIZE: usize = 12;
pub(crate) const LARGE_HEADER_SIZE: usize = 37;
const EXCEPTION_HANDLER_SIZE: usize = 12;

/// `BytecodeFileHeader` fields after the magic, version and source hash, by byte offset.
#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub(crate) enum Field {
    Version = 8,
    FileLength = 32,
    GlobalFunction = 36,
    FunctionCount = 40,
    StringKindCount = 44,
    IdentifierCount = 48,
    StringCount = 52,
    OverflowStringCount = 56,
    StringStorageSize = 60,
    BigIntCount = 64,
    BigIntStorageSize = 68,
    RegExpCount = 72,
    RegExpStorageSize = 76,
    LiteralValueBufferSize = 80,
    ObjectKeyBufferSize = 84,
    ObjectShapeCount = 88,
    StringSwitchCount = 92,
    SegmentId = 96,
    CjsModuleCount = 100,
    FunctionSourceCount = 104,
    DebugInfoOffset = 108,
}

/// `BytecodeOptions` byte of the file header.
pub(crate) const OPTIONS_OFFSET: usize = 112;
pub(crate) const STATIC_BUILTINS: u8 = 1;

/// File sections in layout order, each sized by a header count times a stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Functions,
    Kinds,
    Hashes,
    Strings,
    Overflow,
    Storage,
    Values,
    Keys,
    Shapes,
    BigInts,
    BigIntStorage,
    Regexps,
    RegexpStorage,
    Modules,
    Sources,
}

impl Section {
    pub(crate) const ALL: [Self; 15] = [
        Self::Functions,
        Self::Kinds,
        Self::Hashes,
        Self::Strings,
        Self::Overflow,
        Self::Storage,
        Self::Values,
        Self::Keys,
        Self::Shapes,
        Self::BigInts,
        Self::BigIntStorage,
        Self::Regexps,
        Self::RegexpStorage,
        Self::Modules,
        Self::Sources,
    ];

    pub(crate) const fn count(self) -> Field {
        match self {
            Self::Functions => Field::FunctionCount,
            Self::Kinds => Field::StringKindCount,
            Self::Hashes => Field::IdentifierCount,
            Self::Strings => Field::StringCount,
            Self::Overflow => Field::OverflowStringCount,
            Self::Storage => Field::StringStorageSize,
            Self::Values => Field::LiteralValueBufferSize,
            Self::Keys => Field::ObjectKeyBufferSize,
            Self::Shapes => Field::ObjectShapeCount,
            Self::BigInts => Field::BigIntCount,
            Self::BigIntStorage => Field::BigIntStorageSize,
            Self::Regexps => Field::RegExpCount,
            Self::RegexpStorage => Field::RegExpStorageSize,
            Self::Modules => Field::CjsModuleCount,
            Self::Sources => Field::FunctionSourceCount,
        }
    }

    pub(crate) const fn stride(self) -> usize {
        match self {
            Self::Functions => SMALL_HEADER_SIZE,
            Self::Kinds | Self::Hashes | Self::Strings => 4,
            Self::Storage
            | Self::Values
            | Self::Keys
            | Self::BigIntStorage
            | Self::RegexpStorage => 1,
            Self::Overflow
            | Self::Shapes
            | Self::BigInts
            | Self::Regexps
            | Self::Modules
            | Self::Sources => 8,
        }
    }
}

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

/// Borrowed string storage. UTF-16 may contain unpaired surrogates, so values
/// are compared by code unit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StringValue<'a> {
    pub encoding: Encoding,
    pub bytes: &'a [u8],
}

impl StringValue<'_> {
    fn units(self) -> impl Iterator<Item = u16> {
        let (latin1, utf16) = match self.encoding {
            Encoding::Latin1 => (self.bytes, &[][..]),
            Encoding::Utf16 => (&[][..], self.bytes.as_chunks::<2>().0),
        };
        latin1
            .iter()
            .map(|&byte| u16::from(byte))
            .chain(utf16.iter().map(|unit| u16::from_le_bytes(*unit)))
    }

    /// Hashes code units, so equal text has one fingerprint in either encoding.
    pub(crate) fn fingerprint(self) -> u64 {
        let mut hash = rustc_hash::FxHasher::default();
        for unit in self.units() {
            hash.write_u16(unit);
        }
        hash.finish()
    }

    pub(crate) fn equals(self, text: &str) -> bool {
        self.units().eq(text.encode_utf16())
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

/// Which invocations a function refuses (`ProhibitInvoke`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prohibit {
    Call,
    Construct,
    Neither,
}

/// `FuncKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionKind {
    Normal,
    Generator,
    Async,
}

/// `FunctionHeaderFlag`, without the overflowed bit, which follows the header's encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionFlags {
    pub prohibit: Prohibit,
    pub strict: bool,
    pub exception_handler: bool,
    pub debug_info: bool,
    pub kind: FunctionKind,
}

impl FunctionFlags {
    const STRICT: u8 = 1 << 2;
    const EXCEPTION_HANDLER: u8 = 1 << 3;
    const DEBUG_INFO: u8 = 1 << 4;
    const OVERFLOWED: u8 = 1 << 5;

    fn decode(byte: u8, offset: usize) -> Result<Self> {
        Ok(Self {
            prohibit: match byte & 3 {
                0 => Prohibit::Call,
                1 => Prohibit::Construct,
                2 => Prohibit::Neither,
                _ => return Err(invalid(offset, "invalid prohibited invocation")),
            },
            strict: byte & Self::STRICT != 0,
            exception_handler: byte & Self::EXCEPTION_HANDLER != 0,
            debug_info: byte & Self::DEBUG_INFO != 0,
            kind: match byte >> 6 {
                0 => FunctionKind::Normal,
                1 => FunctionKind::Generator,
                2 => FunctionKind::Async,
                _ => return Err(invalid(offset, "invalid function kind")),
            },
        })
    }

    fn encode(self, overflowed: bool) -> u8 {
        let prohibit = match self.prohibit {
            Prohibit::Call => 0,
            Prohibit::Construct => 1,
            Prohibit::Neither => 2,
        };
        let kind = match self.kind {
            FunctionKind::Normal => 0,
            FunctionKind::Generator => 1,
            FunctionKind::Async => 2,
        };
        let bit = |set: bool, bit: u8| if set { bit } else { 0 };
        prohibit
            | bit(self.strict, Self::STRICT)
            | bit(self.exception_handler, Self::EXCEPTION_HANDLER)
            | bit(self.debug_info, Self::DEBUG_INFO)
            | bit(overflowed, Self::OVERFLOWED)
            | kind << 6
    }
}

/// A bit field of the small function header: `width` bits at `shift` in the
/// little-endian word at byte `word`.
struct BitField {
    word: usize,
    shift: u32,
    width: u32,
}

impl BitField {
    const fn fits(&self, value: u32) -> bool {
        value >> self.width == 0
    }

    fn get(&self, header: &[u8; SMALL_HEADER_SIZE]) -> u32 {
        let word = u32::from_le_bytes(
            header[self.word..self.word + 4]
                .try_into()
                .expect("small header words are in bounds"),
        );
        (word >> self.shift) & ((1 << self.width) - 1)
    }

    fn put(&self, header: &mut [u8; SMALL_HEADER_SIZE], value: u32) {
        let range = self.word..self.word + 4;
        let word = u32::from_le_bytes(
            header[range.clone()]
                .try_into()
                .expect("small header words are in bounds"),
        );
        header[range].copy_from_slice(&(word | value << self.shift).to_le_bytes());
    }
}

/// `SmallFuncHeader` fields (`FUNC_HEADER_FIELDS`). The flags occupy the last byte.
const OFFSET: BitField = BitField {
    word: 0,
    shift: 0,
    width: 25,
};
const PARAMETERS: BitField = BitField {
    word: 0,
    shift: 25,
    width: 5,
};
const LOOP_DEPTH: BitField = BitField {
    word: 0,
    shift: 30,
    width: 2,
};
const SIZE: BitField = BitField {
    word: 4,
    shift: 0,
    width: 14,
};
const NAME: BitField = BitField {
    word: 4,
    shift: 14,
    width: 8,
};
const NUMBER_REGS: BitField = BitField {
    word: 4,
    shift: 22,
    width: 5,
};
const NON_POINTER_REGS: BitField = BitField {
    word: 4,
    shift: 27,
    width: 5,
};
const FRAME_SIZE: BitField = BitField {
    word: 8,
    shift: 0,
    width: 8,
};
const READ_CACHE: BitField = BitField {
    word: 8,
    shift: 8,
    width: 8,
};
const WRITE_CACHE: BitField = BitField {
    word: 8,
    shift: 16,
    width: 6,
};
const OBJECT_CACHE: BitField = BitField {
    word: 8,
    shift: 22,
    width: 1,
};
const PRIVATE_CACHE: BitField = BitField {
    word: 8,
    shift: 23,
    width: 1,
};
const FLAGS: usize = 11;
/// An overflowed small header splits its large header's offset between these fields.
const LARGE_LOW: BitField = OFFSET;
const LARGE_HIGH: BitField = NAME;

#[derive(Debug, Clone)]
pub(crate) struct FunctionHeader {
    pub offset: u32,
    pub parameters: u32,
    pub loop_depth: u32,
    pub size: u32,
    pub name: u32,
    pub number_regs: u32,
    pub non_pointer_regs: u32,
    pub frame_size: u32,
    pub read_cache: u8,
    pub write_cache: u8,
    pub object_cache: u8,
    pub private_cache: u8,
    pub flags: FunctionFlags,
    /// Where the large header is stored, for overflowed functions. Exception
    /// handlers and debug offsets follow it.
    pub large: Option<usize>,
}

impl FunctionHeader {
    pub(crate) fn decode(source: &[u8], position: usize) -> Result<Self> {
        let small: &[u8; SMALL_HEADER_SIZE] = slice(source, position, SMALL_HEADER_SIZE)?
            .try_into()
            .expect("slice has the small header size");
        if small[FLAGS] & FunctionFlags::OVERFLOWED != 0 {
            return Self::decode_large(
                source,
                (LARGE_HIGH.get(small) << 24 | LARGE_LOW.get(small)) as usize,
            );
        }
        let flags = FunctionFlags::decode(small[FLAGS], position)?;
        if flags.exception_handler || flags.debug_info {
            return Err(invalid(
                position,
                "small function header with exception or debug info",
            ));
        }
        Ok(Self {
            offset: OFFSET.get(small),
            parameters: PARAMETERS.get(small),
            loop_depth: LOOP_DEPTH.get(small),
            size: SIZE.get(small),
            name: NAME.get(small),
            number_regs: NUMBER_REGS.get(small),
            non_pointer_regs: NON_POINTER_REGS.get(small),
            frame_size: FRAME_SIZE.get(small),
            read_cache: READ_CACHE.get(small) as u8,
            write_cache: WRITE_CACHE.get(small) as u8,
            object_cache: OBJECT_CACHE.get(small) as u8,
            private_cache: PRIVATE_CACHE.get(small) as u8,
            flags,
            large: None,
        })
    }

    fn decode_large(source: &[u8], large: usize) -> Result<Self> {
        let data = slice(source, large, LARGE_HEADER_SIZE)?;
        let word = |index: usize| read_u32(data, index * 4);
        Ok(Self {
            offset: word(0)?,
            parameters: word(1)?,
            loop_depth: word(2)?,
            size: word(3)?,
            name: word(4)?,
            number_regs: word(5)?,
            non_pointer_regs: word(6)?,
            frame_size: word(7)?,
            read_cache: data[32],
            write_cache: data[33],
            object_cache: data[34],
            private_cache: data[35],
            flags: FunctionFlags::decode(data[36], large + 36)?,
            large: Some(large),
        })
    }

    pub(crate) fn fits_small(&self) -> bool {
        OFFSET.fits(self.offset)
            && PARAMETERS.fits(self.parameters)
            && LOOP_DEPTH.fits(self.loop_depth)
            && SIZE.fits(self.size)
            && NAME.fits(self.name)
            && NUMBER_REGS.fits(self.number_regs)
            && NON_POINTER_REGS.fits(self.non_pointer_regs)
            && FRAME_SIZE.fits(self.frame_size)
            && WRITE_CACHE.fits(self.write_cache.into())
            && OBJECT_CACHE.fits(self.object_cache.into())
            && PRIVATE_CACHE.fits(self.private_cache.into())
            && !self.flags.exception_handler
            && !self.flags.debug_info
    }

    /// The small header, or the overflowed form pointing at the large header at `large`.
    pub(crate) fn encode_small(&self, large: Option<u32>) -> [u8; SMALL_HEADER_SIZE] {
        let mut bytes = [0; SMALL_HEADER_SIZE];
        if let Some(large) = large {
            LARGE_LOW.put(&mut bytes, large & 0x00ff_ffff);
            LARGE_HIGH.put(&mut bytes, large >> 24);
            bytes[FLAGS] = FunctionFlags::OVERFLOWED;
        } else {
            OFFSET.put(&mut bytes, self.offset);
            PARAMETERS.put(&mut bytes, self.parameters);
            LOOP_DEPTH.put(&mut bytes, self.loop_depth);
            SIZE.put(&mut bytes, self.size);
            NAME.put(&mut bytes, self.name);
            NUMBER_REGS.put(&mut bytes, self.number_regs);
            NON_POINTER_REGS.put(&mut bytes, self.non_pointer_regs);
            FRAME_SIZE.put(&mut bytes, self.frame_size);
            READ_CACHE.put(&mut bytes, self.read_cache.into());
            WRITE_CACHE.put(&mut bytes, self.write_cache.into());
            OBJECT_CACHE.put(&mut bytes, self.object_cache.into());
            PRIVATE_CACHE.put(&mut bytes, self.private_cache.into());
            bytes[FLAGS] = self.flags.encode(false);
        }
        bytes
    }

    pub(crate) fn encode_large(&self) -> [u8; LARGE_HEADER_SIZE] {
        let mut bytes = [0; LARGE_HEADER_SIZE];
        for (index, value) in [
            self.offset,
            self.parameters,
            self.loop_depth,
            self.size,
            self.name,
            self.number_regs,
            self.non_pointer_regs,
            self.frame_size,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[32..].copy_from_slice(&[
            self.read_cache,
            self.write_cache,
            self.object_cache,
            self.private_cache,
            self.flags.encode(true),
        ]);
        bytes
    }
}

/// An exception handler covering bytecode offsets `start..end`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExceptionHandler {
    pub start: u32,
    pub end: u32,
    pub target: u32,
}

impl ExceptionHandler {
    pub(crate) fn covers(&self, offset: u32) -> bool {
        (self.start..self.end).contains(&offset)
    }

    /// The handler table that follows a large header: a count, then the handlers.
    pub(crate) fn encode_table(handlers: &[Self]) -> Vec<u8> {
        std::iter::once(handlers.len() as u32)
            .chain(handlers.iter().flat_map(|h| [h.start, h.end, h.target]))
            .flat_map(u32::to_le_bytes)
            .collect()
    }

    pub(crate) fn table_size(count: usize) -> usize {
        4 + count * EXCEPTION_HANDLER_SIZE
    }
}

/// A borrowed function view. Parameter count excludes the implicit `this`.
pub(crate) struct Function<'a> {
    pub version: BytecodeVersion,
    pub header: FunctionHeader,
    pub source: &'a [u8],
}

impl Function<'_> {
    pub(crate) fn name(&self) -> StringId {
        StringId(self.header.name)
    }

    pub(crate) fn parameter_count(&self) -> u32 {
        self.header.parameters.saturating_sub(1)
    }

    pub(crate) fn body_range(&self) -> Range<usize> {
        let start = self.header.offset as usize;
        start..start + self.header.size as usize
    }

    pub(crate) fn instructions(&self) -> Result<Vec<crate::opcode::Instruction>> {
        crate::opcode::decode(self.version, &self.source[self.body_range()])
    }

    pub(crate) fn exception_handlers(&self) -> Result<Vec<ExceptionHandler>> {
        let Some(large) = self
            .header
            .large
            .filter(|_| self.header.flags.exception_handler)
        else {
            return Ok(Vec::new());
        };
        let position = align(large + LARGE_HEADER_SIZE);
        let count = read_u32(self.source, position)? as usize;
        let size = count
            .checked_mul(EXCEPTION_HANDLER_SIZE)
            .ok_or_else(|| invalid(position, "exception table overflow"))?;
        slice(self.source, position + 4, size)?
            .as_chunks::<EXCEPTION_HANDLER_SIZE>()
            .0
            .iter()
            .map(|entry| {
                Ok(ExceptionHandler {
                    start: read_u32(entry, 0)?,
                    end: read_u32(entry, 4)?,
                    target: read_u32(entry, 8)?,
                })
            })
            .collect()
    }
}

/// A validated, borrowed execution-form HBC file. Construction accepts only
/// version 98. Trailing epilogues and all original padding are preserved.
pub struct HermesFile<'a> {
    pub(crate) version: BytecodeVersion,
    pub(crate) source: &'a [u8],
    pub(crate) sections: [Range<usize>; 15],
}

/// Owns immutable source storage and its validated layout for repeated borrowed
/// views. Mapped storage remains file-backed and is released with this owner;
/// constructing views neither copies nor reparses the source. As with
/// `HermesFile`, the underlying file must not change while it is retained.
pub struct HermesImage {
    source: reseam_storage::Bytes,
    version: BytecodeVersion,
    sections: [Range<usize>; 15],
}

impl HermesImage {
    /// Validates the source once, returning the same format errors as
    /// `HermesFile::parse`. No bytecode or string storage is copied.
    pub fn parse(source: reseam_storage::Bytes) -> Result<Self> {
        let file = HermesFile::parse(&source)?;
        let version = file.version;
        let sections = file.sections;
        Ok(Self {
            source,
            version,
            sections,
        })
    }

    /// Borrows the validated file for inspection, editing or streamed writing.
    /// The view cannot outlive the owner of its source storage.
    pub fn file(&self) -> HermesFile<'_> {
        HermesFile {
            source: &self.source,
            version: self.version,
            sections: self.sections.clone(),
        }
    }
}

impl<'a> HermesFile<'a> {
    pub fn version(&self) -> u32 {
        self.version as u32
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
            version: self.version,
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
