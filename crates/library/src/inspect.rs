// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use reseam_apk::ApkFile;
use reseam_patcher::bundle::{BundleInspection, PatchBundle, TRUSTED_KEYS};
use reseam_patcher::options::OptionDeclaration;
use reseam_patcher::patch::Patch;

use crate::dto::{
    ApkMetadata, BundleMetadata, CompatibilityMetadata, InspectResponse, OptionKind,
    OptionMetadata, PatchMetadata, TrustStatus, TrustStore,
};

pub fn built_in_trust_store() -> TrustStore {
    TrustStore::new(TRUSTED_KEYS.iter().copied())
}

pub fn load_bundle_with_trust(bundle_path: &Path, trust_store: &TrustStore) -> Result<PatchBundle> {
    PatchBundle::load_with_trust_anchors(bundle_path, trust_store.keys())
        .with_context(|| format!("failed to load patch bundle {}", bundle_path.display()))
}

pub fn inspect_apk(apk_path: &Path, split_paths: &[PathBuf]) -> Result<ApkMetadata> {
    let apk = open_apk(apk_path, split_paths)?;
    Ok(apk_metadata(&apk))
}

pub fn inspect_with_trust(
    bundle_paths: &[PathBuf],
    apk_path: Option<&Path>,
    split_paths: &[PathBuf],
    trust_store: &TrustStore,
) -> Result<InspectResponse> {
    if bundle_paths.is_empty() {
        bail!("at least one bundle is required");
    }

    let apk = match apk_path {
        Some(path) => Some(inspect_apk(path, split_paths)?),
        None => None,
    };

    let inspections = bundle_paths
        .iter()
        .map(|bundle_path| inspect_bundle_file(bundle_path, trust_store))
        .collect::<Result<Vec<_>>>()?;
    let requires_trust = inspections
        .iter()
        .any(|bundle| bundle.trust_status == TrustStatus::Unknown);

    if requires_trust {
        return Ok(InspectResponse {
            apk,
            bundles: inspections,
            patches: Vec::new(),
            requires_trust: true,
        });
    }

    let loaded_bundles = bundle_paths
        .iter()
        .map(|bundle_path| load_bundle_with_trust(bundle_path, trust_store))
        .collect::<Result<Vec<_>>>()?;
    let bundles = loaded_bundles
        .iter()
        .zip(inspections)
        .map(|(bundle, inspection)| BundleMetadata {
            extension_dex: bundle
                .extension_dex
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            ..inspection
        })
        .collect::<Vec<_>>();
    let patches = loaded_bundles
        .iter()
        .flat_map(|bundle| {
            bundle
                .patches
                .iter()
                .map(|patch| patch_metadata(&bundle.name, patch.as_ref(), apk.as_ref()))
        })
        .collect::<Vec<_>>();

    Ok(InspectResponse {
        apk,
        bundles,
        patches,
        requires_trust: false,
    })
}

pub(crate) fn open_apk(apk_path: &Path, split_paths: &[PathBuf]) -> Result<ApkFile> {
    if split_paths.is_empty() {
        ApkFile::open(apk_path)
            .with_context(|| format!("failed to open APK {}", apk_path.display()))
    } else {
        ApkFile::open_split(apk_path, split_paths)
            .with_context(|| format!("failed to open split APK set {}", apk_path.display()))
    }
}

pub(crate) fn open_patch_apk(apk_path: &Path, split_paths: &[PathBuf]) -> Result<ApkFile> {
    if split_paths.is_empty() {
        ApkFile::open_for_patching(apk_path)
            .with_context(|| format!("failed to open APK {}", apk_path.display()))
    } else {
        ApkFile::open_split_for_patching(apk_path, split_paths)
            .with_context(|| format!("failed to open split APK set {}", apk_path.display()))
    }
}

fn inspect_bundle_file(bundle_path: &Path, trust_store: &TrustStore) -> Result<BundleMetadata> {
    let inspection = PatchBundle::inspect(bundle_path)
        .with_context(|| format!("failed to load patch bundle {}", bundle_path.display()))?;
    Ok(bundle_metadata_from_inspection(
        bundle_path,
        inspection,
        trust_store,
    ))
}

fn bundle_metadata_from_inspection(
    bundle_path: &Path,
    inspection: BundleInspection,
    trust_store: &TrustStore,
) -> BundleMetadata {
    let signer_public_key_hex = hex::encode(inspection.public_key);
    let trust_status = if trust_store.contains(&inspection.public_key) {
        TrustStatus::Trusted
    } else {
        TrustStatus::Unknown
    };

    BundleMetadata {
        file_name: bundle_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("bundle.reseam")
            .to_string(),
        name: inspection.name,
        author: inspection.author,
        description: inspection.description,
        extension_dex: Vec::new(),
        signer_fingerprint: signer_public_key_hex.clone(),
        signer_public_key_hex,
        trust_status,
    }
}

fn apk_metadata(apk: &ApkFile) -> ApkMetadata {
    let dex_files = apk.dex();
    let class_count = dex_files.iter().map(|dex| dex.classes.len()).sum();
    let method_count = dex_files.iter().map(|dex| dex.methods.len()).sum();

    ApkMetadata {
        package_name: apk.package_name().map(str::to_string),
        version_name: apk.version_name().map(str::to_string),
        version_code: apk.version_code(),
        dex_files: dex_files.len(),
        component_count: apk.component_count(),
        split_names: apk.split_names().into_iter().map(str::to_string).collect(),
        class_count,
        method_count,
    }
}

fn option_metadata(option: &OptionDeclaration) -> OptionMetadata {
    OptionMetadata {
        key: option.key.to_string(),
        title: option.title.clone(),
        description: option.description.clone(),
        option_type: OptionKind::from(&option.option_type),
        default_value: option.default_value.as_ref().map(Into::into),
        valid_values: option.valid_values.clone(),
        required: option.required,
    }
}

fn patch_metadata(
    bundle_name: &str,
    patch: &dyn Patch,
    apk: Option<&ApkMetadata>,
) -> PatchMetadata {
    let spec = patch.spec();
    let incompatibility_reason = spec.compatibility_reason(
        apk.and_then(|meta| meta.package_name.as_deref()),
        apk.and_then(|meta| meta.version_name.as_deref()),
    );

    PatchMetadata {
        source_bundle: bundle_name.to_string(),
        name: patch.name().to_string(),
        description: spec.description.to_string(),
        enabled_by_default: spec.enabled_by_default,
        dependencies: spec.dependencies.iter().map(ToString::to_string).collect(),
        compatible_with: spec
            .compatibility
            .iter()
            .map(|entry| CompatibilityMetadata {
                package_name: entry.package.to_string(),
                versions: entry.versions.iter().map(ToString::to_string).collect(),
            })
            .collect(),
        options: spec.options.iter().map(option_metadata).collect(),
        is_compatible: incompatibility_reason.is_none(),
        incompatibility_reason,
    }
}
