// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Hermes execution bytecode. Version 98 is supported.
//!
//! Files borrow their input, including string storage and function bodies. Keep
//! the input mapping alive and unchanged until the file is dropped. Unedited
//! output preserves every byte, including padding, the footer, and any epilogue.

mod assemble;
mod constant;
mod edit;
mod error;
mod exports;
mod index;
mod link;
mod model;
mod opcode;
mod parse;
mod wrap;
mod write;

pub use constant::Constant;
pub use edit::{Editor, Edits};
pub use error::{HermesError, Result};
pub use index::FunctionIndex;
pub use link::ModuleId;
pub use model::{FunctionId, HermesFile, HermesImage};
pub use wrap::Argument;

use model::Function;
