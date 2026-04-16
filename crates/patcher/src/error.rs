// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

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

    #[error("missing dependency: patch {patch} depends on {dependency}")]
    MissingDependency { patch: String, dependency: String },

    #[error("unknown patch: {0}")]
    UnknownPatch(String),

    #[error("invalid patch selection: {0}")]
    InvalidSelection(String),

    #[error("unknown option: patch {patch} has no option '{key}'")]
    UnknownOption { patch: String, key: String },

    #[error("invalid option value: {patch}.{key}: {reason}")]
    InvalidOptionValue {
        patch: String,
        key: String,
        reason: String,
    },

    #[error("missing required option: {patch}.{key}")]
    MissingRequiredOption { patch: String, key: String },

    #[error("DEX error: {0}")]
    Dex(#[from] reseam_apk::reseam_dex::DexError),

    #[error("APK error: {0}")]
    Apk(#[from] reseam_apk::error::ApkError),

    #[error("JVM error: {reason}")]
    Jvm { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, PatcherError>;
