// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;
use tracing::info;
use ureq::Agent;
use url::Url;

use super::http::{agent, checked, json, text_if_found};
use super::{env, file_name};

pub(super) struct Forge {
    api: Url,
    page: Url,
    repository: String,
    commit: Option<String>,
    auth: String,
    agent: Agent,
}

#[derive(Deserialize)]
struct Release {
    id: u64,
    tag_name: String,
    draft: bool,
    body: Option<String>,
    upload_url: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    id: u64,
    name: String,
    browser_download_url: String,
    /// The asset in the API. GitHub names it; Forgejo keeps assets under their release.
    url: Option<String>,
}

/// What an author put in a draft release for a tag before releasing it.
pub(super) struct Draft {
    pub text: String,
    /// The name and address of each image and video attached to it.
    pub media: Vec<(String, Url)>,
}

const MEDIA: [&str; 8] = ["avif", "gif", "jpeg", "jpg", "mp4", "png", "webm", "webp"];

fn media_name(file: &str) -> Option<&str> {
    let (name, extension) = file.rsplit_once('.')?;
    MEDIA
        .contains(&extension.to_ascii_lowercase().as_str())
        .then_some(name)
}

impl Forge {
    pub fn from_env() -> Result<Self> {
        let env = |name| env(name, "run in a GitHub or Forgejo workflow");
        let repository = env("GITHUB_REPOSITORY")?;
        let page = with_segments(
            Url::parse(&env("GITHUB_SERVER_URL")?).context("invalid GITHUB_SERVER_URL")?,
            repository.split('/'),
        );
        Ok(Self {
            api: Url::parse(&env("GITHUB_API_URL")?).context("invalid GITHUB_API_URL")?,
            page,
            repository,
            commit: std::env::var("GITHUB_SHA").ok(),
            auth: format!("token {}", env("RELEASE_TOKEN")?),
            agent: agent(),
        })
    }

    pub fn page(&self) -> &Url {
        &self.page
    }

    pub fn asset_url(&self, tag: &str, name: &str) -> Url {
        with_segments(self.page.clone(), ["releases", "download", tag, name])
    }

    pub fn latest_asset(&self, name: &str) -> Result<Option<String>> {
        let Some(release) = self.releases()?.into_iter().find(|release| !release.draft) else {
            return Ok(None);
        };
        let Some(asset) = release.assets.into_iter().find(|asset| asset.name == name) else {
            return Ok(None);
        };
        text_if_found(
            self.agent
                .get(&asset.browser_download_url)
                .header("Authorization", &self.auth)
                .call(),
        )
    }

    /// The draft release an author prepared for `tag`: its text, and the images and videos attached to it.
    pub fn draft(&self, tag: &str) -> Result<Option<Draft>> {
        let (draft, _) = self.tagged(tag)?;
        Ok(draft.map(|release| Draft {
            text: release.body.unwrap_or_default().trim().to_owned(),
            media: release
                .assets
                .iter()
                .filter_map(|asset| {
                    Some((
                        media_name(&asset.name)?.to_owned(),
                        self.asset_url(tag, &asset.name),
                    ))
                })
                .collect(),
        }))
    }

    /// Publishes the release for `tag` with `files` attached. A draft for the tag becomes the release and keeps
    /// what its author attached; published releases of the tag are replaced.
    pub fn publish(&self, tag: &str, notes: &str, prerelease: bool, files: &[&Path]) -> Result<()> {
        let (draft, published) = self.tagged(tag)?;
        for stale in published {
            self.delete(&self.release_url(Some(stale.id)))?;
        }
        let release = if let Some(draft) = draft {
            draft
        } else {
            // A draft stays hidden from clients until every file is attached.
            let mut body = json!({ "tag_name": tag, "name": tag, "draft": true });
            if let Some(commit) = &self.commit {
                body["target_commitish"] = json!(commit);
            }
            json(
                self.agent
                    .post(self.release_url(None).as_str())
                    .header("Authorization", &self.auth)
                    .send_json(&body),
            )?
        };
        // GitHub ends upload_url with a {?name,label} URI template; Forgejo does not.
        let uploads = release
            .upload_url
            .split_once('{')
            .map_or(release.upload_url.as_str(), |(url, _)| url);
        for path in files {
            let name = file_name(path)?;
            // A run that stopped halfway left its files on the draft, and a forge refuses a second file of a name.
            for stale in release.assets.iter().filter(|asset| asset.name == name) {
                let asset = match &stale.url {
                    Some(url) => {
                        Url::parse(url).context("the forge returned an invalid asset url")?
                    }
                    None => with_segments(
                        self.release_url(Some(release.id)),
                        ["assets", &stale.id.to_string()],
                    ),
                };
                self.delete(&asset)?;
            }
            let mut url =
                Url::parse(uploads).context("the forge returned an invalid upload_url")?;
            url.query_pairs_mut().append_pair("name", name);
            let file =
                File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
            checked(
                self.agent
                    .post(url.as_str())
                    .header("Authorization", &self.auth)
                    .header("Content-Type", "application/octet-stream")
                    .send(file),
            )?;
        }
        checked(
            self.agent
                .patch(self.release_url(Some(release.id)).as_str())
                .header("Authorization", &self.auth)
                .send_json(json!({ "body": notes, "prerelease": prerelease, "draft": false })),
        )?;
        info!(tag, page = %self.page, "published the forge release");
        Ok(())
    }

    /// The releases of `tag`: its draft, and the published ones.
    fn tagged(&self, tag: &str) -> Result<(Option<Release>, Vec<Release>)> {
        let (mut drafts, published): (Vec<_>, Vec<_>) = self
            .releases()?
            .into_iter()
            .filter(|release| release.tag_name == tag)
            .partition(|release| release.draft);
        ensure!(
            drafts.len() <= 1,
            "{tag} has {} draft releases; keep one",
            drafts.len()
        );
        Ok((drafts.pop(), published))
    }

    fn delete(&self, url: &Url) -> Result<()> {
        checked(
            self.agent
                .delete(url.as_str())
                .header("Authorization", &self.auth)
                .call(),
        )?;
        Ok(())
    }

    fn releases(&self) -> Result<Vec<Release>> {
        json(
            self.agent
                .get(self.release_url(None).as_str())
                .header("Authorization", &self.auth)
                .call(),
        )
    }

    fn release_url(&self, id: Option<u64>) -> Url {
        let id = id.map(|id| id.to_string());
        with_segments(
            self.api.clone(),
            ["repos"]
                .into_iter()
                .chain(self.repository.split('/'))
                .chain(["releases"])
                .chain(id.as_deref()),
        )
    }
}

fn with_segments<'a>(mut url: Url, segments: impl IntoIterator<Item = &'a str>) -> Url {
    url.path_segments_mut()
        .expect("http URLs have path segments")
        .pop_if_empty()
        .extend(segments);
    url
}
