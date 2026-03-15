use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatcherError {
    #[error("APK error: {0}")]
    Apk(#[from] stitch_apk::error::ApkError),

    #[error("Patch failed: {name}: {reason}")]
    PatchFailed { name: String, reason: String },

    #[error("Bundle error: {reason}")]
    BundleError { reason: String },

    #[cfg(feature = "lua")]
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PatcherError>;
