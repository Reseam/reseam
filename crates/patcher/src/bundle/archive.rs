// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reseam_storage::ScratchDir;
use tracing::info;
use zip::ZipArchive;

use super::container::{self, MANIFEST_LIMIT, read_entry};
use super::manifest::BundleManifest;
use super::payload::{PayloadKind, copy_payload};
use super::{BundleInfo, PatchBundle};
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
        let mut archive = container::open(path)?;
        let manifest_bytes = read_entry(&mut archive, "manifest.toml", MANIFEST_LIMIT)?;
        let public_key: [u8; 32] = read_entry(&mut archive, "manifest.pubkey", 32)?
            .try_into()
            .map_err(|_| BundleError::Signature("manifest.pubkey must be 32 bytes"))?;
        let signature = Signature::from_slice(&read_entry(&mut archive, "manifest.sig", 64)?)
            .map_err(|_| BundleError::Signature("invalid Ed25519 signature"))?;
        VerifyingKey::from_bytes(&public_key)
            .map_err(|_| BundleError::Signature("invalid Ed25519 public key"))?
            .verify(&manifest_bytes, &signature)
            .map_err(|_| BundleError::Signature("verification failed"))?;
        Ok(Self {
            archive,
            manifest: BundleManifest::parse(&manifest_bytes)?,
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

    /// The fingerprint of `patch`, one of [`Self::patches`], as hex. It changes
    /// only when the files of its module, or of a module that module uses, change.
    pub fn fingerprint(&self, patch: &PatchSpec) -> &str {
        self.manifest
            .fingerprints
            .get(&patch.id)
            .expect("opening checks that every catalog patch has a fingerprint")
    }

    /// Extracts the payload, checking every file against the manifest, and
    /// loads the code of the catalog patches whose ids are in `patches`. Only
    /// their declaring classes are initialized; the rest of the bundle runs no code.
    pub fn load(mut self, patches: &HashSet<String>) -> Result<PatchBundle> {
        info!(bundle = %self.manifest.bundle.name, "loading bundle");
        let extracted = Arc::new(ScratchDir::new("bundle")?);
        let entries = copy_payload(&mut self.archive, &self.manifest.files, |name, _| {
            let out = extracted.path().join(name);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Ok(File::create(out)?)
        })?;
        let mut jars = Vec::new();
        let mut extension_dex = Vec::new();
        for (name, kind) in entries {
            let path = extracted.path().join(name);
            // ART refuses to load writable code on Android 14 and later.
            #[cfg(target_os = "android")]
            if kind != PayloadKind::Resource {
                let mut permissions = std::fs::metadata(&path)?.permissions();
                permissions.set_readonly(true);
                std::fs::set_permissions(&path, permissions)?;
            }
            match kind {
                PayloadKind::Jar => jars.push(path),
                PayloadKind::Dex => extension_dex.push(path),
                PayloadKind::Resource => {}
            }
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
            .filter(|spec| patches.contains(&spec.reference().id))
            .collect();
        loaded_specs.sort_by(|a, b| (&a.bundle, &a.id).cmp(&(&b.bundle, &b.id)));
        declared.sort_by(|a, b| (&a.bundle, &a.id).cmp(&(&b.bundle, &b.id)));
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
