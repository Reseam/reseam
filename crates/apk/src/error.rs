use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApkError {
    #[error("ZIP error: {0}")]
    Zip(#[from] ::zip::result::ZipError),

    #[error("DEX error: {0}")]
    Dex(#[from] stitch_dex::DexError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid APK: {reason}")]
    InvalidApk { reason: String },

    #[error("Binary XML error: {reason}")]
    AxmlError { reason: String },

    #[error("Resource table error: {reason}")]
    ResourceError { reason: String },

    #[error("Split APK error: {reason}")]
    SplitApk { reason: String },
}

pub type Result<T> = std::result::Result<T, ApkError>;
