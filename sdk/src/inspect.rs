// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::{ApkFile, ContainerBundle};
use reseam_patcher::bundle::{BundleArchive, PatchBundle};
use reseam_patcher::PatchSpec;

use crate::dto::{ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata};
use crate::error::Problem;
use crate::trust::TrustStore;

pub fn inspect_apk(apk_path: &Path, split_paths: &[PathBuf]) -> Result<ApkMetadata> {
    // Same tolerance as patching: a repacked APK (stale DEX checksums and
    // signatures) is still an APK, and inspection exists to read it, not to
    // certify it.
    let mut opened = open_apk(apk_path, split_paths, &ApkFile::patch_options())?;
    apk_metadata(&mut opened)
}

pub(crate) fn apk_metadata(opened: &mut OpenedApk) -> Result<ApkMetadata> {
    let application_label = opened.apk.application_label()?;
    let apk = &opened.apk;
    let dex_files = apk.dex();
    Ok(ApkMetadata {
        application_label,
        package_name: apk.package_name().map(Into::into),
        version_name: apk.version_name().map(Into::into),
        version_code: apk.version_code(),
        bundle_kind: opened.bundle.as_ref().map(ContainerBundle::format),
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
    let mut bundles = Vec::with_capacity(request.bundle_paths.len());
    let mut patches = Vec::new();
    for path in &request.bundle_paths {
        let archive = match open_bundle(path) {
            Ok(archive) => archive,
            Err(error) => {
                bundles.push(unreadable_bundle(path, &error));
                continue;
            }
        };
        let mut metadata = bundle_metadata(path, &archive, &request.trust);
        if !metadata.trusted {
            metadata.problem = Some(Problem::UntrustedBundle {
                path: path.display().to_string(),
                public_key: metadata.public_key.clone(),
            });
        } else {
            match archive.load() {
                Ok(bundle) => patches.extend(
                    bundle
                        .patches
                        .iter()
                        .map(|patch| patch_metadata(&bundle.info.name, patch.spec(), apk.as_ref())),
                ),
                Err(error) => metadata.problem = Some(load_problem(path, &error.into())),
            }
        }
        bundles.push(metadata);
    }
    Ok(InspectResponse {
        apk,
        bundles,
        patches,
    })
}

fn unreadable_bundle(path: &Path, error: &anyhow::Error) -> BundleMetadata {
    BundleMetadata {
        file_name: file_name(path),
        name: String::new(),
        author: String::new(),
        description: String::new(),
        files: Vec::new(),
        public_key: String::new(),
        engine: String::new(),
        trusted: false,
        problem: Some(load_problem(path, error)),
    }
}

fn load_problem(path: &Path, error: &anyhow::Error) -> Problem {
    match Problem::classify(error) {
        Problem::Other | Problem::UnreadableApk { .. } => Problem::unreadable_bundle(path),
        problem => problem,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// An opened APK plus the container bundle it came from, when the input was
/// an APKM/XAPK file. The bundle keeps the extracted scratch files alive for
/// as long as the opened APK needs them (paths, mmaps, output naming).
pub(crate) struct OpenedApk {
    pub apk: ApkFile,
    pub bundle: Option<ContainerBundle>,
}

pub(crate) fn open_apk(
    apk_path: &Path,
    split_paths: &[PathBuf],
    options: &ParseOptions,
) -> Result<OpenedApk> {
    let bundle = ContainerBundle::open(apk_path)
        .with_context(|| format!("failed to open APK bundle {}", apk_path.display()))
        .context(Problem::unreadable_apk(apk_path))?;
    ensure!(
        bundle.is_none() || split_paths.is_empty(),
        "split files cannot be combined with an APKM/XAPK container"
    );
    let (base, splits) = match &bundle {
        Some(bundle) => (bundle.base_path(), bundle.split_paths()),
        None => (apk_path, split_paths),
    };
    let apk = ApkFile::open_split(base, splits, options)
        .with_context(|| format!("failed to open APK {}", apk_path.display()))
        .context(Problem::unreadable_apk(apk_path))?;
    Ok(OpenedApk { apk, bundle })
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
                Problem::UntrustedBundle {
                    path: path.display().to_string(),
                    public_key: hex::encode(archive.public_key),
                }
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
        file_name: file_name(path),
        name: info.name.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        files: archive.files().map(str::to_string).collect(),
        public_key: hex::encode(archive.public_key),
        engine: info.engine.clone(),
        trusted: trust.contains(&archive.public_key),
        problem: None,
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
