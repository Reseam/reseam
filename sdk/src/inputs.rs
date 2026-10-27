// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use reseam_model::{PatchPhase, PatchPhaseMetrics};

use crate::apk::OpenedApk;
use crate::bundles::OpenedBundle;
use crate::error::Result;
use crate::metrics::measure;

/// The APK and bundles of one request, each opened once.
pub(crate) struct Inputs {
    pub(crate) apk: Option<OpenedApk>,
    /// Opening the APK, the first phase of a run that patches it.
    pub(crate) opening: PatchPhaseMetrics,
    pub(crate) bundles: Vec<OpenedBundle>,
}

impl Inputs {
    /// Fails when the APK cannot be opened; a bundle that cannot is kept with its error.
    pub(crate) fn open(
        apk_path: Option<&str>,
        split_paths: &[String],
        bundle_paths: &[String],
    ) -> Result<Self> {
        let splits: Vec<PathBuf> = split_paths.iter().map(PathBuf::from).collect();
        let (apk, opening) = measure(PatchPhase::OpenApk, || {
            apk_path
                .map(|path| OpenedApk::open(Path::new(path), &splits))
                .transpose()
        })?;
        Ok(Self {
            apk,
            opening,
            bundles: bundle_paths
                .iter()
                .map(|path| OpenedBundle::open(PathBuf::from(path)))
                .collect(),
        })
    }
}
