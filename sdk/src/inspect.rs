// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{HostError, Result, classify};
use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::{ApkFile, ContainerBundle, ContainerFormat};
use reseam_patcher::bundle::{BundleArchive, PatchBundle};
use reseam_patcher::engine::PatchIndex;
use reseam_patcher::error::PatcherError;
use reseam_patcher::{ApkVersion, PatchPreset, PatchSpec};

use crate::metrics::measure;
use crate::trust::TrustStore;
use reseam_model::{
    ApkMetadata, BundleMetadata, InspectRequest, InspectResponse, PatchMetadata, PatchOutcome,
    PatchPhase, PatchPhaseMetrics, PatchRequest, Problem, RunEvent,
};

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

/// The APK and bundles of one request, each opened once.
pub(crate) struct Inputs {
    pub(crate) apk: Option<OpenedApk>,
    /// Opening the APK, the first phase of a run that patches it.
    pub(crate) opening: PatchPhaseMetrics,
    pub(crate) bundles: Vec<OpenedBundle>,
}

/// A bundle whose signed catalog is read, or why it could not be.
pub(crate) struct OpenedBundle {
    path: PathBuf,
    archive: Result<BundleArchive>,
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
                .map(|path| open_apk(Path::new(path), &splits, ApkFile::patch_options()))
                .transpose()
        })?;
        let bundles = bundle_paths
            .iter()
            .map(|path| {
                let path = PathBuf::from(path);
                let archive = BundleArchive::open(&path).map_err(|source| HostError::Bundle {
                    path: path.clone(),
                    source,
                });
                OpenedBundle { path, archive }
            })
            .collect();
        Ok(Self {
            apk,
            opening,
            bundles,
        })
    }
}

/// Retains the opened APK and verified catalogs between inspection and one patch
/// run. Input files must remain unchanged until this object is consumed or dropped,
/// as with [`ApkInspection`]. Inspection never loads executable bundle payloads.
pub struct PreparedInspection {
    request: InspectRequest,
    inputs: Inputs,
    metadata: InspectResponse,
}

impl PreparedInspection {
    pub fn open(request: &InspectRequest) -> Result<Self> {
        let trust = TrustStore::new(&request.trust)?;
        let mut inputs = Inputs::open(
            request.apk_path.as_deref(),
            &request.split_paths,
            &request.bundle_paths,
        )?;
        let apk = inputs.apk.as_mut().map(apk_metadata).transpose()?;
        let mut bundles = Vec::with_capacity(inputs.bundles.len());
        let mut patches = Vec::new();
        for OpenedBundle { path, archive } in &inputs.bundles {
            let archive = match archive {
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
            inputs,
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

    /// The patch `selector` names, resolved as a [`PatchSelection`](reseam_model::PatchSelection)
    /// resolves it for the inspected package: a reference `<bundle>/<id>`, or an ID
    /// or display name that identifies one patch. `None` when no patch has that name.
    pub fn resolve_patch(&self, selector: &str) -> Result<Option<&PatchMetadata>> {
        let specs: Vec<&PatchSpec> = self
            .metadata
            .patches
            .iter()
            .map(|patch| &patch.spec)
            .collect();
        let package = self
            .metadata
            .apk
            .as_ref()
            .and_then(|apk| apk.package_name.as_deref());
        match PatchIndex::new(&specs)?.resolve(selector, package) {
            Ok(index) => Ok(Some(&self.metadata.patches[index])),
            Err(PatcherError::UnknownPatch(_)) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Consumes the inspected inputs. Paths and their ordering must match inspection;
    /// selection, signer approvals, signing identity and destination may change.
    /// Trust is checked again and every payload hash is verified before execution.
    pub fn patch(self, request: &PatchRequest, emit: impl FnMut(RunEvent)) -> Result<PatchOutcome> {
        if self.request.apk_path.as_deref() != Some(request.apk_path.as_str())
            || self.request.split_paths != request.split_paths
            || self.request.bundle_paths != request.bundle_paths
        {
            return Err(HostError::InvalidRequest(
                "patch inputs differ from the prepared inspection",
            ));
        }
        crate::run::run(request, self.inputs, emit)
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

fn unreadable_bundle(path: &Path, error: &(dyn std::error::Error + 'static)) -> BundleMetadata {
    BundleMetadata {
        file_name: file_name(path),
        name: String::new(),
        author: String::new(),
        description: String::new(),
        license: String::new(),
        files: Vec::new(),
        public_key: String::new(),
        version: String::new(),
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

fn bundle_metadata(path: &Path, archive: &BundleArchive, trust: &TrustStore) -> BundleMetadata {
    let info = archive.info();
    BundleMetadata {
        file_name: file_name(path),
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

fn patch_metadata(spec: &PatchSpec, apk: Option<&ApkMetadata>) -> PatchMetadata {
    let package = apk.and_then(|apk| apk.package_name.as_deref());
    PatchMetadata {
        spec: spec.clone(),
        incompatibility: spec.compatibility.incompatibility(
            package,
            ApkVersion {
                name: apk.and_then(|apk| apk.version_name.as_deref()),
                code: apk.and_then(|apk| apk.version_code),
            },
        ),
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
        // Same tolerance as patching: a repacked APK (stale DEX checksums and
        // signatures) is still an APK, and inspection exists to read it, not to
        // certify it.
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
    pub fn application_icon(&mut self) -> Result<Option<reseam_model::ApplicationIcon>> {
        self.opened
            .apk
            .application_icon()
            .map_err(|source| HostError::Apk {
                path: self.opened.input.clone(),
                source,
            })
    }
}
