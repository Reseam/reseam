//! Code item decoding split into focused helper modules.

pub use orchestration::read_code_item;

mod arithmetic;
mod decode;
mod format;
mod invoke;
mod memory;
mod orchestration;
mod payload;
