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

    #[error("not found: {0}")]
    NotFound(String),

    #[error("dependency cycle: {}", names.join(" -> "))]
    DependencyCycle { names: Vec<String> },

    #[error("DEX error: {0}")]
    Dex(#[from] stitch_apk::stitch_dex::DexError),

    #[error("APK error: {0}")]
    Apk(#[from] stitch_apk::error::ApkError),

    #[error("JVM error: {reason}")]
    Jvm { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, PatcherError>;
