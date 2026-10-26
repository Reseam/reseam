// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::function::SMALL_HEADER_SIZE;

pub(crate) const MAGIC: u64 = 0x1f19_03c1_03bc_1fc6;
/// The only bytecode version the opcode table describes.
pub(crate) const VERSION: u32 = 98;
pub(crate) const HEADER_SIZE: usize = 128;
pub(crate) const FOOTER_SIZE: usize = 20;

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
