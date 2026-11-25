// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Finds functions in AArch64 and 32-bit ARM shared libraries by exported symbol, referenced
//! strings and byte patterns, and edits their code.
//!
//! [`Editor`] owns the library's storage, which must not change while it lives. Unedited output
//! preserves every byte.

mod a64;
mod arm;
mod class;
mod code;
mod data;
mod dwarf;
mod editor;
mod error;
mod file;
mod pattern;
mod query;
mod references;
mod symbols;
#[cfg(test)]
mod tests;

pub use editor::Editor;
pub use error::{ElfError, Result};
pub use pattern::Pattern;
pub use query::Query;

/// A function's code: its start address, size in bytes and instruction set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Function {
    pub address: u64,
    pub size: u64,
    pub instruction_set: InstructionSet,
}

impl Function {
    pub(crate) fn contains(&self, address: u64) -> bool {
        address
            .checked_sub(self.address)
            .is_some_and(|at| at < self.size)
    }
}

/// The instruction set a function is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstructionSet {
    /// AArch64, the only one of 64-bit libraries.
    A64,
    /// 32-bit ARM instructions.
    A32,
    /// Thumb-2: 16- and 32-bit instructions. 32-bit libraries mix it with A32 function by
    /// function.
    T32,
}

/// A conditional branch in a function's code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Branch {
    pub address: u64,
    /// Its size in bytes: 2 or 4.
    pub size: u64,
    /// Where it jumps when its condition holds.
    pub target: u64,
    pub instruction_set: InstructionSet,
}
