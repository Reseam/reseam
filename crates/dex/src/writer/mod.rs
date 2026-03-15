//! DEX serialization entrypoints and helpers.

pub(crate) mod code_writer;
pub(crate) mod debug_writer;
pub(crate) mod sort;
pub mod write;

pub use write::write;
