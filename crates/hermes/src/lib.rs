//! Hermes execution bytecode. Version 98 is supported.
//!
//! Files borrow their input, including string storage and function bodies. Keep
//! the input mapping alive and unchanged until the file is dropped. Unedited
//! output preserves every byte, including padding, the footer, and any epilogue.

mod assemble;
mod edit;
mod error;
mod exports;
mod index;
mod link;
mod model;
pub mod opcode;
mod parse;
mod wrap;
mod write;

pub use edit::{Editor, Edits};
pub use error::{HermesError, Result};
pub use index::FunctionIndex;
pub use link::ModuleId;
pub use model::HermesImage;
pub use model::{Function, FunctionId, HermesFile, StringId, StringKind, StringValue};
pub use opcode::BytecodeVersion;
