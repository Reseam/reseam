// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use reseam_patcher::PatchSpec;
use reseam_patcher::bundle::BundleArchive;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::info;

use super::create_parent;
use crate::app::{PublishManagerCommand, PublishPatchesCommand, ReleaseArgs};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    patches: Option<Vec<PatchSpec>>,
}

/// Adds a bundle release to `patches.json`, taking the publisher identity
/// from the signed archive.
pub fn run_publish_patches(command: &PublishPatchesCommand) -> Result<()> {
    let archive = BundleArchive::open(&command.bundle)
        .with_context(|| format!("failed to open bundle {}", command.bundle.display()))?;
    let publisher = {
        let info = archive.info();
        Publisher {
            name: info.name.clone(),
            author: info.author.clone(),
            description: info.description.clone(),
            homepage: None,
            public_key: Some(hex::encode(archive.public_key())),
        }
    };
    let patches = archive.patches().to_vec();
    publish(&command.out, publisher, &command.release, Some(patches))
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
    publish(&command.out, publisher, &command.release, None)
}

fn publish(
    out: &Path,
    mut publisher: Publisher,
    release: &ReleaseArgs,
    patches: Option<Vec<PatchSpec>>,
) -> Result<()> {
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

    let existing: Option<Index> = match std::fs::File::open(out) {
        Ok(file) => Some(
            serde_json::from_reader(std::io::BufReader::new(file))
                .with_context(|| format!("parse {}", out.display()))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("read {}", out.display())),
    };
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
            patches,
        },
    );

    write_index_atomically(
        out,
        &Index {
            bundle: publisher,
            releases,
        },
    )?;
    info!(out = %out.display(), "release index written");
    Ok(())
}

fn write_index_atomically(path: &Path, value: &Index) -> Result<()> {
    create_parent(path)?;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut builder = tempfile::Builder::new();
    builder.prefix(".reseam-index-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut staged = builder.tempfile_in(parent)?;
    {
        let mut writer = std::io::BufWriter::new(staged.as_file_mut());
        serde_json::to_writer_pretty(&mut writer, value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    staged
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish {}", path.display()))?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::run_publish_manager;
    use crate::app::{PublishManagerCommand, ReleaseArgs};

    #[test]
    fn release_indices_use_regular_file_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let reference = dir.path().join("regular-file");
        std::fs::write(&reference, b"").unwrap();
        let command = PublishManagerCommand {
            name: "Reseam Manager".into(),
            author: "Reseam".into(),
            summary: String::new(),
            out: dir.path().join("manager.json"),
            release: ReleaseArgs {
                version: "1.0.0".into(),
                url: "https://example.com/manager.apk".into(),
                description: None,
                description_file: None,
                homepage: None,
                created_at: Some("2026-10-01T00:00:00Z".into()),
                prerelease: false,
            },
        };
        run_publish_manager(&command).unwrap();
        assert_eq!(
            std::fs::metadata(&command.out)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            std::fs::metadata(reference).unwrap().permissions().mode() & 0o777
        );
    }
}
