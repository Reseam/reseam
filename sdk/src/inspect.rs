// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::ApkFile;
use reseam_patcher::bundle::{BundleArchive, PatchBundle};
use reseam_patcher::PatchSpec;

use crate::dto::{ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata};
use crate::trust::TrustStore;

pub fn inspect_apk(apk_path: &Path, split_paths: &[PathBuf]) -> Result<ApkMetadata> {
    let apk = open_apk(apk_path, split_paths, &ParseOptions::default())?;
    let dex_files = apk.dex();
    Ok(ApkMetadata {
        package_name: apk.package_name().map(Into::into),
        version_name: apk.version_name().map(Into::into),
        version_code: apk.version_code(),
        dex_files: dex_files.len(),
        component_count: apk.components().len(),
        split_names: apk.components()[1..]
            .iter()
            .map(|component| component.name().to_string())
            .collect(),
        class_count: dex_files.iter().map(|dex| dex.classes.len()).sum(),
        method_count: dex_files.iter().map(|dex| dex.methods.len()).sum(),
    })
}

pub fn inspect(request: &InspectRequest) -> Result<InspectResponse> {
    let apk = request
        .apk_path
        .as_deref()
        .map(|path| inspect_apk(path, &request.split_paths))
        .transpose()?;
    let archives = request
        .bundle_paths
        .iter()
        .map(|path| open_bundle(path))
        .collect::<Result<Vec<_>>>()?;
    let bundles: Vec<BundleMetadata> = request
        .bundle_paths
        .iter()
        .zip(&archives)
        .map(|(path, archive)| bundle_metadata(path, archive, &request.trust))
        .collect();
    let mut patches = Vec::new();
    if bundles.iter().all(|bundle| bundle.trusted) {
        for archive in archives {
            let bundle = archive.load()?;
            patches.extend(
                bundle
                    .patches
                    .iter()
                    .map(|patch| patch_metadata(&bundle.info.name, patch.spec(), apk.as_ref())),
            );
        }
    }
    Ok(InspectResponse {
        apk,
        bundles,
        patches,
    })
}

pub(crate) fn open_apk(
    apk_path: &Path,
    split_paths: &[PathBuf],
    options: &ParseOptions,
) -> Result<ApkFile> {
    ApkFile::open_split(apk_path, split_paths, options)
        .with_context(|| format!("failed to open APK {}", apk_path.display()))
}

/// Loads bundles signed by a key in `trust`; anything else is an error.
pub fn load_bundles(paths: &[PathBuf], trust: &TrustStore) -> Result<Vec<PatchBundle>> {
    ensure!(!paths.is_empty(), "at least one bundle is required");
    paths
        .iter()
        .map(|path| {
            let archive = open_bundle(path)?;
            ensure!(
                trust.contains(&archive.public_key),
                "bundle {} is signed by an untrusted key {}",
                path.display(),
                hex::encode(archive.public_key)
            );
            archive
                .load()
                .with_context(|| format!("failed to load bundle {}", path.display()))
        })
        .collect()
}

fn open_bundle(path: &Path) -> Result<BundleArchive> {
    BundleArchive::open(path).with_context(|| format!("failed to open bundle {}", path.display()))
}

fn bundle_metadata(path: &Path, archive: &BundleArchive, trust: &TrustStore) -> BundleMetadata {
    let info = archive.info();
    BundleMetadata {
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        name: info.name.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        files: archive.files().map(str::to_string).collect(),
        public_key: hex::encode(archive.public_key),
        engine: info.engine.clone(),
        trusted: trust.contains(&archive.public_key),
    }
}

fn patch_metadata(bundle: &str, spec: &PatchSpec, apk: Option<&ApkMetadata>) -> PatchMetadata {
    PatchMetadata {
        bundle: bundle.to_string(),
        spec: spec.clone(),
        incompatibility: spec.incompatibility(
            apk.and_then(|apk| apk.package_name.as_deref()),
            apk.and_then(|apk| apk.version_name.as_deref()),
        ),
    }
}
