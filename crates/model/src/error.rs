// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Failures in model validation and engine problems reported across the host boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Error, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[boltffi::data]
pub enum Problem {
    #[error("bundle {bundle} was built for engine {built}; this is {running}")]
    BundleTooOld {
        bundle: String,
        built: String,
        running: String,
    },
    #[error("bundle {bundle} needs engine {built}; this is {running}")]
    EngineTooOld {
        bundle: String,
        built: String,
        running: String,
    },
    #[error("bundle {path} is signed by an untrusted key {public_key}")]
    UntrustedBundle { path: String, public_key: String },
    #[error("bundle {path} could not be read")]
    UnreadableBundle { path: String },
    #[error("APK {path} could not be opened")]
    UnreadableApk { path: String },
    #[error("{} patch(es) failed: {}", patches.len(), patches.join(", "))]
    PatchesFailed { patches: Vec<String> },

    #[error("option {key}: expected {expected:?}, got {actual:?}")]
    OptionType {
        key: String,
        expected: crate::OptionType,
        actual: crate::OptionType,
    },
    #[error("option {key}: '{value}' is not in [{}]", allowed.join(", "))]
    OptionChoice {
        key: String,
        value: String,
        allowed: Vec<String>,
    },
    #[error("input has {components} components; single-file output requires one")]
    SingleFileComponents { components: usize },
    #[error("APK has no package name")]
    MissingPackage,
    #[error("incompatible package: {package}")]
    IncompatiblePackage { package: String },
    #[error("unknown preset '{value}'; expected recommended, all or none")]
    UnknownPreset { value: String },
    #[error("internal error")]
    Other,
}

impl Problem {
    pub fn unreadable_bundle(path: &Path) -> Self {
        Self::UnreadableBundle {
            path: path.display().to_string(),
        }
    }

    pub fn unreadable_apk(path: &Path) -> Self {
        Self::UnreadableApk {
            path: path.display().to_string(),
        }
    }
}

/// A typed failure: the problem category and full diagnostic error chain.
#[derive(Debug, Serialize, Deserialize)]
#[boltffi::data]
pub struct SdkError {
    pub problem: Problem,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Problem>;
