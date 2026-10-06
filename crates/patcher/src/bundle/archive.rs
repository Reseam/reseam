// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reseam_storage::ScratchDir;
use tracing::info;
use zip::ZipArchive;

use super::copy_hashed;
use super::{
    BUNDLE_MIMETYPE, BundleInfo, BundleManifest, CONTROL_ENTRIES, ManifestHeader, PatchBundle,
    PayloadKind, bundle_error, check_engine, check_info, payload_kind,
};
use crate::error::Result;
use crate::patch::PatchSpec;

/// A bundle whose manifest signature and format version have been checked.
/// Payload hashes are checked when the payload is read by [`Self::load`].
pub struct BundleArchive {
    archive: ZipArchive<File>,
    manifest: BundleManifest,
    public_key: [u8; 32],
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
        // Catalog schemas can differ between engine lines, so report compatibility first.
        let header: ManifestHeader = toml::from_str(manifest_text)?;
        check_info(&header.bundle)?;
        check_engine(&header.bundle)?;
        let manifest: BundleManifest = toml::from_str(manifest_text)?;
        manifest.check_patches()?;
        Ok(Self {
            archive,
            manifest,
            public_key,
        })
    }

    pub fn info(&self) -> &BundleInfo {
        &self.manifest.bundle
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.manifest.files.keys().map(String::as_str)
    }

    /// Reads signed patch metadata without extracting payloads or executing code.
    /// Signature verification identifies the signer; it does not establish trust.
    pub fn patches(&self) -> &[PatchSpec] {
        &self.manifest.patches
    }

    /// Extracts the payload, checking every file against the manifest, and
    /// loads the patches it contains.
    pub fn load(mut self) -> Result<PatchBundle> {
        info!(bundle = %self.manifest.bundle.name, "loading bundle");
        let extracted = Arc::new(
            ScratchDir::new("bundle")
                .map_err(|e| bundle_error(format!("failed to create scratch directory: {e}")))?,
        );
        let mut jars = Vec::new();
        let mut extension_dex = Vec::new();
        let mut remaining = std::mem::take(&mut self.manifest.files);
        for index in 0..self.archive.len() {
            let mut entry = self.archive.by_index(index)?;
            let name = entry.name().to_string();
            if entry.is_dir() || CONTROL_ENTRIES.contains(&name.as_str()) {
                continue;
            }
            let expected = remaining.remove(&name).ok_or_else(|| {
                bundle_error(format!(
                    "file {name} is undeclared or occurs more than once"
                ))
            })?;
            let expected: [u8; 32] = hex::decode(expected)
                .map_err(|error| bundle_error(format!("invalid hash for {name}: {error}")))?
                .try_into()
                .map_err(|_| bundle_error(format!("hash for {name} must be SHA-256")))?;
            let kind = payload_kind(&name)?;
            let out = extracted.path().join(&name);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = File::create(&out)?;
            if copy_hashed(&mut entry, &mut file)
                .map_err(|error| bundle_error(format!("extract payload {name}: {error}")))?
                != expected
            {
                return Err(bundle_error(format!("hash mismatch for {name}")));
            }
            // ART refuses to load writable code on Android 14 and later.
            #[cfg(target_os = "android")]
            if kind != PayloadKind::Resource {
                let mut permissions = file.metadata()?.permissions();
                permissions.set_readonly(true);
                file.set_permissions(permissions)?;
            }
            match kind {
                PayloadKind::Jar => jars.push(out),
                PayloadKind::Dex => extension_dex.push(out),
                PayloadKind::Resource => {}
            }
        }
        if !remaining.is_empty() {
            return Err(bundle_error(
                "manifest declares files missing from the archive",
            ));
        }
        jars.sort();
        extension_dex.sort();

        #[cfg(feature = "bridge")]
        let patches =
            crate::kotlin::load_patches(&jars, Arc::clone(&extracted), &self.manifest.bundle.name)?;
        #[cfg(not(feature = "bridge"))]
        let patches = Vec::new();

        let mut loaded: Vec<_> = patches.iter().map(crate::patch::Patch::spec).collect();
        let mut declared: Vec<_> = self.manifest.patches.iter().collect();
        loaded.sort_by(|a, b| a.id.cmp(&b.id));
        declared.sort_by(|a, b| a.id.cmp(&b.id));
        if loaded != declared {
            return Err(bundle_error(
                "loaded patch metadata differs from the signed catalog; rebuild the bundle",
            ));
        }

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

fn read_entry(archive: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name).map_err(|error| {
        bundle_error(format!("required entry `{name}` cannot be read: {error}"))
    })?;
    let limit = match name {
        "manifest.toml" => 16 * 1024 * 1024,
        "manifest.pubkey" => 32,
        "manifest.sig" => 64,
        "mimetype" => BUNDLE_MIMETYPE.len() as u64,
        _ => return Err(bundle_error(format!("unknown control entry {name}"))),
    };
    if entry.size() > limit {
        return Err(bundle_error(format!(
            "control entry {name} exceeds {limit} bytes"
        )));
    }
    let mut buf = Vec::new();
    entry.by_ref().take(limit + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(bundle_error(format!(
            "control entry {name} exceeds {limit} bytes"
        )));
    }
    Ok(buf)
}
