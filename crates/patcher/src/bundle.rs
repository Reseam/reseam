// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{PatcherError, Result};
use crate::patch::Patch;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::{debug, info};
use zip::ZipArchive;

pub const BUNDLE_MIMETYPE: &str = "application/vnd.reseam.bundle";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;

pub const TRUSTED_KEYS: &[[u8; 32]] = &[
    [
        0xa1, 0xf8, 0x6a, 0xde, 0x34, 0x48, 0xcc, 0xb8, 0x77, 0x16, 0x08, 0x64, 0xc9, 0xbd, 0xd1,
        0x35, 0xbb, 0x80, 0x7e, 0x94, 0x46, 0x6b, 0x43, 0xa8, 0x8c, 0x56, 0x6b, 0xcd, 0xae, 0x9c,
        0x37, 0x53,
    ],
];

#[derive(Debug, Deserialize)]
struct BundleManifest {
    bundle: BundleInfo,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct BundleInfo {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    format_version: u32,
}

#[derive(Debug, Clone)]
pub struct BundleInspection {
    pub name: String,
    pub author: String,
    pub description: String,
    pub public_key: [u8; 32],
}

pub struct PatchBundle {
    pub name: String,
    pub author: String,
    pub description: String,
    pub patches: Vec<Box<dyn Patch>>,
    pub extension_dex: Vec<PathBuf>,
    _extracted: TempDir,
}

pub struct BundleKeepAlive {
    _extracted: TempDir,
}

impl PatchBundle {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::load_with_trust_anchors(path, TRUSTED_KEYS)
    }

    pub fn inspect(path: impl AsRef<Path>) -> Result<BundleInspection> {
        let mut archive = open_bundle_archive(path.as_ref())?;
        scan_payload_entries(&mut archive.archive, &archive.manifest, |_, _| Ok(()))?;

        Ok(BundleInspection {
            name: archive.manifest.bundle.name,
            author: archive.manifest.bundle.author,
            description: archive.manifest.bundle.description,
            public_key: archive.public_key,
        })
    }

    pub fn load_with_trust_anchors(
        path: impl AsRef<Path>,
        trusted_keys: &[[u8; 32]],
    ) -> Result<Self> {
        let path = path.as_ref();
        info!(bundle_path = %path.display(), "loading .reseam bundle");
        let mut archive = open_bundle_archive(path)?;
        if !trusted_keys.iter().any(|key| key == &archive.public_key) {
            return Err(PatcherError::Bundle {
                reason: "signing key is not in the trusted anchor list".into(),
            });
        }

        let tempdir = tempfile::tempdir().map_err(|e| PatcherError::Bundle {
            reason: format!("failed to create tempdir: {e}"),
        })?;
        let mut jar_files = Vec::new();
        let mut extension_dex = Vec::new();
        scan_payload_entries(&mut archive.archive, &archive.manifest, |name, contents| {
            let out = tempdir.path().join(name);
            std::fs::write(&out, contents).map_err(|error| PatcherError::Bundle {
                reason: format!("write {}: {error}", out.display()),
            })?;

            if name.ends_with(".jar") {
                jar_files.push(out);
            } else if name.ends_with(".dex") || name.ends_with(".rve") {
                extension_dex.push(out);
            }
            Ok(())
        })?;

        jar_files.sort();
        extension_dex.sort();

        let mut patches: Vec<Box<dyn Patch>> = Vec::new();
        #[cfg(feature = "kotlin")]
        {
            debug!(jar_count = jar_files.len(), "loading Kotlin patches");
            if !jar_files.is_empty() {
                patches.extend(crate::kotlin::load_kotlin_patches(
                    &jar_files,
                    tempdir.path(),
                    &extension_dex,
                )?);
            }
        }

        info!(
            bundle_name = %archive.manifest.bundle.name,
            patch_count = patches.len(),
            extension_dex_count = extension_dex.len(),
            "patch bundle loaded"
        );

        Ok(Self {
            name: archive.manifest.bundle.name,
            author: archive.manifest.bundle.author,
            description: archive.manifest.bundle.description,
            patches,
            extension_dex,
            _extracted: tempdir,
        })
    }

    pub fn into_patches_and_keepalive(self) -> (Vec<Box<dyn Patch>>, BundleKeepAlive) {
        (
            self.patches,
            BundleKeepAlive {
                _extracted: self._extracted,
            },
        )
    }
}

