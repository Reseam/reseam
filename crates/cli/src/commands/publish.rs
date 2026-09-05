// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::{ensure, Context, Result};
use reseam_patcher::bundle::BundleArchive;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::info;

use super::create_parent;
use crate::app::{PublishManagerCommand, PublishPatchesCommand, ReleaseArgs};

/// `patches.json` and `manager.json` share one shape: who publishes, then
/// releases newest first. The `bundle` key is the API's name for the
/// publisher in both files.
#[derive(Debug, Serialize, Deserialize)]
struct Index {
    bundle: Publisher,
    releases: Vec<Release>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Publisher {
    name: String,
    author: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    public_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Release {
    version: String,
    created_at: String,
    description: String,
    download_url: String,
    prerelease: bool,
}

/// Adds a bundle release to `patches.json`, taking the publisher identity
/// from the signed archive.
pub fn run_publish_patches(command: &PublishPatchesCommand) -> Result<()> {
    let archive = BundleArchive::open(&command.bundle)
        .with_context(|| format!("failed to open bundle {}", command.bundle.display()))?;
    let info = archive.info();
    let publisher = Publisher {
        name: info.name.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        homepage: None,
        public_key: Some(hex::encode(archive.public_key)),
    };
    publish(&command.out, publisher, &command.release)
}

/// Adds a manager release to `manager.json`.
pub fn run_publish_manager(command: &PublishManagerCommand) -> Result<()> {
    ensure!(!command.name.trim().is_empty(), "--name must not be empty");
    let publisher = Publisher {
        name: command.name.clone(),
        author: command.author.clone(),
        description: command.summary.clone(),
        homepage: None,
        public_key: None,
    };
    publish(&command.out, publisher, &command.release)
}

/// Rewrites `out` with `release` on top, replacing any release of the same
/// version. An existing index must belong to the same signer.
fn publish(out: &Path, mut publisher: Publisher, release: &ReleaseArgs) -> Result<()> {
    ensure!(
        !release.version.trim().is_empty(),
        "--version must not be empty"
    );
    ensure!(!release.url.trim().is_empty(), "--url must not be empty");

    let description = match (&release.description, &release.description_file) {
        (Some(text), _) => text.clone(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
        (None, None) => String::new(),
    };
    let created_at = match &release.created_at {
        Some(value) => {
            OffsetDateTime::parse(value, &Rfc3339)
                .context("--created-at must be an RFC3339 timestamp")?;
            value.clone()
        }
        None => OffsetDateTime::now_utc().format(&Rfc3339)?,
    };

    let existing: Option<Index> = out
        .exists()
        .then(|| {
            let json = std::fs::read_to_string(out)
                .with_context(|| format!("failed to read {}", out.display()))?;
            serde_json::from_str(&json)
                .with_context(|| format!("failed to parse {}", out.display()))
        })
        .transpose()?;
    if let Some(existing) = &existing {
        ensure!(
            existing.bundle.public_key == publisher.public_key,
            "refusing to change the public key in {} (existing {:?}, new {:?})",
            out.display(),
            existing.bundle.public_key,
            publisher.public_key
        );
    }
    publisher.homepage = release.homepage.clone().or_else(|| {
        existing
            .as_ref()
            .and_then(|index| index.bundle.homepage.clone())
    });
    let mut releases = existing.map(|index| index.releases).unwrap_or_default();
    releases.retain(|entry| entry.version != release.version);
    releases.insert(
        0,
        Release {
            version: release.version.clone(),
            created_at,
            description,
            download_url: release.url.clone(),
            prerelease: release.prerelease,
        },
    );

    write_json_atomically(
        out,
        &Index {
            bundle: publisher,
            releases,
        },
    )?;
    info!(out = %out.display(), "release index written");
    Ok(())
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    create_parent(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid output path {}", path.display()))?;
    let tmp_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut json = serde_json::to_vec_pretty(value)?;
    json.push(b'\n');
    std::fs::write(&tmp_path, json)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to move {} to {}",
            tmp_path.display(),
            path.display()
        )
    })
}
