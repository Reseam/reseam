// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::{ApkFile, ContainerBundle};
use reseam_patcher::bundle::{BundleArchive, PatchBundle};
use reseam_patcher::PatchSpec;

use crate::error::Problem;
use crate::trust::TrustStore;
use crate::{ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata};

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
    let trust = TrustStore::from_hex(&request.trust.keys).map_err(anyhow::Error::msg)?;
    let splits = request
        .split_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let apk = request
        .apk_path
        .as_deref()
        .map(|path| inspect_apk(Path::new(path), &splits))
        .transpose()?;
    let mut bundles = Vec::with_capacity(request.bundle_paths.len());
    let mut patches = Vec::new();
    for path in &request.bundle_paths {
        let path = Path::new(path);
        let archive = match open_bundle(path) {
            Ok(archive) => archive,
            Err(error) => {
                bundles.push(unreadable_bundle(path, &error));
                continue;
            }
        };
        let mut metadata = bundle_metadata(path, &archive, &trust);
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
                        .map(|patch| patch_metadata(patch.spec(), apk.as_ref())),
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
    match crate::error::classify(error) {
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
    let bundle =
        ContainerBundle::open(apk_path).map_err(|error| unreadable_apk(apk_path, error.into()))?;
    ensure!(
        bundle.is_none() || split_paths.is_empty(),
        "split files cannot be combined with an APKM/XAPK container"
    );
    let (base, splits) = match &bundle {
        Some(bundle) => (bundle.base_path(), bundle.split_paths()),
        None => (apk_path, split_paths),
    };
    let apk = ApkFile::open_split(base, splits, options)
        .map_err(|error| unreadable_apk(apk_path, error.into()))?;
    Ok(OpenedApk { apk, bundle })
}

/// The problem is the root cause so `classify` finds it; the engine's own text stays as the detail.
fn unreadable_apk(path: &Path, error: anyhow::Error) -> anyhow::Error {
    anyhow::Error::new(Problem::unreadable_apk(path))
        .context(format!("failed to open APK {}: {error:#}", path.display()))
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

fn patch_metadata(spec: &PatchSpec, apk: Option<&ApkMetadata>) -> PatchMetadata {
    PatchMetadata {
        spec: spec.clone(),
        incompatibility: spec.incompatibility(
            apk.and_then(|apk| apk.package_name.as_deref()),
            apk.and_then(|apk| apk.version_name.as_deref()),
        ),
    }
}

/// An opened APK whose extracted component files remain valid until it is dropped.
///
/// Owns the container extraction directory. Consumers must finish reading component
/// paths before dropping this inspection.
pub struct ApkInspection {
    opened: OpenedApk,
}

impl ApkInspection {
    /// Opens an APK or container, returning an error for unreadable input.
    pub fn open(path: &Path, split_paths: &[PathBuf]) -> Result<Self> {
        Ok(Self {
            opened: open_apk(path, split_paths, &ApkFile::patch_options())?,
        })
    }

    /// Reads application and bytecode metadata, propagating malformed-input errors.
    pub fn metadata(&mut self) -> Result<ApkMetadata> {
        apk_metadata(&mut self.opened)
    }

    /// Returns the base component path, valid for this inspection's lifetime.
    pub fn base_path(&self) -> &Path {
        self.opened.apk.base().path()
    }

    /// Returns additional component paths in their original order.
    pub fn split_paths(&self) -> impl Iterator<Item = &Path> {
        self.opened.apk.components()[1..]
            .iter()
            .map(|apk| apk.path())
    }

    /// Resolves a bitmap or adaptive icon, or none when no icon can be resolved.
    pub fn application_icon(&mut self) -> Result<Option<reseam_apk::ApplicationIcon>> {
        Ok(self.opened.apk.application_icon()?)
    }
}
