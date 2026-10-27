// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reseam_storage::ScratchDir;
use tracing::info;
use zip::ZipArchive;

use reseam_storage::copy_hashed;

use super::manifest::{BundleManifest, ManifestHeader};
use super::payload::{PayloadKind, payload_kind};
use super::{BUNDLE_MIMETYPE, BundleInfo, CONTROL_ENTRIES, PatchBundle};
use crate::error::{BundleError, Result};
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
        let archive_error = |source| BundleError::Archive {
            path: path.to_owned(),
            source,
        };
        let mut archive = ZipArchive::new(File::open(path)?).map_err(archive_error)?;
        if read_entry(&mut archive, "mimetype", BUNDLE_MIMETYPE.len() as u64)?
            != BUNDLE_MIMETYPE.as_bytes()
        {
            return Err(BundleError::Mimetype.into());
        }
        let manifest_bytes = read_entry(&mut archive, "manifest.toml", 16 * 1024 * 1024)?;
        let public_key: [u8; 32] = read_entry(&mut archive, "manifest.pubkey", 32)?
            .try_into()
            .map_err(|_| BundleError::Signature("manifest.pubkey must be 32 bytes"))?;
        let signature = Signature::from_slice(&read_entry(&mut archive, "manifest.sig", 64)?)
            .map_err(|_| BundleError::Signature("invalid Ed25519 signature"))?;
        VerifyingKey::from_bytes(&public_key)
            .map_err(|_| BundleError::Signature("invalid Ed25519 public key"))?
            .verify(&manifest_bytes, &signature)
            .map_err(|_| BundleError::Signature("verification failed"))?;

        let manifest_text =
            std::str::from_utf8(&manifest_bytes).map_err(|_| BundleError::ManifestEncoding)?;
        // Catalog schemas can differ between engine lines, so report compatibility first.
        let header: ManifestHeader = toml::from_str(manifest_text)?;
        header.bundle.check()?;
        header.bundle.check_engine()?;
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
    /// loads the code of the catalog patches whose ids are in `patches`. Only
    /// their declaring classes are initialized; the rest of the bundle runs no code.
    pub fn load(mut self, patches: &HashSet<String>) -> Result<PatchBundle> {
        info!(bundle = %self.manifest.bundle.name, "loading bundle");
        let extracted = Arc::new(ScratchDir::new("bundle")?);
        let mut jars = Vec::new();
        let mut extension_dex = Vec::new();
        let mut remaining = std::mem::take(&mut self.manifest.files);
        for index in 0..self.archive.len() {
            let mut entry = self.archive.by_index(index)?;
            let name = entry.name().to_string();
            if entry.is_dir() || CONTROL_ENTRIES.contains(&name.as_str()) {
                continue;
            }
            let expected = remaining
                .remove(&name)
                .ok_or_else(|| BundleError::UndeclaredPayload(name.clone()))?;
            let expected: [u8; 32] = hex::decode(expected)
                .ok()
                .and_then(|hash| hash.try_into().ok())
                .ok_or_else(|| BundleError::PayloadHash(name.clone()))?;
            let kind = payload_kind(&name)?;
            let out = extracted.path().join(&name);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = File::create(&out)?;
            let hash =
                copy_hashed(&mut entry, &mut file).map_err(|source| BundleError::Payload {
                    name: name.clone(),
                    source,
                })?;
            if hash != expected {
                return Err(BundleError::HashMismatch(name).into());
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
            return Err(BundleError::MissingPayload.into());
        }
        jars.sort();
        extension_dex.sort();

        #[cfg(feature = "bridge")]
        let loaded = {
            let mut declarations = super::declarations::declarations(&jars)?;
            for members in declarations.values_mut() {
                members.retain(|declaration| patches.contains(&declaration.id));
            }
            declarations.retain(|_, members| !members.is_empty());
            crate::bridge::load_patches(
                &jars,
                &declarations,
                Arc::clone(&extracted),
                &self.manifest.bundle,
            )?
        };
        #[cfg(not(feature = "bridge"))]
        let loaded = Vec::new();

        let mut loaded_specs: Vec<_> = loaded.iter().map(crate::patch::Patch::spec).collect();
        let mut declared: Vec<_> = self
            .manifest
            .patches
            .iter()
            .filter(|spec| patches.contains(&spec.reference.id))
            .collect();
        loaded_specs.sort_by(|a, b| a.reference.cmp(&b.reference));
        declared.sort_by(|a, b| a.reference.cmp(&b.reference));
        if loaded_specs != declared {
            return Err(BundleError::CatalogMismatch.into());
        }

        info!(
            bundle = %self.manifest.bundle.name,
            patch_count = loaded.len(),
            extension_dex_count = extension_dex.len(),
            "bundle loaded"
        );
        Ok(PatchBundle {
            info: self.manifest.bundle,
            public_key: self.public_key,
            catalog: self.manifest.patches,
            patches: loaded,
            extension_dex,
            _extracted: extracted,
        })
    }
}

/// A control entry, which must not exceed `limit` bytes.
fn read_entry(archive: &mut ZipArchive<File>, name: &'static str, limit: u64) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .map_err(|source| BundleError::MissingEntry { name, source })?;
    let mut buf = Vec::new();
    entry.by_ref().take(limit + 1).read_to_end(&mut buf)?;
    if entry.size() > limit || buf.len() as u64 > limit {
        return Err(BundleError::EntryTooLarge { name, limit }.into());
    }
    Ok(buf)
}
