use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatcherError {
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

    #[error("APK error: {0}")]
    Apk(#[from] stitch_apk::error::ApkError),

    #[cfg(feature = "lua")]
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PatcherError>;

pub(crate) fn invalid(section: &'static str, reason: impl Into<String>) -> PatcherError {
    PatcherError::Invalid {
        section,
        reason: reason.into(),
    }
}
