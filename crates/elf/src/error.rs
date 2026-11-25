// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use thiserror::Error;

use crate::Function;

/// A malformed or unsupported library, a lookup without exactly one match, or an output failure.
#[derive(Debug, Error)]
pub enum ElfError {
    #[error("not an ELF file")]
    Magic,
    #[error(
        "unsupported ELF file: {0}; supported: little-endian 64-bit AArch64 and 32-bit ARM shared libraries"
    )]
    Unsupported(&'static str),
    /// Data that breaks the format, at a byte offset of the file.
    #[error("invalid ELF file at offset {offset:#x}: {reason}")]
    Invalid { offset: usize, reason: &'static str },
    #[error("ELF file has no {0} section")]
    MissingSection(&'static str),
    #[error("address {0:#x} is not stored in the ELF file")]
    Unmapped(u64),
    #[error("invalid byte pattern {pattern:?}: {reason}")]
    Pattern {
        pattern: String,
        reason: &'static str,
    },
    #[error("native function lookup matched {count} functions: {query}")]
    Match { query: String, count: usize },
    #[error(
        "byte pattern {pattern} covers {count} conditional branches of the native function at {:#x}; it must cover one",
        .function.address
    )]
    Branch {
        pattern: String,
        function: Function,
        count: usize,
    },
    #[error("native function at {:#x} is {} bytes; the edit needs {needed}", .function.address, .function.size)]
    TooSmall { function: Function, needed: u64 },
    #[error(transparent)]
    Truncated(#[from] reseam_bytes::Truncated),
    #[error("ELF output: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ElfError>;

pub(crate) fn invalid(offset: usize, reason: &'static str) -> ElfError {
    ElfError::Invalid { offset, reason }
}
