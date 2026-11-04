// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_storage::{ScratchDir, copy_hashed};

use super::container;
use super::manifest::BundleManifest;
use super::stage::{Staged, stage};
use super::{BundleInfo, BundleKey, ENGINE_VERSION, manifest_info};
use crate::error::{BundleError, Result};
use crate::patch::PatchSpec;

/// Packs `dir/manifest.toml`, adjacent `.jar`/`.dex` files, and `resources/` into a
/// bundle at `out`, signed with `key`, or unsigned without one for [`super::sign`]
/// to sign elsewhere. Jars must carry both JVM classes and `classes.dex` so the
/// same bundle runs on the desktop JVM and on ART.
/// Initializes the author's patch declarations to include their static metadata
/// in the manifest. Packing therefore runs the author's code.
/// `version` names the release; development builds pass `None`.
pub fn pack(dir: &Path, version: Option<&str>, key: Option<&BundleKey>, out: &Path) -> Result<()> {
    if version.is_some_and(|version| version.trim().is_empty()) {
        return Err(BundleError::EmptyVersion.into());
    }
    let bundle = BundleInfo {
        version: version.unwrap_or_default().to_owned(),
        engine: ENGINE_VERSION.to_string(),
        ..manifest_info(&dir.join("manifest.toml"))?
    };
    let snapshot = Arc::new(ScratchDir::new("bundle-pack")?);
    let payload = stage(dir, &snapshot)?;
    let jars: Vec<PathBuf> = payload
        .iter()
        .filter(|(name, _)| Path::new(name).extension().is_some_and(|ext| ext == "jar"))
        .map(|(_, staged)| staged.path.clone())
        .collect();
    let Catalog {
        patches,
        fingerprints,
    } = catalog(&payload, &jars, Arc::clone(&snapshot), &bundle)?;
    let manifest = BundleManifest {
        patches,
        fingerprints,
        bundle,
        files: payload
            .iter()
            .map(|(name, staged)| (name.clone(), hex::encode(staged.hash)))
            .collect(),
    };
    manifest.check_patches()?;
    let manifest = toml::to_string(&manifest)
        .map_err(BundleError::from)?
        .into_bytes();
    write(&manifest, &payload, key, out)
}

/// What packing reads from the patch jars: each patch's spec, and its fingerprint by patch id.
#[derive(Default)]
struct Catalog {
    patches: Vec<PatchSpec>,
    fingerprints: BTreeMap<String, String>,
}

/// The specs of the patches `jars` declare, read by loading them, and their fingerprints.
#[cfg(feature = "kotlin")]
fn catalog(
    payload: &BTreeMap<String, Staged>,
    jars: &[PathBuf],
    snapshot: Arc<ScratchDir>,
    bundle: &BundleInfo,
) -> Result<Catalog> {
    let declarations = super::declarations::declarations(jars)?;
    let patches: Vec<PatchSpec> =
        crate::bridge::load_patches(jars, &declarations, snapshot, bundle)?
            .iter()
            .map(|patch| patch.spec().clone())
            .collect();
    let fingerprints = super::fingerprint::fingerprints(payload, &declarations, &patches)?;
    Ok(Catalog {
        patches,
        fingerprints,
    })
}

#[cfg(not(feature = "kotlin"))]
fn catalog(
    _payload: &BTreeMap<String, Staged>,
    jars: &[PathBuf],
    _snapshot: Arc<ScratchDir>,
    _bundle: &BundleInfo,
) -> Result<Catalog> {
    if jars.is_empty() {
        Ok(Catalog::default())
    } else {
        Err(BundleError::NoKotlin.into())
    }
}

/// Writes the bundle, copying each staged payload file unless it changed since staging.
fn write(
    manifest: &[u8],
    payload: &BTreeMap<String, Staged>,
    key: Option<&BundleKey>,
    out: &Path,
) -> Result<()> {
    container::write(out, manifest, key, |zip| {
        for (name, staged) in payload {
            let payload_error = |source| BundleError::Payload {
                name: name.clone(),
                source,
            };
            let mut file = File::open(&staged.path).map_err(payload_error)?;
            zip.start_file(name, container::deflated())?;
            if copy_hashed(&mut file, zip).map_err(payload_error)? != staged.hash {
                return Err(BundleError::PayloadChanged(name.clone()).into());
            }
        }
        Ok(())
    })
}
