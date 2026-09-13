// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub use reseam_model::{Problem, SdkError};
use reseam_patcher::error::PatcherError;

/// The problem an error chain describes, `Other` when none is recognised.
pub fn classify(error: &anyhow::Error) -> Problem {
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
                }) => Some(Problem::BundleTooOld {
                    bundle: bundle.clone(),
                    built: built.clone(),
                    running: running.clone(),
                }),
                Some(PatcherError::EngineTooOld {
                    bundle,
                    built,
                    running,
                }) => Some(Problem::EngineTooOld {
                    bundle: bundle.clone(),
                    built: built.clone(),
                    running: running.clone(),
                }),
                _ => None,
            }
        })
        .unwrap_or(Problem::Other)
}
pub fn sdk_error(error: &anyhow::Error) -> SdkError {
    SdkError {
        problem: classify(error),
        message: format!("{error:#}"),
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
            classify(&error),
            Problem::BundleTooOld {
                bundle: "b".into(),
                built: "0.3.0".into(),
                running: "0.5.0".into(),
            }
        );
        assert_eq!(classify(&anyhow::anyhow!("boom")), Problem::Other);
        let json = serde_json::to_string(&sdk_error(&error)).unwrap();
        assert!(json.contains("\"type\":\"bundle_too_old\""));
    }
}
