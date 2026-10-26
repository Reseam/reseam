// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::forge::Forge;
use super::index::Index;
use super::notes;
use super::store::Store;
use crate::app::ReleaseArgs;

/// A release being published: where to, with which notes, and the forge
/// release that announces it.
pub(super) struct Release<'a> {
    pub args: &'a ReleaseArgs,
    pub store: Store,
    announcement: Option<Forge>,
    pub notes: String,
}

impl<'a> Release<'a> {
    pub fn open(args: &'a ReleaseArgs) -> Result<Self> {
        let store = Store::open(args)?;
        let announcement = if args.forge_release {
            Some(Forge::from_env()?)
        } else {
            None
        };
        let notes = match &args.notes {
            Some(path) => std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?,
            None => notes::commit_subjects(&args.tag)?,
        };
        Ok(Self {
            args,
            store,
            announcement,
            notes,
        })
    }

    pub fn forge(&self) -> Option<&Forge> {
        match &self.store {
            Store::Forge(forge) => Some(forge),
            Store::Files(_) => self.announcement.as_ref(),
        }
    }

    pub fn publish(
        &self,
        body: &str,
        prerelease: bool,
        files: &[&Path],
        index: Option<&Path>,
    ) -> Result<()> {
        let tag = &self.args.tag;
        match &self.store {
            Store::Forge(forge) => {
                let assets: Vec<_> = files.iter().copied().chain(index).collect();
                forge.publish(tag, body, prerelease, &assets)
            }
            Store::Files(store) => {
                store.upload(tag, files, index)?;
                if let Some(keep) = self.args.keep {
                    store.prune(keep)?;
                }
                match &self.announcement {
                    Some(forge) => forge.publish(tag, body, prerelease, &[]),
                    None => Ok(()),
                }
            }
        }
    }
}

/// A release index written to a temporary folder, ready to upload.
pub(super) struct Staged {
    _folder: tempfile::TempDir,
    pub path: PathBuf,
}

impl Staged {
    pub fn index(name: &str, index: &Index) -> Result<Self> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join(name);
        std::fs::write(&path, index.to_json()?)?;
        Ok(Self {
            _folder: folder,
            path,
        })
    }
}
