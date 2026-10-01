// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("malformed {section} at offset {offset}: {reason}")]
    Malformed {
        section: &'static str,
        offset: usize,
        reason: &'static str,
    },

    #[error("invalid {section}: {reason}")]
    Invalid {
        section: &'static str,
        reason: String,
    },

    #[error("cryptographic failure while {operation}: {source}")]
    Crypto {
        operation: &'static str,
        source: rcgen::Error,
    },

    #[error("loading signing identity from {} and {}: {source}", key.display(), cert.display())]
    Identity {
        key: std::path::PathBuf,
        cert: std::path::PathBuf,
        source: Box<SignError>,
    },

    #[error("{operation} {}: {source}", path.display())]
    File {
        operation: &'static str,
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("signing file {} is missing; restore it or delete {} to generate a new pair", missing.display(), existing.display())]
    PartialPair {
        missing: std::path::PathBuf,
        existing: std::path::PathBuf,
    },

    #[error("{operation} at file offset {offset}: {source}")]
    IoAt {
        operation: &'static str,
        offset: u64,
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, SignError>;

pub(crate) fn invalid(section: &'static str, reason: impl Into<String>) -> SignError {
    SignError::Invalid {
        section,
        reason: reason.into(),
    }
}

pub(crate) fn io_at(operation: &'static str, offset: u64, source: std::io::Error) -> SignError {
    SignError::IoAt {
        operation,
        offset,
        source,
    }
}

pub(crate) fn crypto(operation: &'static str, source: rcgen::Error) -> SignError {
    SignError::Crypto { operation, source }
}
