// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `.reseam` bundles: a zip whose `manifest.toml` lists every payload file
//! with its SHA-256 and is signed with Ed25519. Whether to trust the signing
//! key is the host's decision; this module only verifies that the bundle is
//! intact and signed by the key it carries.

mod archive;
mod pack;

use std::collections::BTreeMap;
use std::path::PathBuf;

use reseam_apk::scratch::ScratchDir;
use serde::{Deserialize, Serialize};

use crate::error::PatcherError;
use crate::patch::Patch;

pub use archive::BundleArchive;
pub use pack::pack;

pub const BUNDLE_MIMETYPE: &str = "application/vnd.reseam.bundle";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
/// Version of this engine; bundles record the one that packed them.
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

/// Whether a bundle packed by engine `built` loads on engine `running`.
/// `Ordering` tells the caller which side is behind when it does not.
fn engine_compatibility(
    built: &str,
    running: &str,
) -> std::result::Result<std::cmp::Ordering, String> {
    let line = |version: &str| -> std::result::Result<(u32, u32), String> {
        let mut parts = version.split('.').map(|part| part.parse::<u32>());
        match (parts.next(), parts.next()) {
            (Some(Ok(0)), Some(Ok(minor))) => Ok((0, minor)),
            (Some(Ok(major)), Some(Ok(_))) => Ok((major, 0)),
            _ => Err(format!("invalid engine version `{version}`")),
        }
    };
    Ok(line(built)?.cmp(&line(running)?))
}

fn check_engine(info: &BundleInfo) -> crate::error::Result<()> {
    if info.engine.is_empty() {
        return Err(bundle_error(format!(
            "bundle {} was packed before engines were versioned; ask its author for a build with Reseam {ENGINE_VERSION} or newer",
            info.name
        )));
    }
    match engine_compatibility(&info.engine, ENGINE_VERSION).map_err(bundle_error)? {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(bundle_error(format!(
            "bundle {} needs Reseam engine {} or newer; this is {ENGINE_VERSION}. Update Reseam",
            info.name, info.engine
        ))),
        std::cmp::Ordering::Less => Err(bundle_error(format!(
            "bundle {} was built for Reseam engine {}, which this engine ({ENGINE_VERSION}) no longer loads; ask its author for a rebuild",
            info.name, info.engine
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::engine_compatibility;
    use std::cmp::Ordering;

    #[test]
    fn same_line_is_compatible() {
        assert_eq!(engine_compatibility("0.3.1", "0.3.0"), Ok(Ordering::Equal));
        assert_eq!(engine_compatibility("1.2.0", "1.9.3"), Ok(Ordering::Equal));
    }

    #[test]
    fn minor_breaks_before_one_major_after() {
        assert_eq!(
            engine_compatibility("0.4.0", "0.3.0"),
            Ok(Ordering::Greater)
        );
        assert_eq!(engine_compatibility("0.2.0", "0.3.0"), Ok(Ordering::Less));
        assert_eq!(
            engine_compatibility("2.0.0", "1.9.0"),
            Ok(Ordering::Greater)
        );
        assert_eq!(engine_compatibility("1.0.0", "2.0.0"), Ok(Ordering::Less));
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(engine_compatibility("three", "0.3.0").is_err());
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleManifest {
    bundle: BundleInfo,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

fn is_payload(name: &str) -> bool {
    name.ends_with(".jar") || name.ends_with(".dex")
}

/// A loaded bundle. The payload lives in a scratch directory for as long as
/// the bundle does, which is what keeps the patches' extension DEX readable.
pub struct PatchBundle {
    pub info: BundleInfo,
    pub public_key: [u8; 32],
    pub patches: Vec<Box<dyn Patch>>,
    pub extension_dex: Vec<PathBuf>,
    _extracted: ScratchDir,
}

fn bundle_error(reason: impl Into<String>) -> PatcherError {
    PatcherError::Bundle(reason.into())
}
