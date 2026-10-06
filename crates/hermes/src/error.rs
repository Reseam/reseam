// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use thiserror::Error;

/// A malformed file, unsupported operation, or output failure.
#[derive(Debug, Error)]
pub enum HermesError {
    #[error("not a Hermes bytecode file")]
    Magic,
    #[error("unsupported Hermes bytecode version {0}; supported: 98")]
    Version(u32),
    #[error("invalid Hermes bytecode at offset {offset}: {reason}")]
    Invalid { offset: usize, reason: String },
    #[error("unsupported Hermes operation: {0}")]
    Unsupported(String),
    #[error("Hermes lookup matched {count} functions: {query}")]
    Match { query: String, count: usize },
    #[error("Hermes output: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, HermesError>;

pub(crate) fn invalid(offset: usize, reason: impl Into<String>) -> HermesError {
    HermesError::Invalid {
        offset,
        reason: reason.into(),
    }
}
