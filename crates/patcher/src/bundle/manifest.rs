// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The signed `manifest.toml`: who the bundle is, which engine line it runs
//! on, its patch catalog and the hash of every payload file.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use reseam_model::Problem;
use serde::{Deserialize, Serialize};

use super::{BUNDLE_FORMAT_VERSION, ENGINE_VERSION};
use crate::catalog::PatchCatalog;
use crate::error::{BundleError, Result};
use crate::patch::PatchSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The license the bundle is under, such as `GPL-3.0`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    pub format_version: u32,
    /// The release this bundle is, set when it is packed. Development builds have none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Engine version the bundle was packed with. Semver: bundles work with
    /// engines of the same major (same minor while the major is 0).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub engine: String,
}

impl BundleInfo {
    pub(super) fn check(&self) -> Result<()> {
        if !is_slug(&self.name) {
            return Err(BundleError::Name(self.name.clone()).into());
        }
        if self.format_version != BUNDLE_FORMAT_VERSION {
            return Err(BundleError::FormatVersion(self.format_version).into());
        }
        Ok(())
    }

    /// Fails unless this engine runs the bundle's engine line.
    pub(super) fn check_engine(&self) -> Result<()> {
        let ordering = if self.engine.is_empty() {
            Ordering::Less
        } else {
            engine_line(&self.engine)?.cmp(&engine_line(ENGINE_VERSION)?)
        };
        let (bundle, built, running) = (
            self.name.clone(),
            self.engine.clone(),
            ENGINE_VERSION.to_owned(),
        );
        match ordering {
            Ordering::Equal => Ok(()),
            Ordering::Greater => Err(Problem::EngineTooOld {
                bundle,
                built,
                running,
            }
            .into()),
            Ordering::Less => Err(Problem::BundleTooOld {
                bundle,
                built,
                running,
            }
            .into()),
        }
    }
}

/// The major version, or the minor while the major is 0.
fn engine_line(version: &str) -> Result<(u32, u32)> {
    let mut parts = version.split('.').map(str::parse::<u32>);
    match (parts.next(), parts.next()) {
        (Some(Ok(0)), Some(Ok(minor))) => Ok((0, minor)),
        (Some(Ok(major)), Some(Ok(_))) => Ok((major, 0)),
        _ => Err(BundleError::EngineVersion(version.to_owned()).into()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct BundleManifest {
    pub(super) bundle: BundleInfo,
    pub(super) patches: Vec<PatchSpec>,
    #[serde(default)]
    pub(super) files: BTreeMap<String, String>,
}

/// The part of a manifest every engine line reads the same way.
#[derive(Deserialize)]
pub(super) struct ManifestHeader {
    pub(super) bundle: BundleInfo,
}

impl BundleManifest {
    pub(super) fn check_patches(&self) -> Result<()> {
        if let Some(patch) = self.patches.iter().find(|patch| {
            patch.reference.bundle != self.bundle.name || patch.reference.id.is_empty()
        }) {
            return Err(BundleError::PatchIdentity {
                bundle: self.bundle.name.clone(),
                patch: patch.reference.clone(),
            }
            .into());
        }
        PatchCatalog::new(&self.patches.iter().collect::<Vec<_>>())?;
        Ok(())
    }
}

/// Reads and validates a staging manifest without loading or executing patches.
/// The packer supplies the engine version and payload hashes when signing it.
pub fn manifest_info(path: &Path) -> Result<BundleInfo> {
    let manifest: ManifestHeader = toml::from_str(&std::fs::read_to_string(path)?)?;
    manifest.bundle.check()?;
    Ok(manifest.bundle)
}

/// A nonempty slug: lowercase letters or digits separated by single hyphens.
pub(crate) fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
