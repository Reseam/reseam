// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What went wrong, in a shape a host can act on. The engine's error text
//! stays alongside as the detail.

use std::path::Path;

use reseam_apk::ApkError;
use reseam_patcher::error::PatcherError;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Error)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    #[error("internal error")]
    Other,
}

impl Problem {
    pub fn unreadable_bundle(path: &Path) -> Self {
        Self::UnreadableBundle {
            path: path.display().to_string(),
        }
    }

    /// The problem an error chain describes, `Other` when none is recognised.
    pub fn classify(error: &anyhow::Error) -> Self {
        error
            .chain()
            .find_map(|cause| {
                if let Some(problem) = cause.downcast_ref::<Problem>() {
                    return Some(problem.clone());
                }
                match cause.downcast_ref::<PatcherError>() {
                    Some(PatcherError::BundleTooOld {
                        bundle,
                        built,
                        running,
                    }) => Some(Self::BundleTooOld {
                        bundle: bundle.clone(),
                        built: built.clone(),
                        running: running.clone(),
                    }),
                    Some(PatcherError::EngineTooOld {
                        bundle,
                        built,
                        running,
                    }) => Some(Self::EngineTooOld {
                        bundle: bundle.clone(),
                        built: built.clone(),
                        running: running.clone(),
                    }),
                    _ => cause
                        .downcast_ref::<ApkError>()
                        .map(|_| Self::UnreadableApk {
                            path: String::new(),
                        }),
                }
            })
            .unwrap_or(Self::Other)
    }
}

/// The JSON a failed export carries: the problem plus the full error text.
#[derive(Debug, Serialize)]
pub struct SdkError {
    pub problem: Problem,
    pub message: String,
}

impl From<&anyhow::Error> for SdkError {
    fn from(error: &anyhow::Error) -> Self {
        Self {
            problem: Problem::classify(error),
            message: format!("{error:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_through_context_layers() {
        let error = anyhow::Error::from(PatcherError::BundleTooOld {
            bundle: "b".into(),
            built: "0.3.0".into(),
            running: "0.5.0".into(),
        })
        .context("failed to open bundle x.reseam");
        assert_eq!(
            Problem::classify(&error),
            Problem::BundleTooOld {
                bundle: "b".into(),
                built: "0.3.0".into(),
                running: "0.5.0".into(),
            }
        );
        assert_eq!(Problem::classify(&anyhow::anyhow!("boom")), Problem::Other);
        let json = serde_json::to_string(&SdkError::from(&error)).unwrap();
        assert!(json.contains("\"type\":\"bundle_too_old\""));
    }
}