struct OpenBundleArchive<R> {
    archive: ZipArchive<R>,
    manifest: BundleManifest,
    public_key: [u8; 32],
}

fn open_bundle_archive(path: &Path) -> Result<OpenBundleArchive<File>> {
    let file = File::open(path).map_err(|e| PatcherError::Bundle {
        reason: format!("failed to open {}: {e}", path.display()),
    })?;
    let mut archive = ZipArchive::new(file).map_err(|e| PatcherError::Bundle {
        reason: format!("failed to open zip {}: {e}", path.display()),
    })?;

    let mimetype = read_entry(&mut archive, "mimetype")?;
    if mimetype != BUNDLE_MIMETYPE.as_bytes() {
        return Err(PatcherError::Bundle {
            reason: format!("invalid mimetype marker (expected {BUNDLE_MIMETYPE})"),
        });
    }

    let manifest_bytes = read_entry(&mut archive, "manifest.toml")?;
    let pubkey_bytes = read_entry(&mut archive, "manifest.pubkey")?;
    let sig_bytes = read_entry(&mut archive, "manifest.sig")?;

    let public_key: [u8; 32] = pubkey_bytes
        .as_slice()
        .try_into()
        .map_err(|_| PatcherError::Bundle {
            reason: format!(
                "manifest.pubkey has wrong length: {} (expected 32)",
                pubkey_bytes.len()
            ),
        })?;
    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|e| PatcherError::Bundle {
        reason: format!("invalid Ed25519 public key: {e}"),
    })?;
    let signature = Signature::from_slice(&sig_bytes).map_err(|e| PatcherError::Bundle {
        reason: format!("invalid Ed25519 signature: {e}"),
    })?;
    verifying_key
        .verify(&manifest_bytes, &signature)
        .map_err(|_| PatcherError::Bundle {
            reason: "manifest signature verification failed".into(),
        })?;

    let manifest: BundleManifest =
        toml::from_str(std::str::from_utf8(&manifest_bytes).map_err(|e| PatcherError::Bundle {
            reason: format!("manifest.toml not UTF-8: {e}"),
        })?)?;
    if manifest.bundle.format_version != BUNDLE_FORMAT_VERSION {
        return Err(PatcherError::Bundle {
            reason: format!(
                "unsupported bundle format_version: {} (patcher supports {})",
                manifest.bundle.format_version, BUNDLE_FORMAT_VERSION
            ),
        });
    }

    Ok(OpenBundleArchive {
        archive,
        manifest,
        public_key,
    })
}

fn scan_payload_entries<R, F>(
    archive: &mut ZipArchive<R>,
    manifest: &BundleManifest,
    mut on_entry: F,
) -> Result<()>
where
    R: Read + std::io::Seek,
    F: FnMut(&str, &[u8]) -> Result<()>,
{
    let mut seen_payload = BTreeSet::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| PatcherError::Bundle {
            reason: format!("zip entry {index}: {e}"),
        })?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            continue;
        }
        if matches!(
            name.as_str(),
            "mimetype" | "manifest.toml" | "manifest.pubkey" | "manifest.sig"
        ) {
            continue;
        }

        let mut contents = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut contents)
            .map_err(|e| PatcherError::Bundle {
                reason: format!("read {name}: {e}"),
            })?;

        let expected = manifest.files.get(&name).ok_or_else(|| PatcherError::Bundle {
            reason: format!("file {name} is not declared in manifest [files]"),
        })?;
        let actual = hex::encode(Sha256::digest(&contents));
        if actual != *expected {
            return Err(PatcherError::Bundle {
                reason: format!("hash mismatch for {name}"),
            });
        }

        on_entry(&name, &contents)?;
        seen_payload.insert(name);
    }

    for declared in manifest.files.keys() {
        if !seen_payload.contains(declared) {
            return Err(PatcherError::Bundle {
                reason: format!("manifest declares {declared} but it is missing from the archive"),
            });
        }
    }

    Ok(())
}

fn read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name).map_err(|_| PatcherError::Bundle {
        reason: format!("required entry `{name}` missing from bundle"),
    })?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut buf)
        .map_err(|e| PatcherError::Bundle {
            reason: format!("read {name}: {e}"),
        })?;
    Ok(buf)
}
