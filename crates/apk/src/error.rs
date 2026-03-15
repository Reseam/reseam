use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApkError {
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

    #[error("ZIP error: {0}")]
    Zip(#[from] ::zip::result::ZipError),

    #[error("DEX error: {0}")]
    Dex(#[from] stitch_dex::DexError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ApkError>;

pub(crate) fn truncated(
    section: &'static str,
    offset: usize,
    needed: usize,
    available: usize,
) -> ApkError {
    ApkError::Truncated {
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
) -> ApkError {
    ApkError::Malformed {
        section,
        offset,
        reason: reason.into(),
    }
}

pub(crate) fn invalid(section: &'static str, reason: impl Into<String>) -> ApkError {
    ApkError::Invalid {
        section,
        reason: reason.into(),
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
