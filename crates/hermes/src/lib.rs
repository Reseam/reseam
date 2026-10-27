// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Links JavaScript modules into an app's Hermes bytecode and hooks its functions. Version 98
//! is supported.
//!
//! [`Editor`] owns the app's file storage, which must not change while it lives. Unedited output
//! preserves every byte, including padding, the footer, and any epilogue.

mod assemble;
mod bytes;
mod closure;
mod code;
mod dataflow;
mod edit;
mod editor;
mod error;
mod file;
#[cfg(test)]
mod fixtures;
mod function;
mod hook;
mod id;
mod index;
mod interface;
mod layout;
mod link;
mod literal;
mod metro;
mod opcode;
mod parse;
mod string;
mod switch;
mod write;

pub use code::{BlockId, Code, Condition, HookKind, Operand, Place, SlotId, Statement};
pub use edit::ModuleId;
pub use editor::Editor;
pub use error::{HermesError, LinkError, Result, Unsupported};
pub use file::HermesFile;
pub use id::FunctionId;
pub use interface::imports;
pub use link::Import;

use file::HermesImage;
use index::FunctionIndex;
