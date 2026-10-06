// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use crate::error::{HostError, Result, classify};
use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::{ApkFile, ContainerBundle, ContainerFormat};
use reseam_patcher::bundle::{BundleArchive, PatchBundle};
use reseam_patcher::{PatchPreset, PatchSpec};

use crate::error::Problem;
use crate::trust::TrustStore;
use crate::{ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata};

/// Reads app and bytecode metadata with the same input tolerance as patching.
/// Unreadable manifests, resources, or DEX data are reported as APK failures.
pub fn inspect_apk(apk_path: &Path, split_paths: &[PathBuf]) -> Result<ApkMetadata> {
    // Same tolerance as patching: a repacked APK (stale DEX checksums and
    // signatures) is still an APK, and inspection exists to read it, not to
    // certify it.
    let mut opened = open_apk(apk_path, split_paths, ApkFile::patch_options())?;
    apk_metadata(&mut opened)
}

pub(crate) fn apk_metadata(opened: &mut OpenedApk) -> Result<ApkMetadata> {
    let application_label = opened
        .apk
        .application_label()
        .map_err(|source| HostError::Apk {
            path: opened.input.clone(),
            source,
        })?;
    let apk = &opened.apk;
    let dex_files = apk.dex();
    Ok(ApkMetadata {
        application_label,
        package_name: apk.package_name().map(Into::into),
        version_name: apk.version_name().map(Into::into),
        version_code: apk.version_code(),
        bundle_kind: opened.bundle,
        dex_files: dex_files.len(),
        component_count: apk.components().len(),
        split_names: apk.components()[1..]
            .iter()
            .map(|component| component.name().to_string())
            .collect(),
        class_count: dex_files.iter().map(|dex| dex.classes().len()).sum(),
        method_count: dex_files.iter().map(|dex| dex.methods().len()).sum(),
    })
}

/// Inspects an optional APK and all requested bundles.
/// APK and trust failures abort inspection; each bundle failure is recorded in
/// its metadata so hosts can display the readable bundles alongside it.
/// Reads signed patch metadata without loading bundle code, regardless of trust.
pub fn inspect(request: &InspectRequest) -> Result<InspectResponse> {
    Ok(PreparedInspection::open(request)?.metadata)
}

/// Retains the opened APK and verified catalogs between inspection and one patch
/// run. Input files must remain unchanged until this object is consumed or dropped,
/// as with [`ApkInspection`]. Inspection never loads executable bundle payloads.
pub struct PreparedInspection {
    request: InspectRequest,
    pub(crate) opened: Option<OpenedApk>,
    archives: Vec<(PathBuf, Result<BundleArchive>)>,
    metadata: InspectResponse,
}

impl PreparedInspection {
    pub fn open(request: &InspectRequest) -> Result<Self> {
        let trust = TrustStore::from_hex(&request.trust.keys)?;
        let splits = request
            .split_paths
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let mut opened = request
            .apk_path
            .as_deref()
            .map(|path| open_apk(Path::new(path), &splits, ApkFile::patch_options()))
            .transpose()?;
        let apk = opened.as_mut().map(apk_metadata).transpose()?;
        let mut bundles = Vec::with_capacity(request.bundle_paths.len());
        let mut patches = Vec::new();
        let archives: Vec<_> = request
            .bundle_paths
            .iter()
            .map(|path| {
                let path = PathBuf::from(path);
                let archive = open_bundle(&path);
                (path, archive)
            })
            .collect();
        for (path, result) in &archives {
            let archive = match result {
                Ok(archive) => archive,
                Err(error) => {
                    bundles.push(unreadable_bundle(path, error));
                    continue;
                }
            };
            patches.extend(
                archive
                    .patches()
                    .iter()
                    .map(|spec| patch_metadata(spec, apk.as_ref())),
            );
            bundles.push(bundle_metadata(path, archive, &trust));
        }
        Ok(Self {
            request: request.clone(),
            opened,
            archives,
            metadata: InspectResponse {
                apk,
                bundles,
                patches,
            },
        })
    }

    pub fn metadata(&self) -> &InspectResponse {
        &self.metadata
    }

    /// Consumes the inspected inputs. Paths and their ordering must match inspection;
    /// selection, signer approvals, signing identity and destination may change.
    /// Trust is checked again and every payload hash is verified before execution.
    pub fn patch(
        self,
        request: &crate::PatchRequest,
        emit: impl FnMut(crate::RunEvent),
    ) -> Result<crate::PatchOutcome> {
        self.check_request(request)?;
        crate::run::patch_with_inputs(
            request,
            Some(self),
            |_, _| Ok(request.selection.clone()),
            emit,
        )
    }

