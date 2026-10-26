// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Hermes execution bytecode. Version 98 is supported.
//!
//! Files borrow their input, including string storage and function bodies. Keep
//! the input mapping alive and unchanged until the file is dropped. Unedited
//! output preserves every byte, including padding, the footer, and any epilogue.

mod assemble;
mod bytes;
mod code;
mod codegen;
mod dataflow;
mod edit;
mod environment;
mod error;
#[cfg(test)]
mod fixtures;
mod hook;
mod index;
mod interface;
mod link;
mod literal;
mod metro;
mod model;
mod opcode;
mod parse;
mod switch;
mod write;

pub use code::{BlockId, Code, Condition, HookKind, Operand, Place, SlotId, Statement};
pub use edit::{Editor, Edits};
pub use error::{HermesError, Result};
pub use index::FunctionIndex;
pub use interface::imports;
pub use link::{Import, ModuleId};
pub use model::{FunctionId, HermesFile, HermesImage};
