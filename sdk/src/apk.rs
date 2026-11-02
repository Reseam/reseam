// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use reseam_apk::{ApkComponent, ApkFile, ContainerFormat, open_container};
use reseam_model::{ApkMetadata, ApkVersion, ApplicationIcon};

use crate::error::{HostError, Result};

/// An APK, its splits, or an APKM/XAPK container, opened for patching.
pub(crate) struct OpenedApk {
    pub apk: ApkFile,
    pub bundle: Option<ContainerFormat>,
    input: PathBuf,
}

impl OpenedApk {
    // Inspection reads with the same tolerance as patching: a repacked APK
    // (stale DEX checksums and signatures) is still an APK, and inspection
    // exists to read it, not to certify it.
    pub(crate) fn open(apk_path: &Path, split_paths: &[PathBuf]) -> Result<Self> {
        let apk_error = |source| HostError::Apk {
            path: apk_path.to_owned(),
            source,
        };
        let options = ApkFile::patch_options();
        let (bundle, apk) = match open_container(apk_path, options).map_err(apk_error)? {
            Some(_) if !split_paths.is_empty() => {
                return Err(HostError::InvalidRequest(
                    "split files cannot be combined with an APKM/XAPK container",
                ));
            }
            Some((format, apk)) => (Some(format), apk),
            None => (
                None,
                ApkFile::open_split(apk_path, split_paths, options).map_err(apk_error)?,
            ),
        };
        Ok(Self {
            apk,
            bundle,
            input: apk_path.to_owned(),
        })
    }

    pub(crate) fn metadata(&mut self) -> Result<ApkMetadata> {
        let application_label = self
            .apk
            .application_label()
            .map_err(|source| self.error(source))?;
        let apk = &self.apk;
        let dex_files = apk.dex();
        Ok(ApkMetadata {
            application_label,
            package_name: apk.package_name().map(Into::into),
            version_name: apk.version_name().map(Into::into),
            version_code: apk.version_code(),
            bundle_kind: self.bundle,
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

    /// The package and version a plan checks patches against.
    pub(crate) fn identity(&self) -> AppIdentity {
        AppIdentity {
            package: self.apk.package_name().map(Cow::into_owned),
            version_name: self.apk.version_name().map(Cow::into_owned),
            version_code: self.apk.version_code(),
        }
    }

    pub(crate) fn error(&self, source: reseam_apk::ApkError) -> HostError {
        HostError::Apk {
            path: self.input.clone(),
            source,
        }
    }
}

/// What an APK declares itself as, for checking patches against it.
pub(crate) struct AppIdentity {
    pub(crate) package: Option<String>,
    version_name: Option<String>,
    version_code: Option<i32>,
}

impl AppIdentity {
    pub(crate) fn version(&self) -> ApkVersion<'_> {
        ApkVersion {
            name: self.version_name.as_deref(),
            code: self.version_code,
        }
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
            opened: OpenedApk::open(path, split_paths)?,
        })
    }

    /// Reads application and bytecode metadata, propagating malformed-input errors.
    pub fn metadata(&mut self) -> Result<ApkMetadata> {
        self.opened.metadata()
    }

    /// Returns the base component path, valid for this inspection's lifetime.
    pub fn base_path(&self) -> &Path {
        self.opened.apk.base().path()
    }

    /// Returns additional component paths in their original order.
    pub fn split_paths(&self) -> impl Iterator<Item = &Path> {
        self.opened.apk.components()[1..]
            .iter()
            .map(ApkComponent::path)
    }

    /// Resolves a bitmap or adaptive icon, or none when no icon can be resolved.
    pub fn application_icon(&mut self) -> Result<Option<ApplicationIcon>> {
        self.opened
            .apk
            .application_icon()
            .map_err(|source| self.opened.error(source))
    }
}
