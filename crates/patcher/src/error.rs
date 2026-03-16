use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatcherError {
    #[error("patch failed: {name}: {reason}")]
    PatchFailed { name: String, reason: String },

    #[error("bundle error: {reason}")]
    Bundle { reason: String },

    #[error("incompatible: patch {patch} requires {expected}, got {actual}")]
    Incompatible {
        patch: String,
        expected: String,
        actual: String,
    },

    #[error("APK error: {0}")]
    Apk(#[from] stitch_apk::error::ApkError),

    #[cfg(feature = "native")]
    #[error("native patch load error: {0}")]
    NativeLoad(#[from] libloading::Error),

    #[error("lua error: {0}")]
    Lua(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

#[cfg(feature = "lua")]
impl From<mlua::Error> for PatcherError {
    fn from(e: mlua::Error) -> Self {
        PatcherError::Lua(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PatcherError>;
