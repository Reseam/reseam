// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignError {
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

pub type Result<T> = std::result::Result<T, SignError>;

pub(crate) fn malformed(
    section: &'static str,
    offset: usize,
    reason: impl Into<String>,
) -> SignError {
    SignError::Malformed {
        section,
        offset,
        reason: reason.into(),
    }
}

pub(crate) fn invalid(section: &'static str, reason: impl Into<String>) -> SignError {
    SignError::Invalid {
        section,
        reason: reason.into(),
    }
}

pub(crate) fn internal(operation: &'static str, reason: impl Into<String>) -> SignError {
    SignError::Internal {
        operation,
        reason: reason.into(),
    }
}

pub(crate) fn unsupported(feature: &'static str, detail: impl Into<String>) -> SignError {
    SignError::Unsupported {
        feature,
        detail: detail.into(),
    }
}