    fn check_request(&self, request: &crate::PatchRequest) -> Result<()> {
        if self.request.apk_path.as_deref() != Some(request.apk_path.as_str())
            || self.request.split_paths != request.split_paths
            || self.request.bundle_paths != request.bundle_paths
        {
            return Err(HostError::InvalidRequest(
                "patch inputs differ from the prepared inspection",
            ));
        }
        Ok(())
    }

    pub(crate) fn load_bundles(self, trust: &TrustStore) -> Result<Vec<PatchBundle>> {
        if self.archives.is_empty() {
            return Err(HostError::InvalidRequest("at least one bundle is required"));
        }
        self.archives
            .into_iter()
            .map(|(path, archive)| load_archive(&path, archive?, trust))
            .collect()
    }
}

fn unreadable_bundle(path: &Path, error: &(dyn std::error::Error + 'static)) -> BundleMetadata {
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

fn load_problem(path: &Path, error: &(dyn std::error::Error + 'static)) -> Problem {
    match classify(error) {
        Problem::Other | Problem::UnreadableApk { .. } => Problem::unreadable_bundle(path),
        problem => problem,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) struct OpenedApk {
    pub apk: ApkFile,
    pub bundle: Option<ContainerFormat>,
    input: PathBuf,
}

pub(crate) fn open_apk(
    apk_path: &Path,
    split_paths: &[PathBuf],
    options: ParseOptions,
) -> Result<OpenedApk> {
    let bundle = ContainerBundle::open(apk_path).map_err(|source| HostError::Apk {
        path: apk_path.to_owned(),
        source,
    })?;
    if bundle.is_some() && !split_paths.is_empty() {
        return Err(HostError::InvalidRequest(
            "split files cannot be combined with an APKM/XAPK container",
        ));
    }
    let format = bundle.as_ref().map(ContainerBundle::format);
    let apk = match bundle {
        Some(bundle) => bundle.into_apk(options),
        None => ApkFile::open_split(apk_path, split_paths, options),
    }
    .map_err(|source| HostError::Apk {
        path: apk_path.to_owned(),
        source,
    })?;
    Ok(OpenedApk {
        apk,
        bundle: format,
        input: apk_path.to_owned(),
    })
}

/// Loads bundles signed by a key in `trust`; anything else is an error.
pub fn load_bundles(paths: &[PathBuf], trust: &TrustStore) -> Result<Vec<PatchBundle>> {
    if paths.is_empty() {
        return Err(HostError::InvalidRequest("at least one bundle is required"));
    }
    paths
        .iter()
        .map(|path| {
            let archive = open_bundle(path)?;
            load_archive(path, archive, trust)
        })
        .collect()
}

fn load_archive(path: &Path, archive: BundleArchive, trust: &TrustStore) -> Result<PatchBundle> {
    if !trust.contains(archive.public_key()) {
        return Err(Problem::UntrustedBundle {
            path: path.display().to_string(),
            public_key: hex::encode(archive.public_key()),
        }
        .into());
    }
    archive.load().map_err(|source| HostError::Bundle {
        path: path.to_owned(),
        source,
    })
}

fn open_bundle(path: &Path) -> Result<BundleArchive> {
    BundleArchive::open(path).map_err(|source| HostError::Bundle {
        path: path.to_owned(),
        source,
    })
}

fn bundle_metadata(path: &Path, archive: &BundleArchive, trust: &TrustStore) -> BundleMetadata {
    let info = archive.info();
    BundleMetadata {
        file_name: file_name(path),
        name: info.name.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        files: archive.files().map(str::to_string).collect(),
        public_key: hex::encode(archive.public_key()),
        engine: info.engine.clone(),
        trusted: trust.contains(archive.public_key()),
        problem: None,
    }
}

fn patch_metadata(spec: &PatchSpec, apk: Option<&ApkMetadata>) -> PatchMetadata {
    let package = apk.and_then(|apk| apk.package_name.as_deref());
    PatchMetadata {
        spec: spec.clone(),
        incompatibility: spec
            .incompatibility(package, apk.and_then(|apk| apk.version_name.as_deref())),
        presets: [PatchPreset::Recommended, PatchPreset::All]
            .into_iter()
            .filter(|&preset| spec.in_preset(preset, package))
            .collect(),
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
            opened: open_apk(path, split_paths, ApkFile::patch_options())?,
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
            .map(reseam_apk::ApkComponent::path)
    }

    /// Resolves a bitmap or adaptive icon, or none when no icon can be resolved.
    pub fn application_icon(&mut self) -> Result<Option<reseam_apk::ApplicationIcon>> {
        self.opened
            .apk
            .application_icon()
            .map_err(|source| HostError::Apk {
                path: self.opened.input.clone(),
                source,
            })
    }
}
