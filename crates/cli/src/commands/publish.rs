// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::app::PublishPatchesCommand;

#[derive(Debug)]
struct BundleArchiveInfo {
    name: String,
    author: String,
    description: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
struct BundleIndexManifest {
    bundle: BundleIndexManifestInfo,
}

#[derive(Debug, Deserialize)]
struct BundleIndexManifestInfo {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatchesIndex {
    bundle: PatchesIndexBundle,
    releases: Vec<PatchesIndexRelease>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatchesIndexBundle {
    name: String,
    author: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatchesIndexRelease {
    version: String,
    created_at: String,
    description: String,
    download_url: String,
    prerelease: bool,
}

pub fn run_publish_patches(command: &PublishPatchesCommand) -> Result<()> {
    if command.version.trim().is_empty() {
        bail!("--version must not be empty");
    }
    if command.url.trim().is_empty() {
        bail!("--url must not be empty");
    }

    let archive = inspect_reseam_archive(&command.bundle).with_context(|| {
        format!(
            "failed to inspect bundle archive {}",
            command.bundle.display()
        )
    })?;

    let release_description = match (&command.description, &command.description_file) {
        (Some(text), None) => text.clone(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
        (None, None) => String::new(),
        (Some(_), Some(_)) => bail!("--description and --description-file are mutually exclusive"),
    };

    let created_at = match &command.created_at {
        Some(value) => {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                .context("--created-at must be an RFC3339 timestamp")?;
            value.clone()
        }
        None => time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .context("failed to format current time")?,
    };

    let existing = if command.out.exists() {
        let json = std::fs::read_to_string(&command.out)
            .with_context(|| format!("failed to read {}", command.out.display()))?;
        Some(
            serde_json::from_str::<PatchesIndex>(&json)
                .with_context(|| format!("failed to parse {}", command.out.display()))?,
        )
    } else {
        None
    };

    if let Some(existing) = &existing {
        if existing.bundle.public_key != archive.public_key {
            bail!(
                "refusing to change bundle public key in {} (existing {}, archive {})",
                command.out.display(),
                existing.bundle.public_key,
                archive.public_key
            );
        }
    }

    let homepage = command.homepage.clone().or_else(|| {
        existing
            .as_ref()
            .and_then(|index| index.bundle.homepage.clone())
    });

    let mut releases = existing
        .map(|mut index| {
            index
                .releases
                .retain(|release| release.version != command.version);
            index.releases
        })
        .unwrap_or_default();

    releases.insert(
        0,
        PatchesIndexRelease {
            version: command.version.clone(),
            created_at,
            description: release_description,
            download_url: command.url.clone(),
            prerelease: command.prerelease,
        },
    );

    let index = PatchesIndex {
        bundle: PatchesIndexBundle {
            name: archive.name,
            author: archive.author,
            description: archive.description,
            homepage,
            public_key: archive.public_key,
        },
        releases,
    };

    write_json_atomically(&command.out, &index)?;
    info!(out = %command.out.display(), "patches index written");
    Ok(())
}

fn inspect_reseam_archive(path: &Path) -> Result<BundleArchiveInfo> {
    let file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("failed to open zip {}", path.display()))?;

    let mimetype = read_zip_entry(&mut archive, "mimetype")?;
    if mimetype != reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes() {
        bail!(
            "invalid mimetype marker (expected {})",
            reseam_patcher::bundle::BUNDLE_MIMETYPE
        );
    }

    let manifest_bytes = read_zip_entry(&mut archive, "manifest.toml")?;
    let public_key_bytes = read_zip_entry(&mut archive, "manifest.pubkey")?;
    let signature_bytes = read_zip_entry(&mut archive, "manifest.sig")?;

    let public_key: [u8; 32] = public_key_bytes.as_slice().try_into().with_context(|| {
        format!(
            "manifest.pubkey has wrong length: {} (expected 32)",
            public_key_bytes.len()
        )
    })?;
    let verifying_key =
        VerifyingKey::from_bytes(&public_key).context("invalid Ed25519 public key")?;
    let signature = Signature::from_slice(&signature_bytes).context("invalid Ed25519 signature")?;
    verifying_key
        .verify(&manifest_bytes, &signature)
        .context("manifest signature verification failed")?;

    let manifest: BundleIndexManifest =
        toml::from_str(std::str::from_utf8(&manifest_bytes).context("manifest.toml is not UTF-8")?)
            .context("failed to parse manifest.toml")?;

    Ok(BundleArchiveInfo {
        name: manifest.bundle.name,
        author: manifest.bundle.author,
        description: manifest.bundle.description,
        public_key: hex::encode(public_key),
    })
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("missing required entry `{name}`"))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read `{name}`"))?;
    Ok(bytes)
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid output path {}", path.display()))?;
    let tmp_name = format!(".{file_name}.{}.tmp", std::process::id());
    let tmp_path = parent.unwrap_or_else(|| Path::new(".")).join(tmp_name);

    let json = serde_json::to_vec_pretty(value).context("failed to serialize patches index")?;
    std::fs::write(&tmp_path, [&json[..], b"\n"].concat())
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to move {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}
