use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Crypto error: {reason}")]
    Crypto { reason: String },

    #[error("Invalid APK signing block: {reason}")]
    InvalidSigningBlock { reason: String },
}

pub type Result<T> = std::result::Result<T, SignError>;
