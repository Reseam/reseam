// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod directory;
mod forge;
pub(super) mod http;
pub(super) mod index;
mod notes;
mod publish;
mod s3;
mod store;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use reseam_model::{InspectRequest, InspectResponse, Trust};
use reseam_sdk::inspect;
use semver::Version;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use self::index::{Entry, Index, Publisher};
use self::publish::{Release, Staged};
use self::store::Store;
use crate::app::{ReleaseFilesCommand, ReleaseManagerCommand, ReleasePatchesCommand};

const PATCHES_INDEX: &str = "patches.json";
const MANAGER_INDEX: &str = "manager.json";

pub fn run_release_patches(command: &ReleasePatchesCommand) -> Result<()> {
    let InspectResponse {
        mut bundles,
        patches,
        ..
    } = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![command.bundle.display().to_string()],
        trust: Trust::default(),
    })?;
    let info = bundles.remove(0);
    if let Some(problem) = info.problem {
        bail!(
            "failed to open bundle {}: {problem}",
            command.bundle.display()
        );
    }
    ensure!(
        !info.version.is_empty(),
        "{} has no version; pack it with --version",
        command.bundle.display()
    );
    let release = Release::open(&command.release)?;
    let previous = release.store.previous_index(PATCHES_INDEX)?;
    let homepage = command
        .homepage
        .clone()
        .or_else(|| {
            previous
                .as_ref()
                .and_then(|index| index.homepage().map(str::to_owned))
        })
        .or_else(|| release.forge().map(|forge| forge.page().to_string()));
    let publisher = Publisher {
        name: info.name.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        homepage,
        public_key: Some(info.public_key),
    };
    let prerelease = is_prerelease(&info.version);
    let entry = Entry {
        version: info.version.clone(),
        created_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        description: release.notes.clone(),
        download_url: release
            .store
            .download_url(&release.args.tag, file_name(&command.bundle)?)?
            .to_string(),
        prerelease,
        patches: Some(patches.into_iter().map(|patch| patch.spec).collect()),
    };
    let staged = Staged::index(PATCHES_INDEX, &Index::add(previous, publisher, entry)?)?;
    let body = match &release.store {
        Store::Forge(forge) => format!(
            "[Add to Reseam Manager](https://reseam.app/add/?url={})\n\n{}",
            url::form_urlencoded::byte_serialize(forge.page().as_str().as_bytes())
                .collect::<String>(),
            release.notes
        ),
        Store::Files(_) => release.notes.clone(),
    };
    release.publish(&body, prerelease, &[&command.bundle], Some(&staged.path))
}

pub fn run_release_manager(command: &ReleaseManagerCommand) -> Result<()> {
    ensure!(!command.name.trim().is_empty(), "--name must not be empty");
    let files = files_in(&command.dir)?;
    ensure!(
        files
            .iter()
            .all(|path| path.file_name() != Some(MANAGER_INDEX.as_ref())),
        "{} already holds a {MANAGER_INDEX}",
        command.dir.display()
    );
    let release = Release::open(&command.release)?;
    let version = release.args.tag.trim_start_matches('v').to_owned();
    let prerelease = is_prerelease(&version);
    let publisher = Publisher {
        name: command.name.clone(),
        author: command.author.clone(),
        description: command.summary.clone(),
        homepage: None,
        public_key: None,
    };
    let entry = Entry {
        version,
        created_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        description: release.notes.clone(),
        download_url: command.download_url.to_string(),
        prerelease,
        patches: None,
    };
    let previous = release.store.previous_index(MANAGER_INDEX)?;
    let staged = Staged::index(MANAGER_INDEX, &Index::add(previous, publisher, entry)?)?;
    let files: Vec<_> = files.iter().map(PathBuf::as_path).collect();
    release.publish(&release.notes, prerelease, &files, Some(&staged.path))
}

pub fn run_release_files(command: &ReleaseFilesCommand) -> Result<()> {
    let files = files_in(&command.dir)?;
    let release = Release::open(&command.release)?;
    let files: Vec<_> = files.iter().map(PathBuf::as_path).collect();
    let prerelease = is_prerelease(&release.args.tag);
    release.publish(&release.notes, prerelease, &files, None)
}

fn files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?
    {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    ensure!(!files.is_empty(), "{} holds no files", dir.display());
    files.sort();
    Ok(files)
}

fn is_prerelease(version: &str) -> bool {
    Version::parse(version.trim_start_matches('v')).is_ok_and(|version| !version.pre.is_empty())
}

fn file_name(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("{} has no UTF-8 file name", path.display()))
}

/// The variable `name`; `hint` says where a release expects it from.
fn env(name: &str, hint: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("{name} is not set; {hint}"))
}
