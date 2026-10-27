// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use reseam_model::{BundleMetadata, PatchSpec, Problem};
use reseam_patcher::bundle::{BundleArchive, PatchBundle};

use crate::error::{HostError, Result, classify};
use crate::trust::TrustStore;

/// A bundle whose signed catalog is read, or why it could not be.
pub(crate) struct OpenedBundle {
    path: PathBuf,
    archive: Result<BundleArchive>,
}

impl OpenedBundle {
    pub(crate) fn open(path: PathBuf) -> Self {
        let archive = BundleArchive::open(&path).map_err(|source| HostError::Bundle {
            path: path.clone(),
            source,
        });
        Self { path, archive }
    }

    /// The patches the bundle declares; none when its catalog cannot be read.
    pub(crate) fn catalog(&self) -> &[PatchSpec] {
        self.archive.as_ref().map_or(&[], BundleArchive::patches)
    }

    /// What the bundle's signed manifest says, or the problem reading it.
    pub(crate) fn metadata(&self, trust: &TrustStore) -> BundleMetadata {
        let file_name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let archive = match &self.archive {
            Ok(archive) => archive,
            Err(error) => {
                return BundleMetadata {
                    file_name,
                    name: String::new(),
                    author: String::new(),
                    description: String::new(),
                    license: String::new(),
                    files: Vec::new(),
                    public_key: String::new(),
                    version: String::new(),
                    engine: String::new(),
                    trusted: false,
                    problem: Some(load_problem(&self.path, error)),
                };
            }
        };
        let info = archive.info();
        BundleMetadata {
            file_name,
            name: info.name.clone(),
            author: info.author.clone(),
            description: info.description.clone(),
            license: info.license.clone(),
            files: archive.files().map(str::to_string).collect(),
            public_key: hex::encode(archive.public_key()),
            version: info.version.clone(),
            engine: info.engine.clone(),
            trusted: trust.contains(archive.public_key()),
            problem: None,
        }
    }
}

fn load_problem(path: &Path, error: &HostError) -> Problem {
    match classify(error) {
        Problem::Other | Problem::UnreadableApk { .. } => Problem::unreadable_bundle(path),
        problem => problem,
    }
}

/// Bundles signed by a key in `trust`; anything else is an error.
pub(crate) fn trusted_archives(
    bundles: Vec<OpenedBundle>,
    trust: &TrustStore,
) -> Result<Vec<TrustedArchive>> {
    if bundles.is_empty() {
        return Err(HostError::InvalidRequest("at least one bundle is required"));
    }
    bundles
        .into_iter()
        .map(|bundle| TrustedArchive::new(bundle.path, bundle.archive?, trust))
        .collect()
}

/// A bundle signed by a trusted key whose catalog is read and whose code is not loaded yet.
pub(crate) struct TrustedArchive {
    path: PathBuf,
    archive: BundleArchive,
}

impl TrustedArchive {
    fn new(path: PathBuf, archive: BundleArchive, trust: &TrustStore) -> Result<Self> {
        if !trust.contains(archive.public_key()) {
            return Err(Problem::UntrustedBundle {
                path: path.display().to_string(),
                public_key: hex::encode(archive.public_key()),
            }
            .into());
        }
        Ok(Self { path, archive })
    }

    pub(crate) fn catalog(&self) -> &[PatchSpec] {
        self.archive.patches()
    }

    /// Loads the code of the catalog patches whose ids are in `patches`.
    pub(crate) fn load(self, patches: &HashSet<String>) -> Result<PatchBundle> {
        self.archive
            .load(patches)
            .map_err(|source| HostError::Bundle {
                path: self.path,
                source,
            })
    }
}
