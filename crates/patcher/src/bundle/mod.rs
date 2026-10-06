// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `.reseam` bundles: a zip whose Ed25519-signed `manifest.toml` contains the
//! static patch catalog and every payload file's SHA-256. Whether to trust the signing
//! key is the host's decision; this module only verifies that the bundle is
//! intact and signed by the key it carries.

mod archive;
#[cfg(feature = "bridge")]
pub(crate) mod declarations;
pub(crate) mod index;
mod pack;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use reseam_storage::ScratchDir;
use serde::{Deserialize, Serialize};

use crate::error::PatcherError;
use crate::patch::{Patch, PatchSpec, is_slug};

pub use archive::BundleArchive;
pub(crate) use pack::copy_hashed;
pub use pack::pack;

pub const BUNDLE_MIMETYPE: &str = "application/vnd.reseam.bundle";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
pub(crate) use index::PATCH_INDEX;
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

const CONTROL_ENTRIES: [&str; 4] = [
    "mimetype",
    "manifest.toml",
    "manifest.pubkey",
    "manifest.sig",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub format_version: u32,
    /// Engine version the bundle was packed with. Semver: bundles work with
    /// engines of the same major (same minor while the major is 0).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub engine: String,
}

fn check_engine(info: &BundleInfo) -> crate::error::Result<()> {
    let line = |version: &str| -> crate::error::Result<(u32, u32)> {
        let mut parts = version.split('.').map(str::parse::<u32>);
        match (parts.next(), parts.next()) {
            (Some(Ok(0)), Some(Ok(minor))) => Ok((0, minor)),
            (Some(Ok(major)), Some(Ok(_))) => Ok((major, 0)),
            _ => Err(bundle_error(format!("invalid engine version `{version}`"))),
        }
    };
    let ordering = if info.engine.is_empty() {
        std::cmp::Ordering::Less
    } else {
        line(&info.engine)?.cmp(&line(ENGINE_VERSION)?)
    };
    match ordering {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(PatcherError::EngineTooOld {
            bundle: info.name.clone(),
            built: info.engine.clone(),
            running: ENGINE_VERSION.to_owned(),
        }),
        std::cmp::Ordering::Less => Err(PatcherError::BundleTooOld {
            bundle: info.name.clone(),
            built: info.engine.clone(),
            running: ENGINE_VERSION.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize, Deserialize)]
struct BundleManifest {
    bundle: BundleInfo,
    patches: Vec<PatchSpec>,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct ManifestHeader {
    bundle: BundleInfo,
}

impl BundleManifest {
    fn check_patches(&self) -> crate::error::Result<()> {
        for patch in &self.patches {
            if patch.bundle != self.bundle.name || patch.id.is_empty() {
                return Err(bundle_error(format!(
                    "invalid patch identity {}/{} in bundle {}",
                    patch.bundle, patch.id, self.bundle.name
                )));
            }
        }
        crate::engine::PatchIndex::new(&self.patches.iter().collect::<Vec<_>>())?;
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PayloadKind {
    Jar,
    Dex,
    Resource,
}

fn payload_kind(name: &str) -> crate::error::Result<PayloadKind> {
    // Use portable names: Windows interprets backslashes, drive prefixes and
    // device names even when the archive was created on Unix.
    if name.split('/').any(|part| {
        part.is_empty()
            || part == "."
            || part == ".."
            || part.contains(['\\', ':', '\0'])
            || part.ends_with(['.', ' '])
            || part.chars().any(char::is_control)
            || matches!(
                part.split('.')
                    .next()
                    .unwrap_or("")
                    .to_ascii_uppercase()
                    .as_str(),
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
    }) {
        return Err(bundle_error(format!("unsafe payload path {name}")));
    }
    if name.starts_with("resources/") {
        Ok(PayloadKind::Resource)
    } else if !name.contains('/')
        && Path::new(name)
            .extension()
            .is_some_and(|extension| extension == "jar")
    {
        Ok(PayloadKind::Jar)
    } else if !name.contains('/')
        && Path::new(name)
            .extension()
            .is_some_and(|extension| extension == "dex")
    {
        Ok(PayloadKind::Dex)
    } else {
        Err(bundle_error(format!("unsupported payload {name}")))
    }
}

/// Reads and validates a staging manifest without loading or executing patches.
/// The packer supplies the engine version and payload hashes when signing it.
pub fn manifest_info(path: &Path) -> crate::error::Result<BundleInfo> {
    Ok(read_manifest(path)?.bundle)
}

fn read_manifest(path: &Path) -> crate::error::Result<ManifestHeader> {
    let manifest: ManifestHeader = toml::from_str(&std::fs::read_to_string(path)?)?;
    check_info(&manifest.bundle)?;
    Ok(manifest)
}

fn check_info(info: &BundleInfo) -> crate::error::Result<()> {
    check_name(&info.name)?;
    if info.format_version != BUNDLE_FORMAT_VERSION {
        return Err(bundle_error(format!(
            "unsupported bundle format_version {} (supported: {BUNDLE_FORMAT_VERSION})",
            info.format_version
        )));
    }
    Ok(())
}

/// A loaded bundle. The payload lives in a scratch directory for as long as
/// the bundle does, which is what keeps the patches' extension DEX readable.
pub struct PatchBundle {
    info: BundleInfo,
    public_key: [u8; 32],
    patches: Vec<Patch>,
    extension_dex: Vec<PathBuf>,
    _extracted: std::sync::Arc<ScratchDir>,
}

impl PatchBundle {
    pub fn info(&self) -> &BundleInfo {
        &self.info
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// Takes the patches. Each retains its class loader and bundle files until
    /// it is dropped, including resources and classes loaded during execution.
    pub fn into_patches(self) -> Vec<Patch> {
        self.patches
    }

    /// Extension files remain immutable and readable for the bundle's lifetime.
    pub fn extension_dex(&self) -> &[PathBuf] {
        &self.extension_dex
    }
}

fn bundle_error(reason: impl Into<String>) -> PatcherError {
    PatcherError::Bundle(reason.into())
}

fn check_name(name: &str) -> crate::error::Result<()> {
    if is_slug(name) {
        Ok(())
    } else {
        Err(bundle_error(format!(
            "bundle name '{name}' must be lowercase letters, digits, and hyphens"
        )))
    }
}
