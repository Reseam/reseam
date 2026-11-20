// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::forge::{Forge, Published};
use reseam_model::ReleaseIndex;

use super::index;
use super::notes;
use super::store::Store;
use crate::app::ReleaseArgs;

/// A release being published: where to, with which notes, and the forge
/// release that announces it.
pub(super) struct Release<'a> {
    pub args: &'a ReleaseArgs,
    pub store: Store,
    announcement: Option<Forge>,
    /// What the forge release says. Its page lists the attached media itself.
    pub notes: String,
    media: String,
    /// The index this release adds to, as published before it.
    pub previous: Option<ReleaseIndex>,
    published: Published,
}

impl<'a> Release<'a> {
    /// Opens the release of `args.tag`. With `index`, the name of the release index it adds to, a published forge
    /// release of a tag that index does not list yet is taken as the author's draft.
    pub fn open(args: &'a ReleaseArgs, index: Option<&str>) -> Result<Self> {
        let store = Store::open(args)?;
        let previous = index
            .map(|name| store.previous_index(name))
            .transpose()?
            .flatten();
        let published = match (index, &previous) {
            (Some(_), None) => Published::Draft,
            (Some(_), Some(previous)) if !index::lists(previous, &args.tag) => Published::Draft,
            _ => Published::Earlier,
        };
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
        let mut release = Self {
            args,
            store,
            announcement,
            notes,
            media: String::new(),
            previous,
            published,
        };
        let draft = release
            .forge()
            .map(|forge| forge.draft(&args.tag, published))
            .transpose()?;
        if let Some(draft) = draft.flatten() {
            release.notes = notes::paragraphs([&draft.text, release.notes.trim()]);
            release.media = draft
                .media
                .iter()
                .map(|(name, url)| format!("![{name}]({url})"))
                .collect::<Vec<_>>()
                .join("\n\n");
        }
        Ok(release)
    }

    /// The notes clients show: the author's media in Markdown, then what the forge release says.
    pub fn description(&self) -> String {
        notes::paragraphs([&self.media, &self.notes])
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
                forge.publish(tag, body, prerelease, &assets, self.published)
            }
            Store::Files(store) => {
                store.upload(tag, files, index)?;
                if let Some(keep) = self.args.keep {
                    store.prune(keep)?;
                }
                match &self.announcement {
                    Some(forge) => forge.publish(tag, body, prerelease, &[], self.published),
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
    pub fn index(name: &str, release_index: &ReleaseIndex) -> Result<Self> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join(name);
        std::fs::write(&path, index::to_json(release_index)?)?;
        Ok(Self {
            _folder: folder,
            path,
        })
    }
}
