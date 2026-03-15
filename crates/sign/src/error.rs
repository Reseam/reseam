use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto error: {reason}")]
    Crypto { reason: String },

    #[error("invalid APK signing block: {reason}")]
    InvalidSigningBlock { reason: String },

    #[error("invalid APK: {reason}")]
    InvalidApk { reason: String },

    #[error("key error: {reason}")]
    Key { reason: String },
}

pub type Result<T> = std::result::Result<T, SignError>;
