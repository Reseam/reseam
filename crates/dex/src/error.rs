use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Errors produced while parsing, validating, or writing DEX data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DexError {
    #[error("Invalid magic bytes: expected dex\\n0NN\\0, got {found:?}")]
    InvalidMagic { found: [u8; 8] },

    #[error("Unsupported DEX version: {version}")]
    UnsupportedVersion { version: String },

    #[error("Checksum mismatch: expected {expected:#010x}, computed {computed:#010x}")]
    ChecksumMismatch { expected: u32, computed: u32 },

    #[error("Signature mismatch")]
    SignatureMismatch,

    #[error("File truncated: expected {expected} bytes, got {actual}")]
    FileTruncated { expected: usize, actual: usize },

    #[error("Invalid offset {offset:#010x} for {section} (file size: {file_size:#010x})")]
    InvalidOffset {
        offset: u32,
        section: &'static str,
        file_size: u32,
    },

    #[error("Invalid LEB128 encoding at offset {offset:#010x}")]
    InvalidLeb128 { offset: usize },

    #[error("Invalid MUTF-8 encoding at offset {offset:#010x}: {detail}")]
    InvalidMutf8 { offset: usize, detail: String },

    #[error("Invalid opcode {opcode:#04x} at code offset {offset}")]
    InvalidOpcode { opcode: u8, offset: u32 },

    #[error("String table not sorted at index {index}")]
    StringTableUnsorted { index: u32 },

    #[error("Duplicate class definition for {descriptor}")]
    DuplicateClass { descriptor: String },

    #[error("Index out of bounds: {index_type} index {index} >= table size {table_size}")]
    IndexOutOfBounds {
        index_type: &'static str,
        index: u32,
        table_size: u32,
    },

    #[error("Alignment violation: {section} at offset {offset:#010x} (required: {required}-byte)")]
    AlignmentViolation {
        section: &'static str,
        offset: u32,
        required: u32,
    },

    #[error("Invalid header size: expected 0x70, got {size:#x}")]
    InvalidHeaderSize { size: u32 },

    #[error("Invalid endian tag: expected 0x12345678, got {tag:#010x}")]
    InvalidEndianTag { tag: u32 },

    #[error("Invalid encoded value type {type_byte:#04x}")]
    InvalidEncodedValueType { type_byte: u8 },

    #[error("Invalid annotation visibility {value}")]
    InvalidAnnotationVisibility { value: u8 },

    #[error("Invalid method handle type {value}")]
    InvalidMethodHandleType { value: u16 },

    #[error("Invalid debug bytecode {opcode:#04x}")]
    InvalidDebugBytecode { opcode: u8 },

    #[error("Buffer exhausted at offset {offset}")]
    BufferExhausted { offset: usize },

    #[error("Invalid catch handler size")]
    InvalidCatchHandlerSize,

    #[error("Invalid {kind}: {descriptor}")]
    InvalidDescriptor {
        kind: &'static str,
        descriptor: String,
    },

    #[error("Invalid call site at index {index}: {detail}")]
    InvalidCallSite { index: u32, detail: String },

    #[error("Invalid hidden API flag {value}")]
    InvalidHiddenApiFlag { value: u32 },

    #[error("Parser panic while {context}: {detail}")]
    ParserPanic {
        context: &'static str,
        detail: String,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenient result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, DexError>;

/// Converts unexpected parser panics into structured errors.
pub(crate) fn catch_parser_panic<T>(
    context: &'static str,
    f: impl FnOnce() -> Result<T>,
) -> Result<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => Err(DexError::ParserPanic {
            context,
            detail: panic_payload_message(&payload),
        }),
    }
}

fn panic_payload_message(payload: &Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".to_owned()
}
