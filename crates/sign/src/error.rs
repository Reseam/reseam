// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("invalid {section}: {reason}")]
    Invalid {
        section: &'static str,
        reason: String,
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
