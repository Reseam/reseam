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
