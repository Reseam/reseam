// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{Read, Seek};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reseam_apk::scratch::ScratchDir;
use sha2::{Digest, Sha256};
use tracing::info;
use zip::ZipArchive;

use super::{
    bundle_error, check_engine, is_payload, BundleInfo, BundleManifest, PatchBundle,
    BUNDLE_FORMAT_VERSION, BUNDLE_MIMETYPE, CONTROL_ENTRIES,
};
use crate::error::Result;

/// A bundle whose manifest signature and format version have been checked.
/// Payload hashes are checked when the payload is read by [`Self::load`].
pub struct BundleArchive {
    archive: ZipArchive<File>,
    manifest: BundleManifest,
    pub public_key: [u8; 32],
}

impl BundleArchive {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .map_err(|e| bundle_error(format!("failed to open {}: {e}", path.display())))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| bundle_error(format!("failed to open zip {}: {e}", path.display())))?;

        if read_entry(&mut archive, "mimetype")? != BUNDLE_MIMETYPE.as_bytes() {
            return Err(bundle_error(format!(
                "invalid mimetype marker (expected {BUNDLE_MIMETYPE})"
            )));
        }
        let manifest_bytes = read_entry(&mut archive, "manifest.toml")?;
        let public_key: [u8; 32] = read_entry(&mut archive, "manifest.pubkey")?
            .try_into()
            .map_err(|_| bundle_error("manifest.pubkey must be 32 bytes"))?;
        let signature = Signature::from_slice(&read_entry(&mut archive, "manifest.sig")?)
            .map_err(|e| bundle_error(format!("invalid Ed25519 signature: {e}")))?;
        VerifyingKey::from_bytes(&public_key)
            .map_err(|e| bundle_error(format!("invalid Ed25519 public key: {e}")))?
            .verify(&manifest_bytes, &signature)
            .map_err(|_| bundle_error("manifest signature verification failed"))?;

        let manifest_text = std::str::from_utf8(&manifest_bytes)
            .map_err(|e| bundle_error(format!("manifest.toml not UTF-8: {e}")))?;
        let manifest: BundleManifest = toml::from_str(manifest_text)?;
        if manifest.bundle.format_version != BUNDLE_FORMAT_VERSION {
            return Err(bundle_error(format!(
                "unsupported bundle format_version: {} (patcher supports {})",
                manifest.bundle.format_version, BUNDLE_FORMAT_VERSION
            )));
        }
        check_engine(&manifest.bundle)?;
        Ok(Self {
            archive,
            manifest,
            public_key,
        })
    }

    pub fn info(&self) -> &BundleInfo {
        &self.manifest.bundle
    }

    /// Payload file names as declared by the signed manifest.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.manifest.files.keys().map(String::as_str)
    }

    /// Extracts the payload, checking every file against the manifest, and
    /// loads the patches it contains.
    pub fn load(mut self) -> Result<PatchBundle> {
        info!(bundle = %self.manifest.bundle.name, "loading bundle");
        let extracted = ScratchDir::new("bundle")
            .map_err(|e| bundle_error(format!("failed to create scratch directory: {e}")))?;
        let mut jars = Vec::new();
        let mut extension_dex = Vec::new();
        let mut seen = 0;
        for index in 0..self.archive.len() {
            let mut entry = self.archive.by_index(index)?;
            let name = entry.name().to_string();
            if entry.is_dir() || CONTROL_ENTRIES.contains(&name.as_str()) {
                continue;
            }
            let expected = self.manifest.files.get(&name).ok_or_else(|| {
                bundle_error(format!("file {name} is not declared in manifest [files]"))
            })?;
            let mut contents = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut contents)?;
            if hex::encode(Sha256::digest(&contents)) != *expected {
                return Err(bundle_error(format!("hash mismatch for {name}")));
            }
            seen += 1;
            if !is_payload(&name) {
                continue;
            }
            let out = extracted.path().join(&name);
            std::fs::write(&out, contents)?;
            // ART refuses to load a writable dex file (Android 14 and later).
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o444))?;
            if name.ends_with(".jar") {
                jars.push(out);
            } else {
                extension_dex.push(out);
            }
        }
        if seen != self.manifest.files.len() {
            return Err(bundle_error(
                "manifest declares files missing from the archive",
            ));
        }
        jars.sort();
        extension_dex.sort();

        #[cfg(feature = "kotlin")]
        let patches = crate::kotlin::load_patches(&jars, extracted.path())?;
        #[cfg(not(feature = "kotlin"))]
        let patches = Vec::new();

        info!(
            bundle = %self.manifest.bundle.name,
            patch_count = patches.len(),
            extension_dex_count = extension_dex.len(),
            "bundle loaded"
        );
        Ok(PatchBundle {
            info: self.manifest.bundle,
            public_key: self.public_key,
            patches,
            extension_dex,
            _extracted: extracted,
        })
    }
}

fn read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| bundle_error(format!("required entry `{name}` missing from bundle")))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}
