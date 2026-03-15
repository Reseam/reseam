/// Errors produced while parsing, validating, or writing DEX data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DexError {
    #[error("truncated {section} at offset {offset:#x}: need {needed} bytes, have {available}")]
    Truncated {
        section: &'static str,
        offset: usize,
        needed: usize,
        available: usize,
    },

    #[error("malformed {section} at offset {offset:#x}: {reason}")]
    Malformed {
        section: &'static str,
        offset: usize,
        reason: String,
    },

    #[error("invalid {section}: {reason}")]
    Invalid {
        section: &'static str,
        reason: String,
    },

    #[error("unsupported {feature}: {detail}")]
    Unsupported {
        feature: &'static str,
        detail: String,
    },

    #[error("internal error while {operation}: {reason}")]
    Internal {
        operation: &'static str,
        reason: String,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenient result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, DexError>;

pub(crate) fn truncated(
    section: &'static str,
    offset: usize,
    needed: usize,
    available: usize,
) -> DexError {
    DexError::Truncated {
        section,
        offset,
        needed,
        available,
    }
}

pub(crate) fn malformed(
    section: &'static str,
    offset: usize,
    reason: impl Into<String>,
) -> DexError {
    DexError::Malformed {
        section,
        offset,
        reason: reason.into(),
    }
}

pub(crate) fn invalid(section: &'static str, reason: impl Into<String>) -> DexError {
    DexError::Invalid {
        section,
        reason: reason.into(),
    }
}

pub(crate) fn unsupported(feature: &'static str, detail: impl Into<String>) -> DexError {
    DexError::Unsupported {
        feature,
        detail: detail.into(),
    }
}

pub(crate) fn require_len(
    buf: &[u8],
    offset: usize,
    needed: usize,
    section: &'static str,
) -> Result<()> {
    if offset.checked_add(needed).is_none_or(|end| end > buf.len()) {
        return Err(truncated(
            section,
            offset,
            needed,
            buf.len().saturating_sub(offset),
        ));
    }
    Ok(())
}

pub(crate) fn slice<'a>(
    buf: &'a [u8],
    offset: usize,
    len: usize,
    section: &'static str,
) -> Result<&'a [u8]> {
    require_len(buf, offset, len, section)?;
    Ok(&buf[offset..offset + len])
}

pub(crate) fn read_u8(buf: &[u8], offset: usize, section: &'static str) -> Result<u8> {
    require_len(buf, offset, 1, section)?;
    Ok(buf[offset])
}

pub(crate) fn read_u16_le(buf: &[u8], offset: usize, section: &'static str) -> Result<u16> {
    let bytes = slice(buf, offset, 2, section)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn read_u32_le(buf: &[u8], offset: usize, section: &'static str) -> Result<u32> {
    let bytes = slice(buf, offset, 4, section)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn buffer_exhausted(section: &'static str, offset: usize) -> DexError {
    truncated(section, offset, 1, 0)
}

pub(crate) fn invalid_leb128(offset: usize) -> DexError {
    malformed("leb128", offset, "invalid LEB128 encoding")
}

pub(crate) fn invalid_mutf8(offset: usize, detail: impl Into<String>) -> DexError {
    malformed("mutf8", offset, detail)
}

pub(crate) fn invalid_descriptor(kind: &'static str, descriptor: impl Into<String>) -> DexError {
    invalid(kind, format!("invalid descriptor: {}", descriptor.into()))
}

pub(crate) fn index_out_of_bounds(
    index_type: &'static str,
    index: u32,
    table_size: u32,
) -> DexError {
    invalid(
        index_type,
        format!("index {index} is out of bounds for table size {table_size}"),
    )
}

pub(crate) fn invalid_offset(section: &'static str, offset: u32, file_size: u32) -> DexError {
    malformed(
        section,
        offset as usize,
        format!("offset is outside file bounds (file size: {file_size:#x})"),
    )
}

pub(crate) fn invalid_magic(found: [u8; 8]) -> DexError {
    invalid("dex header", format!("invalid magic bytes: {found:?}"))
}

pub(crate) fn checksum_mismatch(expected: u32, computed: u32) -> DexError {
    invalid(
        "dex header",
        format!("checksum mismatch: expected {expected:#010x}, computed {computed:#010x}"),
    )
}

pub(crate) fn signature_mismatch() -> DexError {
    invalid("dex header", "signature mismatch")
}

pub(crate) fn invalid_method_handle_type(value: u16) -> DexError {
    invalid(
        "method handle",
        format!("invalid method handle type {value}"),
    )
}

pub(crate) fn invalid_call_site(index: u32, detail: impl Into<String>) -> DexError {
    malformed("call site", index as usize, detail)
}

pub(crate) fn invalid_hidden_api_flag(value: u32) -> DexError {
    invalid("hidden api", format!("invalid hidden API flag {value}"))
}

pub(crate) fn invalid_annotation_visibility(value: u8) -> DexError {
    invalid(
        "annotation",
        format!("invalid annotation visibility {value}"),
    )
}

pub(crate) fn invalid_encoded_value_type(type_byte: u8) -> DexError {
    invalid(
        "encoded value",
        format!("invalid encoded value type {type_byte:#04x}"),
    )
}
