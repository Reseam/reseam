// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Display;
use std::num::NonZeroUsize;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use semver::Version;
use tracing::info;
use url::Url;

use super::directory::Directory;
use super::file_name;
use super::forge::Forge;
use super::index::Index;
use super::s3::S3;
use crate::app::{ReleaseArgs, Target};

/// Where a release's files go.
pub(super) enum Store {
    /// Assets of the forge release for the tag.
    Forge(Forge),
    Files(Files),
}

impl Store {
    pub fn open(args: &ReleaseArgs) -> Result<Self> {
        let storage: Box<dyn Storage> = match &args.to {
            Target::Forge => {
                ensure!(
                    args.public_url.is_none(),
                    "--public-url is for s3 and folder targets"
                );
                ensure!(
                    !args.forge_release,
                    "--forge-release is for s3 and folder targets"
                );
                ensure!(args.keep.is_none(), "--keep is for s3 and folder targets");
                return Ok(Self::Forge(Forge::from_env()?));
            }
            Target::S3 { bucket, prefix } => Box::new(S3::from_env(bucket, prefix)?),
            Target::Directory(root) => Box::new(Directory::new(root.clone())),
        };
        Ok(Self::Files(Files {
            storage,
            public_url: args.public_url.clone(),
        }))
    }

    pub fn previous_index(&self, name: &str) -> Result<Option<Index>> {
        let json = match self {
            Self::Forge(forge) => forge.latest_asset(name)?,
            Self::Files(files) => files.storage.get(name)?,
        };
        json.as_deref().map(Index::parse).transpose()
    }

    pub fn download_url(&self, tag: &str, name: &str) -> Result<Url> {
        match self {
            Self::Forge(forge) => Ok(forge.asset_url(tag, name)),
            Self::Files(files) => files.download_url(tag, name),
        }
    }
}

/// A target that keeps each release in a folder named after its tag, and the
/// latest index beside those folders. Keys are `/`-separated and relative to
/// the target.
pub(super) trait Storage: Display {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn put(&self, key: &str, path: &Path, caching: Caching) -> Result<()>;
    /// The names of the folders at the top of the target.
    fn folders(&self) -> Result<Vec<String>>;
    fn delete_folder(&self, name: &str) -> Result<()>;
    /// The path of `key` under the URL the target is served from.
    fn public_path(&self, key: &str) -> String;
}

#[derive(Clone, Copy)]
pub(super) enum Caching {
    /// A file of a release, which never changes.
    Release,
    /// The latest index, replaced by every release.
    Latest,
}

pub(super) struct Files {
    storage: Box<dyn Storage>,
    public_url: Option<Url>,
}

impl Files {
    fn download_url(&self, tag: &str, name: &str) -> Result<Url> {
        let mut url = self
            .public_url
            .clone()
            .context("--public-url is required to link the release from its index")?;
        url.path_segments_mut()
            .expect("http URLs have path segments")
            .pop_if_empty()
            .extend(
                self.storage
                    .public_path(&format!("{tag}/{name}"))
                    .split('/'),
            );
        Ok(url)
    }

    pub fn upload(&self, tag: &str, files: &[&Path], index: Option<&Path>) -> Result<()> {
        for path in files.iter().copied().chain(index) {
            self.storage.put(
                &format!("{tag}/{}", file_name(path)?),
                path,
                Caching::Release,
            )?;
        }
        // The latest index goes last, so it never points at files still uploading.
        if let Some(index) = index {
            self.storage
                .put(file_name(index)?, index, Caching::Latest)?;
        }
        info!(tag, to = %self.storage, "uploaded the release");
        Ok(())
    }

    /// Deletes all but the newest `keep` releases.
    pub fn prune(&self, keep: NonZeroUsize) -> Result<()> {
        let mut releases: Vec<_> = self
            .storage
            .folders()?
            .into_iter()
            .filter_map(|tag| Some((Version::parse(tag.trim_start_matches('v')).ok()?, tag)))
            .collect();
        releases.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, tag) in releases.into_iter().skip(keep.get()) {
            self.storage.delete_folder(&tag)?;
            info!(tag, "pruned an old release");
        }
        Ok(())
    }
}
