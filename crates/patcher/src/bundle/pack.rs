// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_storage::{ScratchDir, copy_hashed};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::manifest::BundleManifest;
use super::stage::{Staged, stage};
use super::{BUNDLE_MIMETYPE, BundleInfo, BundleKey, ENGINE_VERSION, manifest_info};
use crate::error::{BundleError, Result};
use crate::patch::PatchSpec;

/// Packs `dir/manifest.toml`, adjacent `.jar`/`.dex` files, and `resources/` into a
/// signed bundle at `out`. Jars must carry both JVM classes and `classes.dex`
/// so the same bundle runs on the desktop JVM and on ART.
/// Initializes the author's patch declarations to include their static metadata
/// in the signed manifest. Packing therefore requires trusted source code.
/// `version` names the release; development builds pass `None`.
pub fn pack(dir: &Path, version: Option<&str>, key: &BundleKey, out: &Path) -> Result<()> {
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
    let manifest = BundleManifest {
        patches: catalog(&jars, Arc::clone(&snapshot), &bundle)?,
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

/// The specs of the patches `jars` declare, read by loading them.
#[cfg(feature = "kotlin")]
fn catalog(
    jars: &[PathBuf],
    snapshot: Arc<ScratchDir>,
    bundle: &BundleInfo,
) -> Result<Vec<PatchSpec>> {
    let declarations = super::declarations::declarations(jars)?;
    Ok(
        crate::bridge::load_patches(jars, &declarations, snapshot, bundle)?
            .iter()
            .map(|patch| patch.spec().clone())
            .collect(),
    )
}

#[cfg(not(feature = "kotlin"))]
fn catalog(
    jars: &[PathBuf],
    _snapshot: Arc<ScratchDir>,
    _bundle: &BundleInfo,
) -> Result<Vec<PatchSpec>> {
    if jars.is_empty() {
        Ok(Vec::new())
    } else {
        Err(BundleError::NoKotlin.into())
    }
}

/// Writes the signed bundle next to `out`, then moves it into place.
fn write(
    manifest: &[u8],
    payload: &BTreeMap<String, Staged>,
    key: &BundleKey,
    out: &Path,
) -> Result<()> {
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let parent = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut zip = ZipWriter::new(temporary.as_file_mut());
    for (name, options, bytes) in [
        ("mimetype", stored, BUNDLE_MIMETYPE.as_bytes()),
        ("manifest.toml", deflated, manifest),
        ("manifest.pubkey", stored, key.public_key().as_slice()),
        ("manifest.sig", stored, key.sign(manifest).as_slice()),
    ] {
        zip.start_file(name, options)?;
        zip.write_all(bytes)?;
    }
    for (name, staged) in payload {
        let payload_error = |source| BundleError::Payload {
            name: name.clone(),
            source,
        };
        let mut file = File::open(&staged.path).map_err(payload_error)?;
        zip.start_file(name, deflated)?;
        if copy_hashed(&mut file, &mut zip).map_err(payload_error)? != staged.hash {
            return Err(BundleError::PayloadChanged(name.clone()).into());
        }
    }
    zip.finish()?;
    temporary.as_file().sync_all()?;
    temporary.persist(out).map_err(|error| error.error)?;
    Ok(())
}
