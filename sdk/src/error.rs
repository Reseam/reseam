// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use reseam_model::{Problem, SdkError};
use reseam_patcher::error::PatcherError;
use thiserror::Error;

/// A host operation failed. Sources retain engine diagnostics and input paths.
#[derive(Debug, Error)]
pub enum HostError {
    #[error("failed to read APK {}: {source}", path.display())]
    Apk {
        path: PathBuf,
        source: reseam_apk::ApkError,
    },
    #[error("failed to write APK {}: {source}", path.display())]
    WriteApk {
        path: PathBuf,
        source: reseam_apk::ApkError,
    },
    #[error("failed to load bundle {}: {source}", path.display())]
    Bundle { path: PathBuf, source: PatcherError },
    #[error(transparent)]
    Patcher(#[from] PatcherError),
    #[error(transparent)]
    Signing(#[from] reseam_sign::SignError),
    #[error(transparent)]
    Problem(#[from] Problem),
    #[error("invalid trusted key `{key}`: {source}")]
    Trust {
        key: String,
        source: hex::FromHexError,
    },
    #[error("{0}")]
    InvalidRequest(&'static str),
    #[error("failed to read DEX code: {0}")]
    Dex(#[from] reseam_dex::DexError),
    #[error("nothing in the APK defines or references {0}")]
    UnknownMember(reseam_model::analysis::Member),
    #[error("the APK has no resource 0x{0:08x}")]
    UnknownResource(u32),
    #[error("the manifest declares no component {0}")]
    UnknownComponent(reseam_model::analysis::ClassRef),
    #[error("{operation} {}: {source}", path.display())]
    File {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    #[error("failed to read the bundle template: {0}")]
    TemplateArchive(#[from] zip::result::ZipError),
    #[error("the bundle template has a file outside its folder: {0}")]
    TemplatePath(String),
    #[error("the bundle template's {file} has no single `{setting}` line")]
    TemplateSetting {
        file: &'static str,
        setting: &'static str,
    },
    #[error("publication failed: {source}; rollback also failed: {rollback}; recover prior outputs from {}", recovery.display())]
    Publication {
        source: Box<HostError>,
        rollback: Box<HostError>,
        recovery: PathBuf,
    },
}

impl From<HostError> for SdkError {
    fn from(error: HostError) -> Self {
        sdk_error(&error)
    }
}

pub type Result<T> = std::result::Result<T, HostError>;

pub(crate) fn file_error(
    operation: &'static str,
    path: &std::path::Path,
    source: std::io::Error,
) -> HostError {
    HostError::File {
        operation,
        path: path.to_owned(),
        source,
    }
}

/// Converts a source chain to the stable problem and diagnostic text hosts receive.
fn sdk_error(error: &(dyn std::error::Error + 'static)) -> SdkError {
    let chain: Vec<_> = std::iter::successors(Some(error), |cause| cause.source()).collect();
    SdkError {
        problem: classify(error),
        message: chain
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(": "),
    }
}

pub(crate) fn classify(error: &(dyn std::error::Error + 'static)) -> Problem {
    std::iter::successors(Some(error), |cause| cause.source())
        .find_map(|cause| {
            if let Some(problem) = cause.downcast_ref::<Problem>() {
                return Some(problem.clone());
            }
            match cause.downcast_ref::<HostError>() {
                Some(HostError::Apk { path, .. }) => return Some(Problem::unreadable_apk(path)),
                Some(HostError::Problem(problem)) => return Some(problem.clone()),
                Some(HostError::Patcher(source)) => return Some(classify(source)),
                Some(HostError::Bundle { source, .. }) => {
                    let problem = classify(source);
                    if problem != Problem::Other {
                        return Some(problem);
                    }
                }
                _ => {}
            }
            match cause.downcast_ref::<PatcherError>() {
                Some(PatcherError::Problem(problem)) => Some(problem.clone()),
                Some(PatcherError::RewriteOutput { patch, .. }) => Some(Problem::PatchesFailed {
                    patches: vec![patch.clone()],
                }),
                _ => None,
            }
        })
        .unwrap_or(Problem::Other)
}
